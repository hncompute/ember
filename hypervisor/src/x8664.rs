use std::{collections::BTreeMap, sync::Arc};

use crate::{CpuIds, Hypervisor, Vcpu, Vm};
use base::Result;
use memory::GuestAddress;
use serde::{Deserialize, Serialize};
/// Wrapper to get/set state of a x8664 VM
pub trait VmX8664: Vm {
    /// Gets the `HypervisorX86_64` that created this VM.
    fn get_hypervisor(&self) -> &dyn HypervisorX8664;

    /// Create a Vcpu with the specified Vcpu ID.
    fn create_vcpu(&self, id: usize) -> Result<Arc<dyn VcpuX8664>>;

    /// Sets the address of the three-page region in the VM's address space.
    fn set_tss_addr(&self, addr: GuestAddress) -> Result<()>;

    /// Sets the address of a one-page region in the VM's address space.
    fn set_identity_map_addr(&self, addr: GuestAddress) -> Result<()>;
}

/// Manage cpuids for x8664 hypervisor e.g., KVM and checking its capabilities
pub trait HypervisorX8664: Hypervisor {
    /// Get the system supported CPUID values.
    fn get_supported_cpuid(&self) -> Result<CpuIds>;
}

/// Initial state for x86_64 VCPUs.
#[derive(Clone, Default)]
pub struct VcpuInitX8664 {
    /// General-purpose registers.
    pub regs: Regs,
    /// Special registers.
    pub sregs: Sregs,
    /// Floating-point registers.
    pub fpu: Fpu,
    /// Machine-specific registers.
    pub msrs: BTreeMap<u32, u64>,
}

/// Wrapper around creating and using a vCPU on x8664
pub trait VcpuX8664: Vcpu {}

/// State of a VCPU's general purpose registers.
#[repr(C)]
#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct Regs {
    pub rax: u64,
    pub rbx: u64,
    pub rcx: u64,
    pub rdx: u64,
    pub rsi: u64,
    pub rdi: u64,
    pub rsp: u64,
    pub rbp: u64,
    pub r8: u64,
    pub r9: u64,
    pub r10: u64,
    pub r11: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
    pub rip: u64,
    pub rflags: u64,
}

impl Default for Regs {
    fn default() -> Self {
        Regs {
            rax: 0,
            rbx: 0,
            rcx: 0,
            rdx: 0,
            rsi: 0,
            rdi: 0,
            rsp: 0,
            rbp: 0,
            r8: 0,
            r9: 0,
            r10: 0,
            r11: 0,
            r12: 0,
            r13: 0,
            r14: 0,
            r15: 0,
            rip: 0xfff0, // Reset vector.
            rflags: 0x2, // Bit 1 (0x2) is always 1.
        }
    }
}

/// State of a VCPU's special registers.
#[repr(C)]
#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct Sregs {
    pub cs: Segment,
    pub ds: Segment,
    pub es: Segment,
    pub fs: Segment,
    pub gs: Segment,
    pub ss: Segment,
    pub tr: Segment,
    pub gdt: DescriptorTable,
    pub ldt: Segment,
    pub idt: DescriptorTable,
    pub cr0: u64,
    pub cr2: u64,
    pub cr3: u64,
    pub cr4: u64,
    pub cr8: u64,
    pub efer: u64,
}

