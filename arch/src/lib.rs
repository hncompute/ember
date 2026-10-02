use acpi_tables::sdt::SDT;
use anyhow::Result;
use remain::sorted;
use std::{error::Error as StdError, fs::File};

pub mod x86_64;
pub use x86_64::*;

pub mod config;
pub use config::{BootSourceConfig, InitramfsConfig};

/// Default (smallest) memory page size for the supported architectures.
pub const PAGE_SIZE: usize = 4096;

pub enum VmImage {
    Kernel(File),
    Bios(File),
}

// Hold the pieces to build a VM.
#[sorted] // Ensure enum variants are sorted
pub struct VmComponents {
    pub acpi_sdts: Vec<SDT>,
    pub hugepages: bool,
    pub initramfs_image: Option<File>,
    pub memory_size: u64,
    pub vm_image: VmImage,
}

pub trait LinuxArch {
    type Error: StdError;
    type ArchMemoryLayout;

    // Decide architecture layout
    fn arch_memory_layout(conponents: &VmComponents) -> Result<Self::ArchMemoryLayout>;
}
