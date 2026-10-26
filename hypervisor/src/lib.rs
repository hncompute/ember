pub use VmX8664 as VmArch;
pub use base::Result;

/// Thread-safe wrapper for VM to get/set its state
/// NOTE: Send and Sync are unsafe. Vm type is entirely of type Send and Sync.
pub trait Vm: Send + Sync {
    /// Make a shadow clone of the fd of this VM
    fn try_clone_descriptor(&self) -> Result<SafeDescriptor>;
}
