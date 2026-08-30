use anyhow::{Context, Result};
use linux_loader::{
    bootparam::boot_params, configurator::{BootConfigurator, BootParams, linux::LinuxBootConfigurator}, loader::{Cmdline, Elf, KernelLoader, load_cmdline},
};
use std::{
    fs::File,
    io::{Seek, SeekFrom},
    path::Path,
};

use vm_memory::{
    Address, GuestAddress, GuestMemoryBackend, GuestMemoryMmap, ReadVolatile, VolatileMemory,
};

use crate::arch::DEFAULT_KERNEL_CMDLINE;

const FIRST_ADDR_PAST_32BITS: u64 = 1 << 32; // 4 GiB
const MEM_32BIT_GAP_SIZE: u64 = 768 << 20; // 768 MiB
// Reserved region of physical address space for MMIO mappings,
// keep device address ranges from colliding with RAM
const MMIO_MEM_START: u64 = FIRST_ADDR_PAST_32BITS - MEM_32BIT_GAP_SIZE;

// EBDA is located in the LAST 1KB of the FIRST 640KiB of memory i.e., [0x9FC00, 0x9FFFF]
// We mark first [0x0, EBDA_START] region as usable RAM
// and [EBDA_START, (EBDA_START + EBDA_SIZE)] as reserved.
const EBDA_START: u64 = 0x9fc00;
const EBDA_SIZE: u64 = 1 << 10;
// Value taken from https://elixir.bootlin.com/linux/v5.10.68/source/arch/x86/include/uapi/asm/e820.h#L31
// Usable normal RAM
const E820_RAM: u32 = 1;
// Reserved area that should be avoided during memory allocations
const E820_RESERVED: u32 = 2;

pub fn load_kernel<P: AsRef<Path>>(
    kernel_image_path: P, // Cheap ref-to-ref conversion, so caller can accept an owned string?
    guest_mem: &GuestMemoryMmap,
) -> Result<GuestAddress> {
    let mut kernel_file = File::open(kernel_image_path).expect("open kernel file failed");
    let kernel_load = Elf::load(
        guest_mem,
        None,
        &mut kernel_file,
        Some(GuestAddress(super::layout::KERNEL_START_ADDRESS)),
    )?
    .kernel_load;

    Ok(kernel_load)
}

pub fn load_initramfs<P: AsRef<Path>>(
    initramfs_path: P,
    vm_memory: &GuestMemoryMmap,
) -> Result<crate::arch::InitramfsConfig> {
    let mut image = File::open(initramfs_path).context("failed to open initramfs file")?;

    // Offset = size of obj + specified num of bytes
    let size = image.seek(SeekFrom::End(0))? as usize;
    if size == 0 {
        // Macro for return Err
        anyhow::bail!("Initramfs image seek returned a size of zero")
    }

    image.seek(SeekFrom::Start(0))?;

    let addr = initramfs_load_addr(vm_memory, size as usize)?;

    // Return a slice of raw memory
    // with volatile access (prevent compiler from performing optimizations normally applied to
    // regular memory reads and writes)
    // like MMIO or shared memory regions that can change externally.
    let mut slice = vm_memory.get_slice(GuestAddress(addr), size as usize)?;

    // Fill the slice with image bytes
    image.read_exact_volatile(&mut slice)?;

    Ok(crate::arch::InitramfsConfig {
        address: GuestAddress(addr),
        size,
    })
}

pub fn load_boot_cmdline(
    boot_args: &Option<String>,
    guest_mem: &GuestMemoryMmap,
) -> Result<(GuestAddress, usize)> {
    let cmdline_addr = GuestAddress(crate::arch::layout::CMDLINE_START);
    let cmdline_str = match boot_args.as_ref() {
        None => DEFAULT_KERNEL_CMDLINE,
        Some(str) => str.as_str(),
    };
    // Safely convert one data type to another
    // when the conversion might fail (not fit?)
    let cmdline = Cmdline::try_from(cmdline_str, super::layout::CMDLINE_MAX_SIZE)?;
    let size = cmdline
        .as_cstring()
        .map(|cmdline_cstring| cmdline_cstring.as_bytes_with_nul().len())?;

    load_cmdline(guest_mem, cmdline_addr, &cmdline).context("failed to boot cmdline")?;

    Ok((cmdline_addr, size))
}

