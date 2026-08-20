use std::{io, ops::Deref};

use vm_superio::Trigger;
use vmm_sys_util::eventfd::{EFD_NONBLOCK, EventFd};

/// Wrapper to implement trigger functionality for EventFd
/// that handles events in legacy devices
#[derive(Debug)]
pub struct EventFdTrigger(pub EventFd);

/// NOTE: Satisfy the Trigger trait
impl Trigger for EventFdTrigger {
    type E = io::Error;

    fn trigger(&self) -> io::Result<()> {
        // Increment the eventfd counter
        // so threads/event loops which poll/epoll that fd can be woken
        self.write(1)
    }
}

/// Use EventFd directly instead of coercing &EventFd to EventFd
impl Deref for EventFdTrigger {
    type Target = EventFd;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// NOTE: Define custom behaviors here, not satisfying a trait
impl EventFdTrigger {
    pub fn new() -> Self {
        // Non-blocking mode prevents reads/writes from blocking trigger
        let event_fd = EventFd::new(EFD_NONBLOCK).expect("Cannot create eventfd");
        Self(event_fd)
    }

    // Duplicate the underlying FD.
    // Use when different components/threads need to handle the same event FD,
    // so each can poll/write independently.
    pub fn try_clone(&self) -> io::Result<Self> {
        self.0.try_clone().map(Self)
    }
}
