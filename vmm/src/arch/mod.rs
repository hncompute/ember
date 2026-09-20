pub mod x86_64;
pub use x86_64::*;

pub mod config;
pub use config::{BootSourceConfig, InitramfsConfig};


/// Default (smallest) memory page size for the supported architectures.
pub const PAGE_SIZE: usize = 4096;
