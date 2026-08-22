/// Kernel command line start address
pub const CMDLINE_START: u64 = 0x20000;
/// Kernel command line maximum size (depend on 32/64-bit)
pub const CMDLINE_MAX_SIZE: usize = 2048;

/// Start of high memory (legacy compatibility?)
pub const KERNEL_START_ADDRESS: u64 = 0x0010_0000; // 1 MB

/// Zero page aka linux kernel bootparams (virtual memory map + user program args)
pub const ZERO_PAGE_START: u64 = 0x7000;
