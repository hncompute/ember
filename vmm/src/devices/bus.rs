use crate::devices::SerialDevice;

#[derive(Debug)]
pub enum BusDevice {
    // One variant can be active at a time
    // Tuple variant of enum
    Serial(SerialDevice<std::io::Stdin>),
}
