use anyhow::Result;
use std::{
    cmp::Ordering,
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

use crate::devices::SerialDevice;

/// A device container to route reads/writes over some address space.
/// No restrictions on what kind of device/address space this container applies to.
/// Only restriction: No two devices can overlap in this address space.
#[derive(Debug, Clone, Default)]
pub struct Bus {
    // O(log n) lookup/insert (enough for small/moderate no of devices?)
    devices: BTreeMap<BusRange, Arc<Mutex<BusDevice>>>,
}

impl Bus {
    pub fn new() -> Bus {
        Bus {
            devices: BTreeMap::new(),
        }
    }

    /// Put the given device at the given address space
    pub fn insert(&mut self, device: Arc<Mutex<BusDevice>>, base: u64, len: u64) -> Result<()> {
        if len == 0 {
            anyhow::bail!("Cannot insert a device with zero length")
        }

        // Reject cases where new device's range is within old device' range
        if self.get_device(base).is_some() {
            anyhow::bail!("New device overlaps with existing device")
        }

        // The new device with a range before the new device's range, but new device's end overlaps
        // with existing device
        if let Some((BusRange(start, _), _)) = self.first_before(base + len - 1) {
            if start >= base {
                anyhow::bail!("New device end overlaps with existing device")
            }
        }

        if self.devices.insert(BusRange(base, len), device).is_some() {
            anyhow::bail!("Device already exists at this address")
        }
        Ok(())
    }

    /// Read data from the device that owns the range addr and puts it into data
    pub fn read(&self, addr: u64, data: &mut [u8]) -> bool {
        if let Some((offset, dev)) = self.get_device(addr) {
            dev.lock()
                .expect("Failed to acquire device lock")
                .read(offset, data);
            true
        } else {
            false
        }
    }

    pub fn write(&self, addr: u64, data: &[u8]) -> bool {
        if let Some((offset, dev)) = self.get_device(addr) {
            dev.lock()
                .expect("Failed to acquire device lock")
                .write(offset, data);
            true
        } else {
            false
        }
    }

    fn first_before(&self, addr: u64) -> Option<(BusRange, &Mutex<BusDevice>)> {
        for (range, dev) in self.devices.iter().rev() {
            if range.0 <= addr {
                return Some((*range, dev));
            }
        }
        None
    }

    fn get_device(&self, addr: u64) -> Option<(u64, &Mutex<BusDevice>)> {
        if let Some((BusRange(start, len), dev)) = self.first_before(addr) {
            let offset = addr - start;
            if offset < len {
                return Some((offset, dev));
            }
        }
        None
    }
}

#[derive(Debug, Copy, Clone)]
// Start and Length
struct BusRange(u64, u64);

// For mathematically? well-behaved equality
// like i32 with x == x (value equal itself)
impl Eq for BusRange {}

// Enable == and !=
impl PartialEq for BusRange {
    fn eq(&self, other: &BusRange) -> bool {
        self.0 == other.0
    }
}

impl Ord for BusRange {
    fn cmp(&self, other: &BusRange) -> Ordering {
        self.0.cmp(&other.0)
    }
}

impl PartialOrd for BusRange {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug)]
pub enum BusDevice {
    // One variant can be active at a time
    // Tuple variant of enum
    Serial(SerialDevice<std::io::Stdin>),
}

impl BusDevice {
    /// Return a mutable reference to the inner SerialDevice when the BusDevice is of Serial variant
    pub fn serial_mut(&mut self) -> Option<&mut SerialDevice<std::io::Stdin>> {
        match self {
            Self::Serial(x) => Some(x),
        }
    }

    pub fn read(&mut self, offset: u64, data: &mut [u8]) {
        match self {
            Self::Serial(x) => x.bus_read(offset, data),
        }
    }

    pub fn write(&mut self, offset: u64, data: &[u8]) {
        match self {
            Self::Serial(x) => x.bus_write(offset, data),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::{Bus, BusDevice};

    fn test_device() -> Arc<Mutex<BusDevice>> {
        crate::devices::setup_serial_device(std::io::stdin(), std::io::stdout())
            .expect("test serial device should be created")
    }

    #[test]
    fn insert_adds_a_device_to_an_empty_bus() {
        let mut bus = Bus::new();

        bus.insert(test_device(), 0x1000, 0x10)
            .expect("a non-empty range on an empty bus should be accepted");

        assert_eq!(bus.devices.len(), 1);
    }

    #[test]
    fn insert_rejects_a_zero_length_range() {
        let mut bus = Bus::new();

        assert!(bus.insert(test_device(), 0x1000, 0).is_err());
        assert!(bus.devices.is_empty());
    }

    #[test]
    fn insert_rejects_a_device_at_an_existing_base_address() {
        let mut bus = Bus::new();
        bus.insert(test_device(), 0x1000, 0x10).unwrap();

        assert!(bus.insert(test_device(), 0x1000, 0x10).is_err());
        assert_eq!(bus.devices.len(), 1);
    }

    #[test]
    fn insert_rejects_a_range_that_starts_inside_an_existing_device() {
        let mut bus = Bus::new();
        bus.insert(test_device(), 0x1000, 0x20).unwrap();

        assert!(bus.insert(test_device(), 0x1010, 0x10).is_err());
        assert_eq!(bus.devices.len(), 1);
    }

    #[test]
    fn insert_rejects_a_range_that_ends_inside_an_existing_device() {
        let mut bus = Bus::new();
        bus.insert(test_device(), 0x1010, 0x20).unwrap();

        assert!(bus.insert(test_device(), 0x1000, 0x20).is_err());
        assert_eq!(bus.devices.len(), 1);
    }

    #[test]
    fn insert_allows_ranges_that_touch_but_do_not_overlap() {
        let mut bus = Bus::new();
        bus.insert(test_device(), 0x1000, 0x10).unwrap();

        bus.insert(test_device(), 0x1010, 0x10)
            .expect("the end of one range may be the start of the next range");

        assert_eq!(bus.devices.len(), 2);
    }
}
