use kvm_bindings::kvm_segment;

/// Constructor for a conventional segment GDT entry.
/// Derived from https://github.com/torvalds/linux/blob/master/arch/x86/include/asm/segment.h
pub fn gdt_entry(flags: u16, base: u32, limit: u32) -> u64 {
    ((u64::from(base) & 0xff00_0000) << (56 - 24))
        | ((u64::from(flags) & 0x0000_f0ff) << 40)
        | ((u64::from(limit) & 0x000f_0000) << (48 - 16))
        | ((u64::from(base) & 0x00ff_ffff) << 16)
        | (u64::from(limit) & 0x0000_ffff)
}

/// Build the KVM struct for SET_SREGS from the kernel bit fields
pub fn kvm_segment_from_gdt(entry: u64, table_index: u8) -> kvm_segment {
    kvm_segment {
        base: get_base(entry),
        limit: get_limit(entry),
        selector: u16::from(table_index * 8), // Each entry is 8 bytes?
        type_: get_type(entry),
        present: get_p(entry),
        dpl: get_dpl(entry),
        db: get_db(entry),
        s: get_s(entry),
        l: get_l(entry),
        g: get_g(entry),
        avl: get_avl(entry),
        padding: 0,
        unusable: match get_p(entry) {
            0 => 1,
            _ => 0,
        },
    }
}

fn get_base(entry: u64) -> u64 {
    (((entry) & 0xFF00_0000_0000_0000) >> 32)
        | (((entry) & 0x0000_00FF_0000_0000) >> 16)
        | (((entry) & 0x0000_0000_FFFF_0000) >> 16)
}

fn get_limit(entry: u64) -> u32 {
    ((((entry) & 0x000F_0000_0000_0000) >> 32) as u32) | (((entry) & 0x0000_0000_0000_FFFF) as u32)
}

fn get_g(entry: u64) -> u8 {
    ((entry & 0x0080_0000_0000_0000) >> 55) as u8
}

fn get_db(entry: u64) -> u8 {
    ((entry & 0x0040_0000_0000_0000) >> 54) as u8
}

fn get_l(entry: u64) -> u8 {
    ((entry & 0x0020_0000_0000_0000) >> 53) as u8
}

fn get_avl(entry: u64) -> u8 {
    ((entry & 0x0010_0000_0000_0000) >> 52) as u8
}

fn get_p(entry: u64) -> u8 {
    ((entry & 0x0000_8000_0000_0000) >> 47) as u8
}

fn get_dpl(entry: u64) -> u8 {
    ((entry & 0x0000_6000_0000_0000) >> 45) as u8
}

fn get_s(entry: u64) -> u8 {
    ((entry & 0x0000_1000_0000_0000) >> 44) as u8
}

fn get_type(entry: u64) -> u8 {
    ((entry & 0x0000_0F00_0000_0000) >> 40) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn null_gdt_entry_is_zero() {
        let entry = gdt_entry(0, 0, 0);

        assert_eq!(entry, 0);
    }

    #[test]
    fn code_gdt_entry_encodes_flags_base_and_limit() {
        let entry = gdt_entry(0xa09b, 0, 0xfffff);

        assert_eq!(entry, 0x00af_9b00_0000_ffff);
    }

    #[test]
    fn gdt_entry_preserves_nonzero_base() {
        let entry = gdt_entry(0, 0x1234_5678, 0);

        // Verify the individual base fields rather than repeating
        // the implementation in the test.
        assert_eq!((entry >> 16) & 0xffff, 0x5678);
        assert_eq!((entry >> 32) & 0xff, 0x34);
        assert_eq!((entry >> 56) & 0xff, 0x12);
    }

    #[test]
    fn kvm_segment_from_gdt_code_segment() {
        // Create a GDT entry for a code segment
        // flags: 0xa09b (code segment, present, ring 0, 64-bit)
        let entry = gdt_entry(0xa09b, 0, 0xfffff);
        let table_index = 1;

        let segment = kvm_segment_from_gdt(entry, table_index);

        // Verify the segment properties
        assert_eq!(segment.selector, 8); // table_index * 8 = 1 * 8
        assert_eq!(segment.present, 1); // Present bit should be set
        assert_eq!(segment.dpl, 0); // Privilege level 0 (ring 0)
        assert_eq!(segment.s, 1); // Descriptor type (1 = code/data)
        assert_eq!(segment.l, 1); // 64-bit mode
        assert_eq!(segment.db, 0); // Not used in 64-bit mode
        assert_eq!(segment.unusable, 0); // Present, so not unusable
    }

    #[test]
    fn kvm_segment_from_gdt_data_segment() {
        // Create a GDT entry for a data segment
        // flags: 0xc093 (data segment, present, ring 0)
        let entry = gdt_entry(0xc093, 0, 0xfffff);
        let table_index = 2;

        let segment = kvm_segment_from_gdt(entry, table_index);

        assert_eq!(segment.selector, 16); // table_index * 8 = 2 * 8
        assert_eq!(segment.present, 1); // Present bit should be set
        assert_eq!(segment.dpl, 0); // Ring 0
        assert_eq!(segment.s, 1); // Descriptor type
        assert_eq!(segment.unusable, 0); // Not unusable
    }

    #[test]
    fn kvm_segment_from_gdt_null_segment() {
        // Create a NULL GDT entry (all zeros)
        let entry = gdt_entry(0, 0, 0);
        let table_index = 0;

        let segment = kvm_segment_from_gdt(entry, table_index);

        assert_eq!(segment.selector, 0);
        assert_eq!(segment.present, 0); // Not present
        assert_eq!(segment.unusable, 1); // Marked as unusable
        assert_eq!(segment.base, 0);
        assert_eq!(segment.limit, 0);
    }

    #[test]
    fn kvm_segment_from_gdt_selector_calculation() {
        // Test that the selector is correctly calculated from table index
        for idx in 0..4 {
            let entry = gdt_entry(0, 0, 0);
            let segment = kvm_segment_from_gdt(entry, idx);
            assert_eq!(segment.selector, u16::from(idx) * 8);
        }
    }
}