pub fn configure_system(
    guest_mem: &GuestMemoryMmap,
    cmdline_addr: GuestAddress,
    cmdline_size: usize,
    initramfs: &Option<crate::arch::InitramfsConfig>,
) -> Result<()> {
    // Signature for valid boot sector
    const KERNEL_BOOT_FLAG_MAGIC: u16 = 0xaa55;
    // Magic number spelling ASCII string HdrS, used it Linux boot protocol
    const KERNEL_HDR_MAGIC: u32 = 0x5372_6448;
    // Type of loader. We use none of the standards like LILO or GRUB
    const KERNEL_LOADER_OTHER: u8 = 0xff;
    const KERNEL_MIN_ALIGNMENT_BYTES: u32 = 0x0100_0000;

    let first_addr_past_32bits = GuestAddress(FIRST_ADDR_PAST_32BITS);
    let end_32bits_gap_start = GuestAddress(MMIO_MEM_START);

    let himem_start = GuestAddress(crate::arch::layout::KERNEL_START_ADDRESS);

    // TODO: Support mptable
    // Note that this puts the mptable at the last 1k of Linux's 640k base RAM
    // mptable::setup_mptable(guest_mem, num_cpus)?;

    let mut params = boot_params::default();

    params.hdr.type_of_loader = KERNEL_LOADER_OTHER;
    params.hdr.boot_flag = KERNEL_BOOT_FLAG_MAGIC;
    params.hdr.header = KERNEL_HDR_MAGIC;
    params.hdr.cmd_line_ptr = u32::try_from(cmdline_addr.raw_value())?;
    params.hdr.cmdline_size = u32::try_from(cmdline_size)?;
    params.hdr.kernel_alignment = KERNEL_MIN_ALIGNMENT_BYTES;

    if let Some(initramfs_config) = initramfs {
        params.hdr.ramdisk_image = u32::try_from(initramfs_config.address.raw_value())?;
        params.hdr.ramdisk_size = u32::try_from(initramfs_config.size)?;
    }

    add_e820_entry(&mut params, 0, EBDA_START, E820_RAM)?;
    // We do not want to scan beyond the memory boundary (touching the video memory range)
    add_e820_entry(&mut params, EBDA_START, EBDA_SIZE, E820_RESERVED)?;

    let last_addr = guest_mem.last_addr();

    if last_addr < end_32bits_gap_start {
        // Fill in the PCI hole
        add_e820_entry(
            &mut params,
            himem_start.raw_value(),
            last_addr.unchecked_offset_from(himem_start) + 1,
            E820_RAM,
        )?;
    } else {
        // Low-RAM entry up to PCI hole
        add_e820_entry(
            &mut params,
            himem_start.raw_value(),
            end_32bits_gap_start.unchecked_offset_from(himem_start),
            E820_RAM,
        )?;

        // Separate high-RAM entry up to 4 GiB boundary
        if last_addr > first_addr_past_32bits {
            add_e820_entry(
                &mut params,
                first_addr_past_32bits.raw_value(),
                last_addr.unchecked_offset_from(first_addr_past_32bits) + 1,
                E820_RAM,
            )?;
        }
    }

    LinuxBootConfigurator::write_bootparams(&BootParams::new(&params, GuestAddress(crate::arch::layout::ZERO_PAGE_START)), guest_mem).context("failed to write bootparams")?;

    Ok(())
}

