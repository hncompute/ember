use anyhow::{Context, Result};
use kvm_bindings::{KVM_MEM_LOG_DIRTY_PAGES, kvm_userspace_memory_region};
use kvm_ioctls::VmFd;
use vm_memory::{GuestAddress, GuestMemoryBackend, GuestMemoryMmap};

const RAM_BASE: u64 = 0;

pub fn create_guest_memory(vm: &VmFd, ram_size: u64) -> Result<GuestMemoryMmap> {
    // Start of guest address region
    let guest_addr = GuestAddress(RAM_BASE);

    // Allocate anonymous memory for guest memory region(s)
    // Syntax: Turbofish syntax to explicitly set type argument (empty tuple means no metadata)
    // vm-memory handles the mmapp operation
    let guest_mem = GuestMemoryMmap::<()>::from_ranges(&[(guest_addr, ram_size as usize)])
        .context("failed to create guest memory")?;

    // Where guest RAM allocation actually resides in VMM process
    let host_addr = guest_mem
        .get_host_address(guest_addr)
        .context("failed to get host address")?;

    let mem_region = kvm_userspace_memory_region {
        slot: 0,
        guest_phys_addr: RAM_BASE,
        memory_size: ram_size,
        userspace_addr: host_addr as u64,
        // Used here to track guest memory changes?
        // Write-protect PT leaf entries, and 1st write on each page/entry, KVM records the page in
        // the dirty bitmap then makes the mapping? writable.
        // We can also fetch + clear the dirty log
        flags: KVM_MEM_LOG_DIRTY_PAGES,
    };

    unsafe {
        vm.set_user_memory_region(mem_region)
            .context("failed to set user memory region")?;
    }

    vm.set_tss_address(crate::arch::layout::KVM_TSS_ADDRESS)
        .context("failed to set tss address")?;

    Ok(guest_mem)
}
