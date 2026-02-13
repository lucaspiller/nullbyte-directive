//! Memory model primitives and fixed address-space policies.

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

#[cfg(test)]
mod tests {
    use super::{new_address_space, ADDRESS_SPACE_BYTES};

    #[test]
    fn canonical_backing_store_size_is_64kib() {
        let memory = new_address_space();
        assert_eq!(memory.len(), ADDRESS_SPACE_BYTES);
        assert!(memory.iter().all(|byte| *byte == 0));
    }
}
