use anyhow::{Context, Result};
use kvm_ioctls::{Kvm, VcpuExit, VcpuFd, VmFd};
use log::{error, info};
use vm_memory::GuestMemoryMmap;
use vm_superio::Trigger;

use crate::devices::{Bus, EventFdTrigger, PortIODeviceManager, setup_serial_device};

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

    pub fn run(&mut self) -> Result<()> {
        let serial_device = setup_serial_device(std::io::stdin(), std::io::stdout())?;
        let mut pio_device_manager = PortIODeviceManager::new(serial_device.clone())?;
        pio_device_manager.register_devices(&self.vm)?;

        let vcpu_exit_evt = self.start_threaded(pio_device_manager.io_bus.clone())?;

        Ok(())
    }

    fn start_threaded(&mut self, pio_bus: Bus) -> Result<EventFdTrigger> {
        let vcpu = match std::mem::take(&mut self.vcpu) {
            // Take ownership, replace with empty
            Some(vcpu) => vcpu,
            None => return Err(anyhow::anyhow!("vcpu is not initialized")),
        };

        let exit_evt = EventFdTrigger::new();

        // Multiple threads to interact with this FD
        let vcpu_exit_evt = exit_evt.try_clone().context("failed to clone eventfd")?;

        let builder = std::thread::Builder::new();
        let _ = builder
            .name(String::from("vcpu0"))
            .spawn(move || {
                loop {
                    match vcpu.run() {
                        Ok(run) => match run {
                            VcpuExit::IoIn(addr, data) => {
                                pio_bus.read(addr.into(), data);
                            }
                            VcpuExit::IoOut(addr, data) => {
                                pio_bus.write(addr.into(), data);
                            }
                            VcpuExit::MmioRead(_, _) => {
                                info!("mmio read");
                            }
                            VcpuExit::MmioWrite(_, _) => {
                                info!("mmio write");
                            }
                            VcpuExit::Hlt => {
                                info!("KVM_EXIT_HLT");
                                break;
                            }
                            VcpuExit::Shutdown => {
                                error!("KVM_EXIT_SHUTDOWN");
                                break;
                            }
                            VcpuExit::FailEntry(hardware_entry_failure_reason, _cpu) => {
                                error!(
                                    "KVM_EXIT_FAIL_ENTRY: Hardware Failure Reason: 0x{:X}",
                                    hardware_entry_failure_reason
                                );
                                break;
                            }
                            VcpuExit::InternalError => {
                                // TODO: Find how to print suberrors
                                error!("KVM_EXIT_INTERNAL_ERROR");
                                break;
                            }
                            r => {
                                info!("KVM_EXIT: {:?}", r);
                                break;
                            }
                        },

                        Err(e) => {
                            error!("VM run error: {:?}", e);
                            break;
                        }
                    }
                }
                exit_evt.trigger().expect("failed to write to exit_evt");
            })
            .context("failed to spawn vcpu thread");

        Ok(vcpu_exit_evt)
    }
}
