use std::sync::{Arc, Mutex};

// Wrapper to register legacy(?) devices on I/O bus
// currently manage uart and i8042 devices?
#[derive(Debug)]
pub struct PortIODeviceManager {
    pub io_bus: crate::devices::Bus,
    // BusDevice::Serial?
    // Why could this be used by multiple threads? (Hence Arc)
    pub stdio_serial: Arc<Mutex<BusDevice>>,
    // Communication event on port 1 & 3
    pub com_evt_1_3: EventFdTrigger,
    // Communication event on port 2 & 4
    pub com_evt_2_4: EventFdTrigger,
    // Keyboard event
    pub kbd_evt: EventFd,
}
