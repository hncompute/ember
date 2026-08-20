use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

use crate::devices::SerialDevice;

/// A device container to route reads/writes over some address space.
/// No restrictions on what kind of device/address space this container applies to.
/// Only restriction: No two devices can overlap in this address space.
#[derive(Debug, Clone, Default)]
pub struct Bus {
    devices: BTreeMap<BusRange, Arc<Mutex<BusDevice>>>,
}

/// Tuple struct
#[derive(Debug, Copy, Clone)]
struct BusRange(u64, u64);

#[derive(Debug)]
pub enum BusDevice {
    // One variant can be active at a time
    // Tuple variant of enum
    Serial(SerialDevice<std::io::Stdin>),
}
