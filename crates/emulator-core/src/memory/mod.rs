//! Memory model primitives and fixed address-space policies.

use crate::FaultCode;

/// Deterministic fetch/write legality policy helpers.
pub mod access;
/// Fixed memory-region map and address decoder.
pub mod map;

pub use access::{
    validate_fetch_access, validate_mmio_alignment, validate_mmio_width, validate_word_alignment,
    validate_write_access, WORD_ACCESS_BYTES,
};
pub use map::{
    decode_memory_region, MemoryRegion, RegionDescriptor, DIAG_END, DIAG_START,
    FIXED_MEMORY_REGIONS, MMIO_END, MMIO_START, RAM_END, RAM_START, RESERVED_END, RESERVED_START,
    ROM_END, ROM_START,
};

/// Size in bytes of the flat architectural address space (64 KiB).
pub const ADDRESS_SPACE_BYTES: usize = u16::MAX as usize + 1;

/// Allocates a canonical zeroed 64 KiB address-space backing store.
#[must_use]
pub fn new_address_space() -> Box<[u8]> {
    vec![0; ADDRESS_SPACE_BYTES].into_boxed_slice()
}

/// Big-endian read from memory slice at given address (returns u16).
///
/// # Errors
///
/// Returns `FaultCode::IllegalMemoryAccess` if `addr + 1` exceeds slice bounds.
pub fn read_u16_be(slice: &[u8], addr: u16) -> Result<u16, FaultCode> {
    let index = addr as usize;
    if index + 1 >= slice.len() {
        return Err(FaultCode::IllegalMemoryAccess);
    }
    let high = u16::from(slice[index]);
    let low = u16::from(slice[index + 1]);
    Ok((high << 8) | low)
}

/// Big-endian write to memory slice at given address.
///
/// # Errors
///
/// Returns `FaultCode::IllegalMemoryAccess` if `addr + 1` exceeds slice bounds.
pub fn write_u16_be(slice: &mut [u8], addr: u16, value: u16) -> Result<(), FaultCode> {
    let index = addr as usize;
    if index + 1 >= slice.len() {
        return Err(FaultCode::IllegalMemoryAccess);
    }
    slice[index] = (value >> 8) as u8;
    slice[index + 1] = (value & 0xFF) as u8;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{new_address_space, read_u16_be, write_u16_be, FaultCode};

    #[test]
    #[allow(clippy::cast_possible_truncation)]
    fn read_u16_be_at_end_of_memory() {
        let mut memory = new_address_space();
        let last_idx = memory.len() - 2;
        memory[last_idx] = 0xAB;
        memory[last_idx + 1] = 0xCD;
        assert_eq!(read_u16_be(&memory, last_idx as u16), Ok(0xABCD));
    }

    #[test]
    #[allow(clippy::cast_possible_truncation)]
    fn read_u16_be_past_end_fails() {
        let memory = new_address_space();
        let last_addr = (memory.len() - 1) as u16;
        assert_eq!(
            read_u16_be(&memory, last_addr),
            Err(FaultCode::IllegalMemoryAccess)
        );
    }

    #[test]
    fn write_u16_be_at_even_address() {
        let mut memory = new_address_space();
        write_u16_be(&mut memory, 0x00, 0x1234).unwrap();
        assert_eq!(memory[0x00], 0x12);
        assert_eq!(memory[0x01], 0x34);
    }

    #[test]
    fn write_u16_be_at_odd_address() {
        let mut memory = new_address_space();
        write_u16_be(&mut memory, 0x01, 0x1234).unwrap();
        assert_eq!(memory[0x01], 0x12);
        assert_eq!(memory[0x02], 0x34);
    }

    #[test]
    #[allow(clippy::cast_possible_truncation)]
    fn write_u16_be_past_end_fails() {
        let mut memory = new_address_space();
        let last_addr = (memory.len() - 1) as u16;
        assert_eq!(
            write_u16_be(&mut memory, last_addr, 0xABCD),
            Err(FaultCode::IllegalMemoryAccess)
        );
    }

    #[test]
    #[allow(clippy::cast_possible_truncation)]
    fn round_trip_u16_be() {
        let mut memory = new_address_space();
        let test_values = [0x0000, 0x1234, 0xABCD, 0xFFFF];
        for (i, &value) in test_values.iter().enumerate() {
            let addr = (i * 2) as u16;
            write_u16_be(&mut memory, addr, value).unwrap();
            assert_eq!(read_u16_be(&memory, addr), Ok(value));
        }
    }
}
