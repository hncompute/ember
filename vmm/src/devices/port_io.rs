use crate::devices::{BusDevice, EventFdTrigger, SerialDevice, SerialOut};
use anyhow::Result;
use kvm_ioctls::VmFd;
use nix::libc::EFD_NONBLOCK;
use std::sync::{Arc, Mutex};
use vm_superio::{Serial, serial::NoEvents};
use vmm_sys_util::eventfd::EventFd;

// Wrapper to register legacy(?) devices on I/O bus
// currently manage uart and i8042 devices?
#[derive(Debug)]
pub struct PortIODeviceManager {
    pub io_bus: crate::devices::Bus,
    /// Represent a serial device?
    pub stdio_serial: Arc<Mutex<BusDevice>>,
    /// IRQ4/GSI4
    pub com_evt_1_3: EventFdTrigger,
    /// IRQ3/GSI3
    pub com_evt_2_4: EventFdTrigger,
    pub kbd_evt: EventFd,
}

impl PortIODeviceManager {
    /// x86 global system interrupt for communication events on serial ports 1 to 4.
    /// See <https://en.wikipedia.org/wiki/Interrupt_request_(PC_architecture)>.
    const COM_EVT_1_3_GSI: u32 = 4;
    const COM_EVT_2_4_GSI: u32 = 3;
    const KBD_EVT_GSI: u32 = 1;
    /// Legacy serial port device addresses from /dev/ttyS0 to /dev/ttyS3
    /// See <https://tldp.org/HOWTO/Serial-HOWTO-10.html#ss10.1>.
    const SERIAL_PORT_ADDRESSES: [u64; 4] = [0x3f8, 0x2f8, 0x3e8, 0x2e8];
    /// Size of legacy serial ports.
    const SERIAL_PORT_SIZE: u64 = 0x8;

    pub fn new(serial: Arc<Mutex<BusDevice>>) -> Result<Self> {
        // Sanity check to be Serial variant
        debug_assert!(matches!(*serial.lock().unwrap(), BusDevice::Serial(_)));
        let io_bus = crate::devices::Bus::new();
        let com_evt_1_3 = serial
            .lock()
            .expect("Poisoned lock")
            .serial_mut()
            .unwrap()
            .serial
            .interrupt_evt()
            .try_clone()?;

        let com_evt_2_4 = EventFdTrigger::new();
        // Enable non-blocking I/O operations on keyboard events,
        // saving calls to fcntl syscall
        let kbd_evt = EventFd::new(EFD_NONBLOCK)?;

        Ok(PortIODeviceManager {
            io_bus,
            stdio_serial: serial,
            com_evt_1_3,
            com_evt_2_4,
            kbd_evt,
        })
    }

    pub fn register_devices(&mut self, vm_fd: &VmFd) -> Result<()> {
        let serial_2_4 = Arc::new(Mutex::new(BusDevice::Serial(SerialDevice {
            serial: Serial::with_events(
                self.com_evt_2_4.try_clone()?,
                NoEvents,
                SerialOut::Sink(std::io::sink()),
            ),
            input: None,
        })));

        let serial_1_3 = Arc::new(Mutex::new(BusDevice::Serial(SerialDevice {
            serial: Serial::with_events(
                self.com_evt_1_3.try_clone()?,
                NoEvents,
                SerialOut::Sink(std::io::sink()),
            ),
            input: None,
        })));

        // Main active interactive serial port
        self.io_bus.insert(
            self.stdio_serial.clone(),
            Self::SERIAL_PORT_ADDRESSES[0],
            Self::SERIAL_PORT_SIZE,
        )?;

        // TODO: COM2 and COM4 share interrupts but not device state (read/write to buffer,
        // registers, etc.) in real hardware
        // We should not lock the same BusDevice here
        self.io_bus.insert(
            serial_2_4.clone(),
            Self::SERIAL_PORT_ADDRESSES[1],
            Self::SERIAL_PORT_SIZE,
        )?;

        // Dummy devices, sink to /dev/null?
        self.io_bus.insert(
            serial_1_3,
            Self::SERIAL_PORT_ADDRESSES[2],
            Self::SERIAL_PORT_SIZE,
        )?;

        self.io_bus.insert(
            serial_2_4,
            Self::SERIAL_PORT_ADDRESSES[3],
            Self::SERIAL_PORT_SIZE,
        )?;

        vm_fd.register_irqfd(&self.com_evt_1_3, Self::COM_EVT_1_3_GSI)?;
        vm_fd.register_irqfd(&self.com_evt_2_4, Self::COM_EVT_2_4_GSI)?;
        vm_fd.register_irqfd(&self.kbd_evt, Self::KBD_EVT_GSI)?;

        Ok(())
    }
}
