use base::Result;
use base::SafeDescriptor;
use serde::Deserialize;
use serde::Serialize;
use serde_keyvalue::FromKeyValues;
use std::arch::x86_64::CpuidResult;
use std::path::PathBuf;

#[cfg(target_arch = "x86_64")]
pub mod x8664;

#[cfg(target_arch = "x86_64")]
pub use crate::x8664::*;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, FromKeyValues)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub enum HypervisorKind {
    Kvm { device: Option<PathBuf> },
}

/// Thread-safe wrapper for VM to get/set its state
/// NOTE: Send and Sync are unsafe. Vm type is entirely of type Send and Sync.
pub trait Vm: Send + Sync {
    /// Make a shadow clone of the fd of this VM
    fn try_clone_descriptor(&self) -> Result<SafeDescriptor>;
}

/// A virtual CPU holds a virtualized thread's state like registers and interrupt state.
pub trait Vcpu: std::any::Any + Send + Sync {
    /// Run the vCPU until it exits then return the reason for the exit.
    fn run(&self) -> Result<VcpuExit>;
}

/// Reasons for vCPU exit
#[derive(Debug, Clone, Copy)]
pub enum VcpuExit {
    Exception,
    Debug,
    Hlt,
    InternalError,
    Canceled,
}

/// Trait for checking hypervisor capabilities.
pub trait Hypervisor: Send {
    /// Makes a shallow clone (copy top properties and memory references only) of this `Hypervisor`.
    fn try_clone(&self) -> Result<Self>
    where
        Self: Sized;
}

/// Contain supported feature information of the given processor.
/// This entry can be modified by the hypervisor to pass additional information to the guest
/// kernel. Information will be returned in the eax, ebx, ecx and edx registers by the physical CPU.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CpuId {
    /// Leaf - A category of information requested from cpuid
    pub function: u32,
    /// Subleaf - A record within the chosen category
    pub index: u32,
    /// Preserve flags for KVM
    pub flags: u32,
    pub cpuid: CpuidResult,
}

/// Container for list of cpu id entries
pub struct CpuIds {
    pub cpu_id_entries: Vec<CpuId>,
}
