use std::os::fd::AsRawFd;

/// Wrapper over imported serial device
#[derive(Debug)]
pub struct SerialWrapper<T: Trigger, EV: SerialEvents, I: Read + AsRawFd + Send> {
    /// Represent serial device object
    pub serial: Serial<T, EV, SerialOut>,
    /// Input to serial device (must be readable!)
    pub input: Option<I>,
}
/// Type to represent a serial device (send/receive one bit at a time)
pub type SerialDevice<I> = SerialWrapper<EventFdTrigger, SerialEventsWrapper, I>;
