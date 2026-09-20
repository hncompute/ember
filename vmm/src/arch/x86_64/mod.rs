pub mod gdt;
pub mod irq;
pub mod layout;
pub mod memory;
pub mod regs;
pub mod system;
pub mod vcpu;

pub const DEFAULT_KERNEL_CMDLINE: &str =
    "console=ttyS0 noapic noacpi reboot=k panic=1 pci=off nomodule";


// TODO: mod vgpu?
