use anyhow::{Context, Result};
use kvm_ioctls::{Kvm, VcpuFd, VmFd};
use vm_memory::GuestMemoryMmap;

use crate::devices::PortIODeviceManager;

pub struct Vmm {
    pub kvm: Kvm,
    pub vm: VmFd,
    pub guest_mem: GuestMemoryMmap,
    pub vcpu: Option<VcpuFd>,
    pub pio_device_manager: Option<PortIODeviceManager>,
}

impl Vmm {
    pub fn new(ram_size: u64) -> Result<Vmm> {
        let kvm = Kvm::new().context("failed to initialize kvm")?;
        let vm = kvm.create_vm().context("failed to create vm")?;

        crate::arch::irq::init_irqchip(&vm).context("failed to initialize irq chip")?;

        let guest_mem = crate::arch::memory::create_guest_memory(&vm, ram_size)?;
        Ok(Vmm {
            kvm,
            vm,
            guest_mem,
            vcpu: None,
            pio_device_manager: None,
        })
    }
}
