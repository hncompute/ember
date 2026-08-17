pub mod x86_64;
pub use x86_64::*;

pub mod config;
pub use config::BootSourceConfig;

pub const DEFAULT_KERNEL_CMDLINE: &str =
    "console=ttyS0 noapic noacpi reboot=k panic=1 pci=off nomodule";
