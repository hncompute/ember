use anyhow::Result;
use std::{
    io::{Read, Write},
    os::fd::AsRawFd,
    sync::{Arc, Mutex},
};
use vm_superio::{Serial, Trigger, serial::NoEvents, serial::SerialEvents};

use crate::devices::{BusDevice, EventFdTrigger};

#[derive(Debug)]
pub enum SerialOut {
    /// Move data into the void
    Sink(std::io::Sink),
    Stdout(std::io::Stdout),
    // Write to standard output
}

impl Write for SerialOut {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            Self::Sink(sink) => sink.write(buf),
            Self::Stdout(stdout) => stdout.write(buf),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            Self::Sink(sink) => sink.flush(),
            Self::Stdout(stdout) => stdout.flush(),
        }
    }
}

/// Wrapper over imported serial device
#[derive(Debug)]
pub struct SerialWrapper<T: Trigger, EV: SerialEvents, I: Read + AsRawFd + Send> {
    /// Represent serial device object.
    /// This handles UART registers and FIFO, guest reads/writes, interrupt triggering, and output through Write implementation.
    pub serial: Serial<T, EV, SerialOut>,
    /// Input to serial device (must be readable!).
    /// Host stdin/file/socket.
    pub input: Option<I>,
}

/// Type to represent a serial device (send/receive one bit at a time).
/// NOTE: We don't intend to implement custom event handling, thus concrete NoEvents (replace SerialEventsWrapper).
/// If yes, we need to `impl SerialEvents` trait with 4 event hooks/callbacks.
pub type SerialDevice<I> = SerialWrapper<EventFdTrigger, NoEvents, I>;

pub fn setup_serial_device(
    input: std::io::Stdin,
    out: std::io::Stdout,
) -> Result<Arc<Mutex<BusDevice>>> {
    let interrupt_evt = EventFdTrigger::new();

    let serial = Arc::new(Mutex::new(BusDevice::Serial(SerialWrapper {
        // Create instance of Serial with trigger, SerialEvents impl, and output
        serial: Serial::with_events(interrupt_evt, NoEvents, SerialOut::Stdout(out)),
        input: Some(input),
    })));

    Ok(serial)
}