impl Default for Sregs {
    fn default() -> Self {
        // Intel SDM Vol. 3A, 3.4.5.1 ("Code- and Data-Segment Descriptor Types")
        const SEG_TYPE_DATA: u8 = 0b0000;
        const SEG_TYPE_DATA_WRITABLE: u8 = 0b0010;

        const SEG_TYPE_CODE: u8 = 0b1000;
        const SEG_TYPE_CODE_READABLE: u8 = 0b0010;

        const SEG_TYPE_ACCESSED: u8 = 0b0001;

        // Intel SDM Vol. 3A, 3.4.5 ("Segment Descriptors")
        const SEG_S_SYSTEM: u8 = 0; // System segment.
        const SEG_S_CODE_OR_DATA: u8 = 1; // Data/code segment.

        const SEG_LIMIT: u32 = 0xffff; // 65535
        const TABLE_LIMIT: u16 = 0xffff; // 65535

        // 16-bit real-mode code segment (reset vector).
        let code_seg = Segment {
            base: 0xffff0000,
            limit_bytes: SEG_LIMIT,
            selector: 0xf000,
            type_: SEG_TYPE_CODE | SEG_TYPE_CODE_READABLE | SEG_TYPE_ACCESSED, // 11
            present: 1,
            s: SEG_S_CODE_OR_DATA,
            ..Default::default()
        };

        // 16-bit real-mode data segment.
        let data_seg = Segment {
            base: 0,
            limit_bytes: SEG_LIMIT,
            selector: 0,
            type_: SEG_TYPE_DATA | SEG_TYPE_DATA_WRITABLE | SEG_TYPE_ACCESSED, // 3
            present: 1,
            s: SEG_S_CODE_OR_DATA,
            ..Default::default()
        };

        // 16-bit TSS segment.
        let task_seg = Segment {
            base: 0,
            limit_bytes: SEG_LIMIT,
            selector: 0,
            type_: SEG_TYPE_CODE | SEG_TYPE_CODE_READABLE | SEG_TYPE_ACCESSED, // 11
            present: 1,
            s: SEG_S_SYSTEM,
            ..Default::default()
        };

        // Local descriptor table.
        let ldt = Segment {
            base: 0,
            limit_bytes: SEG_LIMIT,
            selector: 0,
            type_: SEG_TYPE_DATA | SEG_TYPE_DATA_WRITABLE, // 2
            present: 1,
            s: SEG_S_SYSTEM,
            ..Default::default()
        };

        // Global descriptor table.
        let gdt = DescriptorTable {
            base: 0,
            limit: TABLE_LIMIT,
        };

        // Interrupt descriptor table.
        let idt = DescriptorTable {
            base: 0,
            limit: TABLE_LIMIT,
        };

        let cr0 = (1 << 4) // CR0.ET (reserved, always 1)
                | (1 << 30); // CR0.CD (cache disable)

        Sregs {
            cs: code_seg,
            ds: data_seg,
            es: data_seg,
            fs: data_seg,
            gs: data_seg,
            ss: data_seg,
            tr: task_seg,
            ldt,
            gdt,
            idt,
            cr0,
            cr2: 0,
            cr3: 0,
            cr4: 0,
            cr8: 0,
            efer: 0,
        }
    }
}

/// State of a memory segment.
#[repr(C)]
#[derive(Debug, Default, Copy, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Segment {
    pub base: u64,
    /// Limit of the segment - always in bytes, regardless of granularity (`g`) field.
    pub limit_bytes: u32,
    pub selector: u16,
    pub type_: u8,
    pub present: u8,
    pub dpl: u8,
    pub db: u8,
    pub s: u8,
    pub l: u8,
    pub g: u8,
    pub avl: u8,
}

/// State of a global descriptor table or interrupt descriptor table.
#[repr(C)]
#[derive(Debug, Default, Copy, Clone, Serialize, Deserialize)]
pub struct DescriptorTable {
    pub base: u64,
    pub limit: u16,
}

/// State of a VCPU's floating point unit.
#[repr(C)]
#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct Fpu {
    /// Subset of x86 instruction set, including eight 80-bit x87 registers
    pub fpr: [FpuReg; 8],
    /// Floating-point control word
    pub fcw: u16,
    /// Floating-point status word
    pub fsw: u16,
    /// Tag word to tell if each x87 register is occupied or emptied
    pub ftwx: u8,
    /// Saved instruction opcode
    pub last_opcode: u16,
    /// Saved instruction pointer
    pub last_ip: u64,
    /// Saved data pointer
    pub last_dp: u64,
    /// XMM0 to XMM15 stored as raw bytes
    pub xmm: [[u8; 16usize]; 16usize],
    /// Status register to hold exception flags, masks, rounding controls, super small values.
    pub mxcsr: u32,
}

impl Default for Fpu {
    fn default() -> Self {
        Fpu {
            fpr: Default::default(),
            fcw: 0x37f, // Intel SDM Vol. 1, 13.6
            fsw: 0,
            ftwx: 0,
            last_opcode: 0,
            last_ip: 0,
            last_dp: 0,
            xmm: Default::default(),
            mxcsr: 0x1f80, // Intel SDM Vol. 1, 11.6.4
        }
    }
}

/// x87 80-bit floating point value
// 79       78                 64 63                          0
// +--------+--------------------+----------------------------+
// | sign   | exponent (15 bits) | significand (64 bits)      |
// +--------+--------------------+----------------------------+
//           sign_exp: u16               significand: u64
#[repr(C)]
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct FpuReg {
    /// 64-bit mantissa.
    pub significand: u64,

    /// 15-bit biased exponent and 1-bit sign.
    pub sign_exp: u16,
}
