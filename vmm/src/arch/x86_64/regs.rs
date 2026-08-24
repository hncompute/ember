use anyhow::{Context, Ok, Result};
use kvm_bindings::{kvm_fpu, kvm_regs, kvm_sregs};
use kvm_ioctls::VcpuFd;
use vm_memory::{Address, Bytes, GuestAddress, GuestMemoryBackend, GuestMemoryMmap};

use crate::arch::gdt::{gdt_entry, kvm_segment_from_gdt};

const PML4_START: u64 = 0x9000;
const PDP_START: u64 = 0xa000;
const PD_START: u64 = 0xb000;

const BOOT_GDT_MAX: usize = 4;

const BOOT_GDT_OFFSET: u64 = 0x500;
const BOOT_IDT_OFFSET: u64 = 0x520;

const BOOT_GDT_ENTRY_FLAGS_CODE: u16 = 0xa09b;
const BOOT_GDT_ENTRY_FLAGS_DATA: u16 = 0xc093;
const BOOT_GDT_ENTRY_FLAGS_TSS: u16 = 0x008b;

/// Activate long mode
const EFER_LMA: u64 = 0x400;
/// Enable long mode
const EFER_LME: u64 = 0x100;

const X86_CR0_PE: u64 = 0x1;
/// Paging enable
const X86_CR0_PG: u64 = 0x8000_0000;
/// Page enabled and active
const X86_CR4_PAE: u64 = 0x20;

const KERNEL_PAGING_SIZE: u64 = 0x200000;
const PDE64_PRESENT: u64 = 1;
const PDE64_RW: u64 = 1 << 1;
const PDE64_PS: u64 = 1 << 7;

const PROT_R: u64 = 1;
const PROT_W: u64 = 2;
const PROT_RW: u64 = PROT_R | PROT_W;

pub fn init_regs(vcpu: &VcpuFd, boot_ip: u64) -> Result<()> {
    let regs = kvm_regs {
        rflags: 2,
        rip: boot_ip,
        // rbp saves a snapshot of rsp
        // so when rsp moves, local vars and function params are still accessible from rbp
        rsp: super::layout::BOOT_STACK_POINTER,
        rbp: super::layout::BOOT_STACK_POINTER,
        rsi: super::layout::ZERO_PAGE_START,
        ..Default::default()
    };

    vcpu.set_regs(&regs).context("failed to set regs")?;

    Ok(())
}

pub fn init_fpu(vcpu: &VcpuFd) -> Result<()> {
    // Subnormal? floating-point numbers are supported
    // rather than being replaced with zeroes
    let fpu = kvm_fpu {
        fcw: 0x37f,    // Floating-point Control Word (legacy stack-based x87 FPU)
        mxcsr: 0x1f80, // SSE/AVX FPU (SIMD)
        ..Default::default()
    };

    vcpu.set_fpu(&fpu).context("failed to set fpu")?;

    Ok(())
}

pub fn init_sregs(guest_mem: &GuestMemoryMmap, vcpu: &VcpuFd) -> Result<()> {
    let mut sregs = vcpu.get_sregs().context("failed to get sregs")?;

    configure_segments_and_sregs(guest_mem, &mut sregs)
        .context("failed to configure segments and sregs")?;

    setup_page_tables(guest_mem, &mut sregs).context("failed to setup page tables")?;

    vcpu.set_sregs(&sregs).context("failed to set sregs")?;

    Ok(())
}

fn configure_segments_and_sregs(guest_mem: &GuestMemoryMmap, sregs: &mut kvm_sregs) -> Result<()> {
    // Global Descriptor Table
    let gdt_table: [u64; BOOT_GDT_MAX] = [
        gdt_entry(0, 0, 0),
        gdt_entry(BOOT_GDT_ENTRY_FLAGS_CODE, 0, 0xfffff),
        gdt_entry(BOOT_GDT_ENTRY_FLAGS_DATA, 0, 0xfffff),
        // TSS (Task State Segment for context switching and privilege changes)
        gdt_entry(BOOT_GDT_ENTRY_FLAGS_TSS, 0, 0xfffff),
    ];

    let code_seg = kvm_segment_from_gdt(gdt_table[1], 1);
    let data_seg = kvm_segment_from_gdt(gdt_table[2], 2);
    let tss_seg = kvm_segment_from_gdt(gdt_table[3], 3);

    // Write segments
    write_gdt_table(&gdt_table[..], guest_mem)?;
    sregs.gdt.base = BOOT_GDT_OFFSET;
    sregs.gdt.limit = u16::try_from(std::mem::size_of_val(&gdt_table))? - 1;

    // Interrupt descriptor table
    write_idt_value(0, guest_mem)?;
    sregs.idt.base = BOOT_IDT_OFFSET;
    sregs.idt.limit = u16::try_from(std::mem::size_of::<u64>())? - 1;

    sregs.cs = code_seg;
    sregs.ds = data_seg;
    sregs.es = data_seg;
    sregs.fs = data_seg;
    sregs.gs = data_seg;
    sregs.ss = data_seg;
    sregs.tr = tss_seg;
    sregs.cr0 |= X86_CR0_PE; // Protection enable
    sregs.efer |= EFER_LME | EFER_LMA; // Enable + Active long mode

    Ok(())
}

