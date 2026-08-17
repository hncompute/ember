use anyhow::{Context, Result};
use kvm_ioctls::VmFd;
use vm_memory::{GuestAddress, GuestMemoryMmap};

const RAM_BASE: u64 = 0;

pub fn create_guest_memory(vm: &VmFd, ram_size: u64) -> Result<GuestMemoryMmap> {
    // Start of guest address region
    let guest_addr = GuestAddress(RAM_BASE);

    // Allocate anonymous memory for guest memory region(s)
    // Syntax: Turbofish syntax to explicitly set type argument (empty tuple means no metadata)
    // vm-memory handles the mmapp operation
    let guest_mem = GuestMemoryMmap::<()>::from_ranges(&[(guest_addr, ram_size as usize)])
        .context("failed to create guest memory")?;

    Ok(guest_mem)
}
