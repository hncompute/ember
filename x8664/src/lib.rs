use anyhow::Result;
use arch::{MemoryRegionConfig, PciConfig};
use remain::sorted;
use resources::AddressRange;
use thiserror::Error;

const KB: u64 = 1 << 10;
const MB: u64 = 1 << 20;
const GB: u64 = 1 << 30;
const FIRST_ADDR_PAST_32BIT: u64 = 1 << 32; // 4 GiB
const MEM_32BIT_GAP_SIZE: u64 = 768 * MB; // 768 MiB
// Reserved region of physical address space for MMIO mappings,
// keep device address ranges from colliding with RAM
const MMIO_MEM_START: u64 = FIRST_ADDR_PAST_32BIT - MEM_32BIT_GAP_SIZE;

// Reserved region for system boot and MMIO
const RESERVED_MEM_SIZE: u64 = 0x800_0000; // 128 MiB
const DEFAULT_PCI_MEM_END: u64 = FIRST_ADDR_PAST_32BIT - RESERVED_MEM_SIZE - 1;

pub struct ArchMemoryLayout {
    // range below 4G (bios, elf segments, initramfs...)
    pci_mmio_before_32bit: AddressRange,
    pcie_cfg_mmio: AddressRange,
}

pub struct X8664Arch;

#[sorted]
#[derive(Error, Debug)]
pub enum Error {
    #[error("bad PCI ECAM configuration: {0}")]
    ConfigurePciEcam(String),
}

impl arch::LinuxArch for X8664Arch {
    type Error = Error;
    type ArchMemoryLayout = ArchMemoryLayout;
}

pub fn create_arch_memory_layout(pci_config: &PciConfig) -> Result<ArchMemoryLayout, Error> {
    let pci_mmio_before_32bit = match pci_config.mem {
        Some(MemoryRegionConfig {
            start,
            size: Some(size),
        }) => AddressRange::from_start_and_size(start, size)
            .ok_or(Error::ConfigurePciMem("region overflowed".to_string()))?,
        Some(MemoryRegionConfig { start, size: None }) => {
            AddressRange::from_start_and_end(start, DEFAULT_PCI_MEM_END)
        }
        None => AddressRange::from_start_and_end(
            pcie_cfg_mmio.start.min(MMIO_MEM_START),
            DEFAULT_PCI_MEM_END,
        ),
    };

    Ok(ArchMemoryLayout {
        pci_mmio_before_32bit,
        pcie_cfg_mmio,
    })
}

#[cfg(test)]
mod tests {
    use arch::{MemoryRegionConfig, PciConfig};
    use vm_memory::GuestAddress;

    use super::*;

    #[test]
    fn regions_lt_4gb_nobios() {
        let arch_memory_layout = setup();
        let regions = arch_memory_regions(&arch_memory_layout, 512 * MB, None);
        assert_eq!(
            regions,
            [
                (
                    // Start of low RAM
                    // Boot params, stack pointer, kernel cmd line, mptable?
                    GuestAddress(0),
                    640 * KB,
                    MemoryRegionOptions {
                        align: 0,
                        purpose: MemoryRegionPurpose::GuestMemoryRegion,
                        file_backed: None,
                    },
                ),
                (
                    GuestAddress(640 * KB),
                    384 * KB,
                    MemoryRegionOptions {
                        align: 0,
                        purpose: MemoryRegionPurpose::ReservedMemory,
                        file_backed: None,
                    },
                ),
                (
                    // Start of high RAM
                    GuestAddress(1 * MB),
                    2 * GB - 1 * MB,
                    MemoryRegionOptions {
                        align: 0,
                        purpose: MemoryRegionPurpose::ReservedMemory,
                        file_backed: None,
                    },
                )
            ]
        );
    }

    fn setup() -> ArchMemoryLayout {
        let pci_config = PciConfig {
            mem: Some(MemoryRegionConfig {
                start: 2 * GB,
                size: None,
            }),
        };
        create_arch_memory_layout(&pci_config).unwrap()
    }
}
