use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::fmt::Formatter;

use std::fmt;

use std::fmt::Debug;
/// Represent an address in the guest's memory
#[derive(Clone, Copy, Deserialize, Serialize)]
pub struct GuestAddress(pub u64);

impl Debug for GuestAddress {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "GuestAddress({:#018x})", self.0)
    }
}

impl PartialEq for GuestAddress {
    fn eq(&self, other: &GuestAddress) -> bool {
        self.0 == other.0
    }
}
impl Eq for GuestAddress {}

/// Comparison always produces a result e.g., Less/Equal/Greater
impl Ord for GuestAddress {
    fn cmp(&self, other: &GuestAddress) -> Ordering {
        self.0.cmp(&other.0)
    }
}

/// Comparison may NOT always produce a result like NaN !== NaN
impl PartialOrd for GuestAddress {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl GuestAddress {
    /// Return the addressas u64 offset from 0x0
    pub fn offset(self) -> u64 {
        self.0
    }
}
