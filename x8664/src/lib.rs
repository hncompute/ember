use std::io;

use anyhow::Result;
use arch::{MemoryRegionConfig, PciConfig, VmImage};
use base::debug;
use memory::{GuestAddress, MemoryRegionOptions, MemoryRegionPurpose};
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
const DEFAULT_PCIE_CFG_MMIO_SIZE: u64 = 0x400_0000; // 64 MiB

const DEFAULT_PCIE_CFG_MMIO_END: u64 = FIRST_ADDR_PAST_32BIT - RESERVED_MEM_SIZE - 1;
const DEFAULT_PCIE_CFG_MMIO_START: u64 = DEFAULT_PCIE_CFG_MMIO_END - DEFAULT_PCIE_CFG_MMIO_SIZE + 1;

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
    #[error("bad PCI mem configuration: {0}")]
    ConfigurePciMem(String),
    #[error("error loading bios: {0}")]
    LoadBios(io::Error),
}

impl arch::LinuxArch for X8664Arch {
    type Error = Error;
    type ArchMemoryLayout = ArchMemoryLayout;

    fn arch_memory_layout(
        conponents: &arch::VmComponents,
    ) -> std::prelude::v1::Result<Self::ArchMemoryLayout, Self::Error> {
        create_arch_memory_layout(&conponents.pci_config)
    }

    fn guest_memory_layout(
        components: &arch::VmComponents,
        arch_memory_layout: &Self::ArchMemoryLayout,
    ) -> std::result::Result<Vec<(GuestAddress, u64, MemoryRegionOptions)>, Self::Error> {
        let bios_size = match &components.vm_image {
            VmImage::Bios(bios_file) => Some(bios_file.metadata().map_err(Error::LoadBios)?.len()),
            VmImage::Kernel(_) => None,
        };

        Ok(arch_memory_regions(
            arch_memory_layout,
            components.memory_size,
            bios_size,
        ))
    }
}

pub fn create_arch_memory_layout(pci_config: &PciConfig) -> Result<ArchMemoryLayout, Error> {
    let pcie_cfg_mmio =
        AddressRange::from_start_and_end(DEFAULT_PCIE_CFG_MMIO_START, DEFAULT_PCIE_CFG_MMIO_END);
    let pci_mmio_before_32bit = match pci_config.mem {
        Some(MemoryRegionConfig {
            start,
            size: Some(size),
        }) => AddressRange::from_start_and_size(start, size)
            .ok_or(Error::ConfigurePciMem("region overflowed".to_string()))?,
        Some(MemoryRegionConfig { start, size: None }) => {
            AddressRange::from_start_and_end(start, DEFAULT_PCIE_CFG_MMIO_END)
        }
        None => AddressRange::from_start_and_end(
            pcie_cfg_mmio.start.min(MMIO_MEM_START),
            DEFAULT_PCIE_CFG_MMIO_END,
        ),
    };

    Ok(ArchMemoryLayout {
        pci_mmio_before_32bit,
        pcie_cfg_mmio,
    })
}

