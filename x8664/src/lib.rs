pub struct ArchMemoryLayout {
    // range below 4G (bios, elf segments, initramfs...)
    pcio_mmio_before_32bit: AddressRange,
    pcie_cfg_mmio: AddressRange,
}
