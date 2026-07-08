use std::{
    ffi::c_void,
    mem,
    ptr::{self, NonNull},
};

use kvm_ioctls::{Kvm, VcpuExit};
use nix::sys::mman;

// Start of memory region, one page in
const GUEST_PHYS_ADDR: u64 = 0x1000;

// 256 MiB
const GUEST_SIZE: usize = 256 << 20;

// Mark the start of Global Descriptor Table (separate memory between application and the OS)
const GDT_OFFSET: usize = 0x0;

// Page Map Level 4 (top-level table) with 512 entries (country),
// each one pointing to a Page Directory Pointer Table ()
// which it self points to 512 Page Directory
const PML4_OFFSET: usize = 0x1000;
const PAGE_TABLE_SIZE: usize = 0x1000; // 521 Kb?
const PAGE_SIZE: usize = 1 << 21;
// Mark the start of code segment? After the page tables
const CODE_OFFSET: usize = PML4_OFFSET + 3 * PAGE_TABLE_SIZE;

/* Guest Memory Layout:
 *
 *  GUEST_PHYS_ADDR --> +-------------------------+
 *                      |        Available        |
 *    + GDT_OFFSET  --> +-------------------------+
 *                      | Global Descriptor Table |
 *    + PML4_OFFSET --> +-------------------------+
 *                      |                         |
 *                      |       Page Tables       |
 *                      |                         |
 *    + CODE_OFFSET --> +-------------------------+ <-- %rip
 *                      |                         |
 *                      |          Code           |
 *                      |                         |
 *                      +-------------------------+
 *                      |                         |
 *                      |                         |
 *                      |        Available        |
 *                      |                         |
 *                      |                         |
 *                      +-------------------------+ <-- %rsp
 *                      |                         |
 *                      |          Stack          |
 *                      |                         |
 *     + GUEST_SIZE --> +-------------------------+ <-- %rbp
 */

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

        // Long mode?
        // copy code to guest memory and point rip at it?
        me.setup_long_mode()?;
        // Filling out page tables
        // and global descriptor table
        // and initialize the segment selectors?
        me.map_pages()?;
        // Initialize general purpose registers like rsp, rbp
        me.init_registers()?;

        // TODO: At this point we are still in real mode, no BIOS, firmware, bootloader

        Ok(me)
    }

    // Write guest code to allocated memory non-overlappingly
    pub fn write_guest_code(&mut self, code: &[u8]) {
        unsafe {
            // Assuming two memory regions do not overlap, thus unsafe
            // and under the hood this uses whatever copy techniques it wants for example SIMD
            ptr::copy_nonoverlapping(
                code.as_ptr(),
                self.guest_mem.as_ptr().add(CODE_OFFSET) as *mut _,
                code.len(),
            );
        }
    }

    // Kick off the execution loop of the guest and listen to I/O exits
    pub fn run_with_io_handler<F>(&mut self, mut io_handler: F) -> anyhow::Result<()>
    where
        F: FnMut(u16, &[u8]),
    {
        loop {
            // Implicit multiplexer here like Go?
            match self.vcpu.run()? {
                VcpuExit::IoOut(port, data) => io_handler(port, data),
                exit => {
                    return Err(anyhow::anyhow!("Unhandled exit reason: {:?}", exit));
                }
            }
        }
    }

    fn setup_long_mode(&mut self) -> anyhow::Result<()> {
        // Get current special registers (monitor specific tasks like tracking the next instruction)
        let mut sregs = self.vcpu.get_sregs()?;

        // Set up GDT for long mode (64-bit OS can access 64-bit instructions and registers)
        sregs.gdt.base = GUEST_PHYS_ADDR + GDT_OFFSET as u64;
        // 3 entries * 8 bytes -1

        // Set the last valid byte offset into the GDT
        sregs.gdt.limit = 23;

        // Write GDT entries to guest memory
        // NOTE: VM is Ring-0 (highest-privileged CPU execution level aka kernel mode) only payload
        // unsafe because we are dereferencing a raw pointer (no Rust aliasing, validity or
        // borrow-checking)
        unsafe {
            let gdt_ptr = self
                .guest_mem
                .as_ptr()
                // Add unsigned offset to pointer
                .add(GDT_OFFSET) as *mut u64;

            // Initialize the stack?
            // Null descriptor? 1st entry
            *gdt_ptr.add(0) = 0x0000000000000000;
            // Code segment (64-bit, executable, present)
            *gdt_ptr.add(1) = 0x00209A0000000000;
            // Data segment (64-bit, writable, present)
            *gdt_ptr.add(2) = 0x0000920000000000;
        }

        // Configure code segment for long mode

        // Base address (ignore in long mode?)
        sregs.cs.base = 0;
        sregs.cs.limit = 0xffffffff;

        // Select with GDT entry to use (index 1 code segment)
        sregs.cs.selector = 1 << 3;
        // Mark segment present
        sregs.cs.present = 1;
        // Code execute/read, accessed
        sregs.cs.type_ = 11;
        // Descriptor privilege level (kernel ring 0)
        sregs.cs.dpl = 0;
        sregs.cs.db = 0;
        // Descriptor type: 1 = Code
        sregs.cs.s = 1;
        // Long mode activate
        sregs.cs.l = 1;
        // Granularity = 4 KiB units (ignored in long mode)
        // Effective limit is 4 GiB
        sregs.cs.g = 1;

        // 32-bit segment (ignored in 64-bit mode)
        sregs.ds.base = 0;
        sregs.ds.limit = 0xffffffff;
        // GDT index
        sregs.ds.selector = 2 << 3;
        sregs.ds.db = 1;
        sregs.ds.present = 1;
        sregs.ds.type_ = 3;
        // Kernel mode
        sregs.ds.dpl = 0;
        // Data segment indicator
        sregs.ds.s = 1;
        sregs.ds.g = 1;

        // Replicate for other segments

        // Extra segment (extension of DS)
        sregs.es = sregs.ds;
        // General-purpose data segment
        sregs.fs = sregs.ds;
        sregs.gs = sregs.ds;
        // Stack segment (local vars, function arguments, return addresses)
        sregs.ss = sregs.ds;

        // Enable long mode

        // Long mode enabled + activated using bitwise OR operator
        sregs.efer |= 0x500;
        // Paging + Protection Enable (switch CPU from real mode to protected mode)
        sregs.cr0 |= 0x80000001;
        // Physical address extension?
        sregs.cr4 |= 0x20;

        self.vcpu.set_sregs(&sregs)?;

        Ok(())
    }

    // Identity map the first 1 GB of guest memory using 2 MB large pages
    // and we EXACTLY match virtual address and physical address
    // 64-bit Virtual Address Layout (4-level paging, 2MB large pages)
    //
    //  63                48 47      39 38      30 29      21 20                0
    // +--------------------+----------+----------+----------+--------------------+
    // |   Unused (16 bits) |  PML4    |  PDPT    |    PD    |  Page Offset       |
    // |   (sign-extended)  | (9 bits) | (9 bits) | (9 bits) |    (21 bits)       |
    // +--------------------+----------+----------+----------+--------------------+
    //
    //  Bit ranges:
    //    [63:48]  Unused          - must mirror bit 47 (canonical address)
    //    [47:39]  PML4 index      - selects entry in PML4 table (0-511)
    //    [38:30]  PDPT index      - selects entry in PDPT table (0-511)
    //    [29:21]  PD index        - selects entry in PD table   (0-511)
    //    [20:0]   Page Offset     - byte offset within the 2MB page (0-2097151)
    //
    //  Translation walk:
    //    %cr3 -> PML4[idx1] -> PDPT[idx2] -> PD[idx3] -> physical 2MB page + offset
    fn map_pages(&mut self) -> anyhow::Result<()> {
        unsafe {
            // Zero out the entire area of memory before use
            // or else the CPU will walk into bogus address and we will get tripple-fault
            ptr::write_bytes(
                self.guest_mem.as_ptr().add(PML4_OFFSET),
                0,
                // 3 table types
                3 * PAGE_TABLE_SIZE,
            );

            let pml4 = self.guest_mem.as_ptr().add(PML4_OFFSET) as *mut u64;
            // Mark present (0x1) + writable (0x2)
            *pml4 = (GUEST_PHYS_ADDR + PML4_OFFSET as u64 + PAGE_TABLE_SIZE as u64) | 0x3;

            // PDPT entry pointing to PD
            let pdpt = self.guest_mem.as_ptr().add(PML4_OFFSET + PAGE_TABLE_SIZE) as *mut u64;

            // Mark present + Writable
            *pdpt = (GUEST_PHYS_ADDR + PML4_OFFSET as u64 + 2 * PAGE_TABLE_SIZE as u64) | 0x3;

            let pd = self
                .guest_mem
                .as_ptr()
                .add(PML4_OFFSET + 2 * PAGE_TABLE_SIZE) as *mut u64;

            // Mark every entry in the PD as Present + Writable + Large Page
            // by filling all 521 entries, each entry is a 2 MB PS=1 huge page?
            (0..512).for_each(|i| *pd.add(i) = (i << 21) as u64 | 0x83);
        }

        // Set CR3 to point to PML4 (its only job as a special-purpose register)
        let mut sregs = self.vcpu.get_sregs()?;

        sregs.cr3 = GUEST_PHYS_ADDR + PML4_OFFSET as u64;

        self.vcpu.set_sregs(&sregs)?;

        Ok(())
    }

    // Initialize the special-purpose registers
    // and copy code into guest (VM) user space
    fn init_registers(&mut self) -> anyhow::Result<()> {
        let mut regs: kvm_bindings::kvm_regs = unsafe { mem::zeroed() };

        // Set up the code segment's entry point and page align the stack
        regs.rip = GUEST_PHYS_ADDR + CODE_OFFSET as u64;
        // Set the rsp at the END of the mapped RAM
        // since x86 stack grows downward, when we populate the stack the resp moves downward
        // and do some page alignment by masking off the bottom 21 bits
        regs.rsp = (GUEST_PHYS_ADDR + GUEST_SIZE as u64) & !(PAGE_SIZE as u64 - 1);
        // At the start rbp == rsp
        // then when we have values in the stack, rsp moves
        // while rbp stays like an anchor for return address
        regs.rbp = (GUEST_PHYS_ADDR + GUEST_SIZE as u64) & !(PAGE_SIZE as u64 - 1);

        // Architecturally defined to always be as 1
        regs.rflags = 1 << 1;

        self.vcpu.set_regs(&regs)?;

        Ok(())
    }
}
