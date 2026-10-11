use acpi_tables::sdt::SDT;
use memory::{GuestAddress, MemoryRegionOptions};
use remain::sorted;
use serde::{Deserialize, Serialize};
use serde_kv_derive::FromKeyValues;
use std::{error::Error as StdError, fs::File};

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
    pub pci_config: PciConfig,
    pub vm_image: VmImage,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryRegionConfig {
    pub start: u64,
    pub size: Option<u64>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, FromKeyValues)]
pub struct PciConfig {
    pub mem: Option<MemoryRegionConfig>,
}

pub trait LinuxArch {
    type Error: StdError;
    type ArchMemoryLayout;

    /// Decide architecture layout
    fn arch_memory_layout(components: &VmComponents)
    -> Result<Self::ArchMemoryLayout, Self::Error>;

    /// Return a Vec of valid memory addresses as pairs of address and length
    fn guest_memory_layout(
        components: &VmComponents,
        arch_memory_layout: &Self::ArchMemoryLayout,
    ) -> std::result::Result<Vec<(GuestAddress, u64, MemoryRegionOptions)>, Self::Error>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use argh::FromArgValue;

    #[test]
    fn parse_pci_config_with_derive() {
        // NOTE: We use [] for nested structs, not input args
        let config = PciConfig::from_arg_value("mem=[start=2147483648,size=1048576]").unwrap();
        assert_eq!(
            config.mem,
            Some(MemoryRegionConfig {
                start: 2147483648,
                size: Some(1048576),
            })
        );
        let config = PciConfig::from_arg_value("mem=[start=2147483648]").unwrap();
        assert_eq!(
            config.mem,
            Some(MemoryRegionConfig {
                start: 2147483648,
                size: None,
            })
        );
    }

    #[test]
    fn derive_returns_parse_errors() {
        let input = "mem=[start=invalid]";
        let expected = serde_keyvalue::from_key_values::<PciConfig>(input)
            .unwrap_err()
            .to_string();
        assert!(!expected.is_empty());
        assert_eq!(PciConfig::from_arg_value(input), Err(expected));
    }
}
