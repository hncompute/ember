use std::{ffi::c_void, ptr::NonNull};

use kvm_ioctls::Kvm;
use nix::sys::mman;

// Start of memory region, one page in
const GUEST_PHYS_ADDR: u64 = 0x1000;

// 256 MiB
const GUEST_SIZE: usize = 256 << 20;

pub struct VM {
    // Shout ioctl to KVM until our program can hold vCPU?
    vcpu: kvm_ioctls::VcpuFd,
    // Trick the VM as this is the real physical RAM
    // equivalent of C void type
    // host-side back buffer for 256 MiB of RAM
    guest_mem: NonNull<c_void>,
}

impl VM {
    pub fn new() -> anyhow::Result<Self> {
        // Open KVM
        // so the ? is to unwrap the Result
        let kvm = Kvm::new()?;

        // Create VM
        let vm = kvm.create_vm()?;

        // Create vCPU
        let vcpu = vm.create_vcpu(0)?;

        // Allocate guest memory with a buffer (physical RAM of host)
        // using unsafe == "just trust me" with power to call functions in C/C++
        // while Rust usualy safety checks still apply
        let guest_mem = unsafe {
            mman::mmap_anonymous(
                None,
                // Perform conversion
                GUEST_SIZE.try_into()?,
                // Perms for the VM with the RAM buffer
                mman::ProtFlags::PROT_READ | mman::ProtFlags::PROT_WRITE,
                mman::MapFlags::MAP_PRIVATE | mman::MapFlags::MAP_ANONYMOUS,
            )?
        };

        // Set up memory region
        let mem_region = kvm_bindings::kvm_userspace_memory_region {
            guest_phys_addr: GUEST_PHYS_ADDR,
            // TODO: Best practice for conversion?
            memory_size: GUEST_SIZE as u64,
            userspace_addr: guest_mem.as_ptr() as u64,
            // Fill other arguments as usual?
            ..Default::default()
        };

        // Wire the mapped region for our hypervisor virtual address space 
        // to the guest's physical address space
        unsafe { vm.set_user_memory_region(mem_region)? }

        // KVM hands our hypervisor a vCPU
        let mut me = Self { vcpu, guest_mem };

        // TODO: At this point we are still in real mode, no BIOS, firmware, bootloader

        Ok(me)
    }
}