fn write_gdt_table(table: &[u64], guest_mem: &GuestMemoryMmap) -> Result<()> {
    let boot_gdt_addr = GuestAddress(BOOT_GDT_OFFSET);

    for (index, entry) in table.iter().enumerate() {
        let addr = guest_mem
            .checked_offset(boot_gdt_addr, index * std::mem::size_of::<u64>())
            .ok_or(anyhow::anyhow!("failed to write GDT"))?;

        // Write object to container at address
        guest_mem
            .write_obj(*entry, addr)
            .context("failed to write GDT entry")?;
    }

    Ok(())
}

fn write_idt_value(val: u64, guest_mem: &GuestMemoryMmap) -> Result<()> {
    let boot_idt_addr = GuestAddress(BOOT_IDT_OFFSET);

    guest_mem
        .write_obj(val, boot_idt_addr)
        .context("failed to write IDT address")?;

    Ok(())
}

fn setup_page_tables(guest_mem: &GuestMemoryMmap, sregs: &mut kvm_sregs) -> Result<()> {
    let boot_pml4_addr = GuestAddress(PML4_START);
    let boot_pdp_addr = GuestAddress(PDP_START);
    let boot_pd_addr = GuestAddress(PD_START);

    // Entry covering virtual address 0...512GB
    guest_mem
        // Set up prot bit?
        .write_obj(boot_pdp_addr.raw_value() | PROT_RW, boot_pml4_addr)
        .context("failed to write PML4 address")?;

    // Entry covering virtual address 0..1GB
    guest_mem
        .write_obj(boot_pd_addr.raw_value() | PROT_RW, boot_pdp_addr)
        .context("failed to write PDPTE address")?;

    // 512 2MB entries (large pages? each entry is88 bytes) covering 0..1GB
    // assuming CPU supports 2MB pages (check pse in /proc/cpuinfo)
    for i in 0..512 {
        let page_addr = i * KERNEL_PAGING_SIZE;
        let flags = PDE64_PRESENT | PDE64_RW | PDE64_PS;
        let entry = page_addr | flags;

        guest_mem
            // Generate 2 MiB size page and enable present + writable + large page size flag (0x83)
            .write_obj(entry, boot_pd_addr.unchecked_add(i * 8))
            .context("failed to write PD address")?;
    }

    sregs.cr3 = boot_pml4_addr.raw_value();
    sregs.cr4 |= X86_CR4_PAE;
    sregs.cr0 |= X86_CR0_PG;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_gdt_entries_to_guest_memory() {
        let memory = GuestMemoryMmap::from_ranges(&[(GuestAddress(0), 0x10_000)]).unwrap();

        let table = [0x1111_1111_1111_1111, 0x2222_2222_2222_2222];

        write_gdt_table(&table, &memory).unwrap();

        let first: u64 = memory.read_obj(GuestAddress(BOOT_GDT_OFFSET)).unwrap();

        let second: u64 = memory.read_obj(GuestAddress(BOOT_GDT_OFFSET + 8)).unwrap();

        assert_eq!(first, table[0]);
        assert_eq!(second, table[1]);
    }

    #[test]
    fn setup_identity_mapped_page_tables() {
        let memory = GuestMemoryMmap::from_ranges(&[(GuestAddress(0), 0x20_000)]).unwrap();

        let mut sregs = kvm_sregs::default();

        setup_page_tables(&memory, &mut sregs).unwrap();

        let pml4_entry: u64 = memory.read_obj(GuestAddress(PML4_START)).unwrap();
        let pdp_entry: u64 = memory.read_obj(GuestAddress(PDP_START)).unwrap();
        let first_pd_entry: u64 = memory.read_obj(GuestAddress(PD_START)).unwrap();
        let second_pd_entry: u64 = memory.read_obj(GuestAddress(PD_START + 8)).unwrap();

        assert_eq!(pml4_entry, PDP_START | 0x03);
        assert_eq!(pdp_entry, PD_START | 0x03);
        assert_eq!(first_pd_entry, 0x83);
        assert_eq!(second_pd_entry, (1 << 21) | 0x83);

        assert_eq!(sregs.cr3, PML4_START);
        assert_ne!(sregs.cr4 & X86_CR4_PAE, 0);
        assert_ne!(sregs.cr0 & X86_CR0_PG, 0);
    }
}
