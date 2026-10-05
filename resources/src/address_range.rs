use serde::{Deserialize, Serialize};

#[derive(Copy, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AddressRange {
    pub start: u64,
    pub end: u64,
}
