use zerocopy::{Immutable, IntoBytes};

/// SDT (System Description Table) is a generic format to create various ACPI tables like
/// DSDT/FADT/MADT
#[derive(Clone)]
pub struct SDT {
    data: Vec<u8>,
}

pub const HEADER_LEN: u32 = 36;
const CHECKSUM_OFFSET: usize = 9;

impl SDT {
    /// Set up ACPI table headers
    pub fn new(
        signature: [u8; 4],
        length: u32,
        revision: u8,
        oem_id: [u8; 6],
        oem_table: [u8; 8],
        oem_revision: u32,
        // Length represents length of entire table including headers
    ) -> Self {
        let len: u32 = if length < HEADER_LEN {
            HEADER_LEN
        } else {
            length
        };
        let mut data = Vec::with_capacity(length as usize);
        data.extend_from_slice(&signature); // Clone then append
        data.extend_from_slice(&len.to_le_bytes());
        data.push(revision);
        data.push(0); // Checksum
        data.extend_from_slice(&oem_id);
        data.extend_from_slice(&oem_table);
        data.extend_from_slice(&oem_revision.to_le_bytes());
        data.extend_from_slice(b"HNCL"); // Table creator
        data.extend_from_slice(&0u32.to_le_bytes());

        data.resize(length as usize, 0);
        let mut sdt = SDT { data };

        sdt.update_checksum();
        sdt
    }

    /// Calculate sum of every byte
    fn update_checksum(&mut self) {
        self.data[CHECKSUM_OFFSET] = 0;
        let checksum = super::generate_checksum(self.data.as_slice());
        self.data[CHECKSUM_OFFSET] = checksum;
    }

    fn as_slice(&self) -> &[u8] {
        self.data.as_slice()
    }

    /// Write a value at a given offset
    fn write<T: IntoBytes + Immutable>(&mut self, offset: usize, value: T) {
        let value_len = std::mem::size_of::<T>();
        if (offset + value_len) > self.data.len() {
            return;
        }
        self.data[offset..offset + value_len].copy_from_slice(value.as_bytes());
        self.update_checksum();
    }
}

#[cfg(test)]
mod tests {
    use super::SDT;

    #[test]
    fn test_sdt() {
        // Test immutable headers
        let mut sdt = SDT::new(*b"TEST", 40, 1, *b"EMBERS", *b"TESTTEST", 1);
        let sum: u8 = sdt
            .as_slice()
            .iter()
            .fold(0u8, |acc, x| acc.wrapping_add(*x));
        assert_eq!(sum, 0);
        sdt.write(36, 0x12345678_u32);
        let sum: u8 = sdt
            .as_slice()
            .iter()
            .fold(0u8, |acc, x| acc.wrapping_add(*x));
        assert_eq!(sum, 0)
    }
}
