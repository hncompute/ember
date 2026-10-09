use remain::sorted;

#[sorted]
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum MemoryRegionPurpose {
    #[default]
    GuestMemoryRegion,

    /// Area that should be backed by GuestMemory but reported as reserved to the guest
    ReservedMemory,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct MemoryRegionOptions {
    pub purpose: MemoryRegionPurpose,
    pub align: u64,
}

impl MemoryRegionOptions {
    pub fn new() -> MemoryRegionOptions {
        Default::default()
    }

    pub fn purpose(mut self, purpose: MemoryRegionPurpose) -> Self {
        self.purpose = purpose;
        self
    }

    pub fn align(mut self, alignment: u64) -> Self {
        self.align = alignment;
        self
    }
}