/// These addresses can be used to configure GuestMemory structure for the platform.
/// For x8664 architecture, all addresses are valid from the start of the kernel except from carve
/// Return a Vec of valid memory addresses.
/// out at the end of 32bit address space (PCI hole?)
pub fn arch_memory_regions(
    arch_memory_layout: &ArchMemoryLayout,
    mem_size: u64,
    bios_size: Option<u64>,
) -> Vec<(GuestAddress, u64, MemoryRegionOptions)> {
    let mut regions = Vec::new();

    // Memory below 1 MB as conventional memory
    // following classic computer architecture
    let mem_below_1m = 640 * KB;
    regions.push((
        GuestAddress(0),
        mem_below_1m,
        MemoryRegionOptions::new().purpose(MemoryRegionPurpose::GuestMemoryRegion),
    ));

    // Reserved/BIOS data area between 640 KB and 1 MB (for system hardware and ROMs)
    // Although this range is a real, GuestMemoryRegion-backed to write BIOS tables here,
    // the guest OS should not treat it as ordinary RAM for allocation (hence ReservedMemory)
    regions.push((
        GuestAddress(mem_below_1m),
        (1 * MB) - (mem_below_1m),
        MemoryRegionOptions::new().purpose(MemoryRegionPurpose::ReservedMemory),
    ));

    // RAM betweeb 1MB to 4GB
    let mem_1m_to_4g = arch_memory_layout.pci_mmio_before_32bit.start.min(mem_size) - 1 * MB;
    regions.push((
        GuestAddress(1 * MB),
        mem_1m_to_4g,
        MemoryRegionOptions::new().purpose(MemoryRegionPurpose::GuestMemoryRegion),
    ));

    // RAM above 4GB
    let mem_above_4g = mem_size.saturating_sub(1 * MB + mem_1m_to_4g); // Stop at min to avoid underflow

    if mem_above_4g > 0 {
        regions.push((
            GuestAddress(FIRST_ADDR_PAST_32BIT),
            mem_above_4g,
            MemoryRegionOptions::new().purpose(MemoryRegionPurpose::GuestMemoryRegion),
        ));
    }

    // TODO: Add BIOS region

    // Not persistent sort all the time with equal elements?
    regions.sort_unstable_by_key(|(addr, _, _)| *addr);

    for (addr, size, options) in &regions {
        debug!(
            "{:#018x}-{:#018x} {:?}",
            addr.offset(),
            addr.offset() + size - 1,
            options.purpose,
        );
    }

    regions
}

#[cfg(test)]
mod tests {
    use arch::{MemoryRegionConfig, PciConfig};
    use memory::GuestAddress;

    use super::*;

    #[test]
    fn regions_lt_4gb_nobios() {
        let arch_memory_layout = setup();
        let size = 512 * MB;
        let regions = arch_memory_regions(&arch_memory_layout, size, None);
        assert_eq!(
            regions,
            [
                (
                    // Boot params, stack pointer, kernel cmd line, mptable?
                    GuestAddress(0),
                    // Start of low RAM
                    640 * KB,
                    MemoryRegionOptions {
                        align: 0,
                        purpose: MemoryRegionPurpose::GuestMemoryRegion,
                    },
                ),
                (
                    GuestAddress(640 * KB),
                    384 * KB,
                    MemoryRegionOptions {
                        align: 0,
                        purpose: MemoryRegionPurpose::ReservedMemory,
                    },
                ),
                (
                    // Start of high RAM
                    GuestAddress(1 * MB),
                    512 * MB - 1 * MB,
                    MemoryRegionOptions {
                        align: 0,
                        purpose: MemoryRegionPurpose::GuestMemoryRegion,
                    },
                )
            ]
        );
    }

    #[test]
    fn regions_gt_4gb_nobios() {
        let arch_memory_layout = setup();
        let size = 4 * GB + 0x8000;
        let regions = arch_memory_regions(&arch_memory_layout, size, None);
        assert_eq!(
            regions,
            [
                (
                    GuestAddress(0),
                    640 * KB,
                    MemoryRegionOptions {
                        align: 0,
                        purpose: MemoryRegionPurpose::GuestMemoryRegion,
                    },
                ),
                (
                    GuestAddress(640 * KB),
                    384 * KB,
                    MemoryRegionOptions {
                        align: 0,
                        purpose: MemoryRegionPurpose::ReservedMemory,
                    },
                ),
                (
                    GuestAddress(1 * MB),
                    2 * GB - 1 * MB,
                    MemoryRegionOptions {
                        align: 0,
                        purpose: MemoryRegionPurpose::GuestMemoryRegion,
                    },
                ),
                (
                    GuestAddress(4 * GB),
                    2 * GB + 0x8000,
                    MemoryRegionOptions {
                        align: 0,
                        purpose: MemoryRegionPurpose::GuestMemoryRegion,
                    },
                ),
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
