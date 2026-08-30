
#[derive(Debug, Default)]
pub struct BootSourceConfig {
    pub kernel_image_path: String,
    pub initramfs_path: Option<String>,
    // DEFAULT_KERNEL_CMDLINE is used if no input
    pub boot_args: Option<String>,
}

/// Type for passing information about initramfs into the guest memory
#[derive(Debug)]
pub struct InitramfsConfig {
    pub address: vm_memory::GuestAddress,
    pub size: usize,
}
