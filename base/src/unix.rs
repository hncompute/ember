use crate::{SafeDescriptor, descriptor::IntoRawDescriptor};
use std::{
    fs::File,
    os::{
        fd::{FromRawFd, OwnedFd},
        unix::io::{IntoRawFd, RawFd},
    },
};

pub type RawDescriptor = RawFd;

impl From<SafeDescriptor> for File {
    fn from(s: SafeDescriptor) -> Self {
        // Safe because we own the descriptor at this point
        unsafe { File::from_raw_fd(s.into_raw_descriptor()) }
    }
}

macro_rules! IntoRawDescriptor {
    ($name:ident) => {
        impl IntoRawDescriptor for $name {
            fn into_raw_descriptor(self) -> RawDescriptor {
                self.into_raw_fd()
            }
        }
    };
}

IntoRawDescriptor!(OwnedFd);
IntoRawDescriptor!(File);
