use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

use serde::{Deserialize, Serialize};

/// A device container for routing reads and writes over some address space.
/// Only restriction is no two devices can overlap in this address sapce.
#[derive(Clone)]
pub struct Bus {
    /// Keep mapping sorted by base address, and ensure there are multiple owners of the device,
    /// but only one thread can access the device at a time.
    devices: BTreeMap<BusRange, Arc<Mutex<dyn BusDevice>>>,
    access_id: usize,
    bus_type: BusType,
}

#[derive(Copy, Clone, Serialize, Deserialize)]
pub struct BusRange {
    pub base: u64,
    pub len: u64,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize, PartialOrd, Ord)]
pub enum BusType {
    Mmio,
    Io,
    Hypercall,
}

/// Information about how a device was accessed.
#[derive(Copy, Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct BusAccessInfo {
    /// Offset from base address that the device was accessed at.
    pub offset: u64,
    /// Absolute address of the device's access in its address space.
    pub address: u64,
    /// ID of the entity requesting a device access, usually the VCPU id.
    pub id: usize,
}

/// Devices that respond to reads and writes in an arbitrary address space.
pub trait BusDevice: Send {
    // Perform a read access to this device
    fn read(&mut self, info: BusAccessInfo, data: &mut [u8]) {}
    // Perform a write access to this device
    fn write(&mut self, info: BusAccessInfo, data: &[u8]) {}
}
