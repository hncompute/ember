use serde::{Deserialize, Serialize};

#[derive(Copy, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AddressRange {
    pub start: u64,
    pub end: u64,
}

impl AddressRange {
    pub const fn from_start_and_end(start: u64, end: u64) -> Self {
        AddressRange { start, end }
    }

    pub const fn from_start_and_size(start: u64, size: u64) -> Option<Self> {
        if size == 0 {
            Some(AddressRange::empty())
        } else if let Some(end) = start.checked_add(size - 1) {
            Some(AddressRange { start, end })
        } else {
            None
        }
    }

    const fn empty() -> Self {
        AddressRange { start: 1, end: 0 }
    }
}
