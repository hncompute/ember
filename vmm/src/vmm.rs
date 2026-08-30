use anyhow::{Context, Result};
use kvm_ioctls::{Kvm, VcpuFd, VmFd};
use vm_memory::GuestMemoryMmap;

use crate::{arch::DEFAULT_KERNEL_CMDLINE, devices::PortIODeviceManager};

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

    pub fn init(&mut self) -> Result<()> {
        // Call ioctl underneath
        let vcpu = self.vm.create_vcpu(0).context("failed to create vcpu")?;

        crate::arch::vcpu::init_cpu_id(&self.kvm, &vcpu)?;

        // TODO: Init model specific registers (msrs?) and long mode?

        crate::arch::regs::init_regs(&vcpu, crate::arch::layout::KERNEL_START_ADDRESS)?;

        // Floating-point unit, math coprocessor?
        crate::arch::regs::init_fpu(&vcpu)?;
        crate::arch::regs::init_sregs(&self.guest_mem, &vcpu)?;

        self.vcpu = Some(vcpu);

        Ok(())
    }

    pub fn load_image(&self, boot_src_cfg: &crate::arch::BootSourceConfig) -> Result<()> {
        crate::arch::system::load_kernel(&boot_src_cfg.kernel_image_path, &self.guest_mem)
            .context("failed to load kernel")?;

        let initramfs = match &boot_src_cfg.initramfs_path {
            Some(p) => Some(
                crate::arch::system::load_initramfs(p, &self.guest_mem)
                    .context("failed to load initramfs")?,
            ),
            None => None,
        };

        let (cmdline_addr, cmdline_size) =
            crate::arch::system::load_boot_cmdline(&boot_src_cfg.boot_args, &self.guest_mem)
                .context("failed to load boot cmdline")?;

        crate::arch::system::configure_system(
            &self.guest_mem,
            cmdline_addr,
            cmdline_size,
            &initramfs,
        )?;

        Ok(())
    }
}