/// Add a region to the e820 map, return an error if there is no space left
fn add_e820_entry(params: &mut boot_params, addr: u64, size: u64, mem_type: u32) -> Result<()> {
    if params.e820_entries as usize >= params.e820_table.len() {
        anyhow::bail!("e820 configuration error")
    }

    params.e820_table[params.e820_entries as usize].addr = addr;
    params.e820_table[params.e820_entries as usize].size = size;
    params.e820_table[params.e820_entries as usize].r#type = mem_type;
    params.e820_entries += 1;

    Ok(())
}

fn initramfs_load_addr(vm_memory: &GuestMemoryMmap, initramfs_size: usize) -> Result<u64> {
    let first_region = vm_memory
        .find_region(GuestAddress::new(0))
        .context("failed to find guest memory region")?;

    // Memory goes from low to high?
    let lowmem_size = first_region.len() as usize;
    if lowmem_size < initramfs_size {
        anyhow::bail!("initramfs size is too big");
    }

    // address & 0xFFFFF000
    // bit 0 in the lower 12 positions
    // rounding down to nearest multiple of 4096
    let align_to_pagesize = |address| address & !(crate::arch::PAGE_SIZE - 1);

    // NOTE: We intend to place initramfs at high address
    // to reserve memory for: Boot params, kernel cmd line and EBDA
    // and we don't want initramfs to overshoot the boundary
    Ok(align_to_pagesize(lowmem_size - initramfs_size) as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use linux_loader::bootparam::boot_params;
    use vm_memory::GuestMemoryMmap;

    #[test]
    fn initramfs_load_addr_valid() {
        let guest_mem = create_guest_memory(0x100000); // 1 MB
        let initramfs_size = 0x1000; // 4 KB

        let result = initramfs_load_addr(&guest_mem, initramfs_size);
        assert!(result.is_ok());

        let addr = result.unwrap();

        assert_eq!(addr % crate::arch::PAGE_SIZE as u64, 0);
        assert!(addr < 0x100000);
        assert!(addr + initramfs_size as u64 <= 0x100000);
    }

    #[test]
    fn initramfs_load_addr_too_large() {
        let guest_mem = create_guest_memory(0x100000);
        let initramfs_size = 0x200000; // 18 KB (larger than memory)

        let result = initramfs_load_addr(&guest_mem, initramfs_size);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("initramfs size is too big")
        );
    }

    #[test]
    fn initramfs_load_addr_page_alignment() {
        let guest_mem = create_guest_memory(0x100000); // 1 MB
        let initramfs_size = 0x3500; // Size that tests alignment

        let result = initramfs_load_addr(&guest_mem, initramfs_size);
        assert!(result.is_ok());

        let addr = result.unwrap();
        // Verify address is page-aligned (4096 bytes)
        assert_eq!(addr % 4096, 0);
    }

    #[test]
    fn initramfs_load_addr_at_boundary() {
        let guest_mem = create_guest_memory(0x10000); // 64 KB
        let initramfs_size = 0x10000; // Exactly fits memory

        let result = initramfs_load_addr(&guest_mem, initramfs_size);
        assert!(result.is_ok());

        let addr = result.unwrap();
        assert_eq!(addr, 0); // Should be at start
        assert_eq!(addr % 4096, 0); // Still page-aligned
    }

    #[test]
    fn initramfs_load_addr_small_initramfs() {
        let guest_mem = create_guest_memory(0x10000); // 64 KB
        let initramfs_size = 0x100; // 256 bytes

        let result = initramfs_load_addr(&guest_mem, initramfs_size);
        assert!(result.is_ok());

        let addr = result.unwrap();
        assert!(addr > 0);
        assert_eq!(addr % 4096, 0);
    }

    #[test]
    fn add_e820_entry_success() {
        let mut params = boot_params::default();

        let result = add_e820_entry(&mut params, 0x0, 0x1000, E820_RAM);
        let entries_len = params.e820_entries;
        let first_entry_addr = params.e820_table[0].addr;
        let first_entry_size = params.e820_table[0].size;
        let first_entry_type = params.e820_table[0].r#type;
        assert!(result.is_ok());
        assert_eq!(entries_len, 1);
        assert_eq!(first_entry_addr, 0x0);
        assert_eq!(first_entry_size, 0x1000);
        assert_eq!(first_entry_type, E820_RAM);
    }

    #[test]
    fn add_e820_entry_multiple() {
        let mut params = boot_params::default();

        let result1 = add_e820_entry(&mut params, 0x0, EBDA_START, E820_RAM);
        assert!(result1.is_ok());

        let result2 = add_e820_entry(&mut params, EBDA_START, EBDA_SIZE, E820_RESERVED);
        assert!(result2.is_ok());

        let entries_len = params.e820_entries;
        let first_entry_type = params.e820_table[0].r#type;
        let second_entry_type = params.e820_table[1].r#type;
        assert_eq!(entries_len, 2);
        assert_eq!(first_entry_type, E820_RAM);
        assert_eq!(second_entry_type, E820_RESERVED);
    }

    #[test]
    fn add_e820_entry_table_full() {
        let mut params = boot_params::default();

        // Fill the e820 table to capacity
        let table_len = params.e820_table.len();
        for i in 0..table_len {
            let result = add_e820_entry(&mut params, i as u64 * 0x1000, 0x1000, E820_RAM);
            assert!(result.is_ok());
        }

        // Attempt to add one more entry when table is full
        let result = add_e820_entry(&mut params, 0x10000, 0x1000, E820_RAM);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("e820 configuration error")
        );
    }

    #[test]
    fn add_e820_entry_reserved_type() {
        let mut params = boot_params::default();

        let result = add_e820_entry(&mut params, 0x9fc00, 0x400, E820_RESERVED);
        assert!(result.is_ok());
        let first_entry_type = params.e820_table[0].r#type;
        let first_entry_addr = params.e820_table[0].addr;
        let first_entry_size = params.e820_table[0].size;
        assert_eq!(first_entry_type, E820_RESERVED);
        assert_eq!(first_entry_addr, 0x9fc00);
        assert_eq!(first_entry_size, 0x400);
    }

    #[test]
    fn add_e820_entry_sequence() {
        let mut params = boot_params::default();

        // Simulate typical boot parameter setup
        add_e820_entry(&mut params, 0, EBDA_START, E820_RAM).unwrap();
        add_e820_entry(&mut params, EBDA_START, EBDA_SIZE, E820_RESERVED).unwrap();
        add_e820_entry(&mut params, 0x100000, 0xf00000, E820_RAM).unwrap();

        let entries_len = params.e820_entries;
        let first_entry_addr = params.e820_table[0].addr;
        let second_entry_addr = params.e820_table[1].addr;
        let third_entry_addr = params.e820_table[2].addr;
        assert_eq!(entries_len, 3);
        // Verify entries are in correct order
        assert_eq!(first_entry_addr, 0);
        assert_eq!(second_entry_addr, EBDA_START);
        assert_eq!(third_entry_addr, 0x100000);
    }

    #[test]
    fn add_e820_entry_large_regions() {
        let mut params = boot_params::default();

        // Add large memory region
        let result = add_e820_entry(&mut params, 0x0, 0x40000000, E820_RAM);
        assert!(result.is_ok());
        let first_entry_size = params.e820_table[0].size;
        assert_eq!(first_entry_size, 0x40000000);
    }

    #[test]
    fn add_e820_entry_zero_size() {
        let mut params = boot_params::default();

        // Test adding entry with zero size (should still succeed at entry level)
        let result = add_e820_entry(&mut params, 0x0, 0x0, E820_RAM);
        assert!(result.is_ok());
        let first_entry_size = params.e820_table[0].size;
        assert_eq!(first_entry_size, 0x0);
    }

    /// Helper function to create test guest memory
    fn create_guest_memory(size: usize) -> GuestMemoryMmap {
        GuestMemoryMmap::from_ranges(&[(GuestAddress(0), size)])
            .expect("failed to create guest memory")
    }
}
