pub mod sdt;

fn generate_checksum(data: &[u8]) -> u8 {
    let sum = data.iter().fold(0u8, |acc, x| acc.wrapping_add(*x));
    let checksum = (255 - sum).wrapping_add(1);
    checksum
}
