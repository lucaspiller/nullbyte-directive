//! Fixed architectural memory-region map and decoding helpers.

/// Inclusive start address of ROM region.
pub const ROM_START: u16 = 0x0000;
/// Inclusive end address of ROM region.
pub const ROM_END: u16 = 0x3FFF;
/// Inclusive start address of RAM region.
pub const RAM_START: u16 = 0x4000;
/// Inclusive end address of RAM region.
pub const RAM_END: u16 = 0xDFFF;
/// Inclusive start address of MMIO region.
pub const MMIO_START: u16 = 0xE000;
/// Inclusive end address of MMIO region.
pub const MMIO_END: u16 = 0xEFFF;
/// Inclusive start address of DIAG region.
pub const DIAG_START: u16 = 0xF000;
/// Inclusive end address of DIAG region.
pub const DIAG_END: u16 = 0xF0FF;
/// Inclusive start address of reserved region.
pub const RESERVED_START: u16 = 0xF100;
/// Inclusive end address of reserved region.
pub const RESERVED_END: u16 = 0xFFFF;

/// Canonical fixed-region descriptor for the architectural memory map.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RegionDescriptor {
    /// Region classification.
    pub region: MemoryRegion,
    /// Inclusive start address.
    pub start: u16,
    /// Inclusive end address.
    pub end: u16,
}

/// Region classification for architectural addresses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MemoryRegion {
    /// ROM region (`0x0000..=0x3FFF`).
    Rom,
    /// RAM region (`0x4000..=0xDFFF`).
    Ram,
    /// MMIO region (`0xE000..=0xEFFF`).
    Mmio,
    /// DIAG region (`0xF000..=0xF0FF`).
    Diag,
    /// Reserved region (`0xF100..=0xFFFF`).
    Reserved,
}

impl MemoryRegion {
    /// Returns the inclusive bounds for this region.
    #[must_use]
    pub const fn bounds(self) -> (u16, u16) {
        match self {
            Self::Rom => (ROM_START, ROM_END),
            Self::Ram => (RAM_START, RAM_END),
            Self::Mmio => (MMIO_START, MMIO_END),
            Self::Diag => (DIAG_START, DIAG_END),
            Self::Reserved => (RESERVED_START, RESERVED_END),
        }
    }

    /// Returns `true` when `addr` belongs to this region.
    #[must_use]
    pub const fn contains(self, addr: u16) -> bool {
        let (start, end) = self.bounds();
        addr >= start && addr <= end
    }

    /// Returns the canonical descriptor for this region.
    #[must_use]
    pub const fn descriptor(self) -> RegionDescriptor {
        let (start, end) = self.bounds();
        RegionDescriptor {
            region: self,
            start,
            end,
        }
    }
}

/// Canonical fixed architectural region layout in ascending address order.
pub const FIXED_MEMORY_REGIONS: [RegionDescriptor; 5] = [
    MemoryRegion::Rom.descriptor(),
    MemoryRegion::Ram.descriptor(),
    MemoryRegion::Mmio.descriptor(),
    MemoryRegion::Diag.descriptor(),
    MemoryRegion::Reserved.descriptor(),
];

const _: () = assert_fixed_region_layout();

const fn assert_fixed_region_layout() {
    assert!(
        FIXED_MEMORY_REGIONS.len() == 5,
        "invalid fixed region descriptor count"
    );

    assert!(
        FIXED_MEMORY_REGIONS[0].start == ROM_START && FIXED_MEMORY_REGIONS[0].end == ROM_END,
        "rom bounds must match spec"
    );
    assert!(
        FIXED_MEMORY_REGIONS[1].start == RAM_START && FIXED_MEMORY_REGIONS[1].end == RAM_END,
        "ram bounds must match spec"
    );
    assert!(
        FIXED_MEMORY_REGIONS[2].start == MMIO_START && FIXED_MEMORY_REGIONS[2].end == MMIO_END,
        "mmio bounds must match spec"
    );
    assert!(
        FIXED_MEMORY_REGIONS[3].start == DIAG_START && FIXED_MEMORY_REGIONS[3].end == DIAG_END,
        "diag bounds must match spec"
    );
    assert!(
        FIXED_MEMORY_REGIONS[4].start == RESERVED_START
            && FIXED_MEMORY_REGIONS[4].end == RESERVED_END,
        "reserved bounds must match spec"
    );

    let mut index = 0;
    while index < FIXED_MEMORY_REGIONS.len() {
        let descriptor = FIXED_MEMORY_REGIONS[index];
        assert!(
            descriptor.start <= descriptor.end,
            "region start cannot be greater than end"
        );

        if index > 0 {
            let previous = FIXED_MEMORY_REGIONS[index - 1];
            assert!(
                previous.end.wrapping_add(1) == descriptor.start,
                "fixed regions must be contiguous"
            );
        }

        index += 1;
    }

    assert!(
        FIXED_MEMORY_REGIONS[0].start == 0x0000 && FIXED_MEMORY_REGIONS[4].end == u16::MAX,
        "fixed regions must cover full address space"
    );
}

/// Decodes an architectural 16-bit address into its fixed memory region.
#[must_use]
pub const fn decode_memory_region(addr: u16) -> MemoryRegion {
    match addr {
        ROM_START..=ROM_END => MemoryRegion::Rom,
        RAM_START..=RAM_END => MemoryRegion::Ram,
        MMIO_START..=MMIO_END => MemoryRegion::Mmio,
        DIAG_START..=DIAG_END => MemoryRegion::Diag,
        RESERVED_START..=RESERVED_END => MemoryRegion::Reserved,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        decode_memory_region, MemoryRegion, RegionDescriptor, DIAG_END, DIAG_START,
        FIXED_MEMORY_REGIONS, MMIO_END, MMIO_START, RAM_END, RAM_START, RESERVED_END,
        RESERVED_START, ROM_END, ROM_START,
    };

    #[test]
    fn region_decode_is_correct_at_boundaries() {
        assert_eq!(decode_memory_region(ROM_START), MemoryRegion::Rom);
        assert_eq!(decode_memory_region(ROM_END), MemoryRegion::Rom);

        assert_eq!(decode_memory_region(RAM_START), MemoryRegion::Ram);
        assert_eq!(decode_memory_region(RAM_END), MemoryRegion::Ram);

        assert_eq!(decode_memory_region(MMIO_START), MemoryRegion::Mmio);
        assert_eq!(decode_memory_region(MMIO_END), MemoryRegion::Mmio);

        assert_eq!(decode_memory_region(DIAG_START), MemoryRegion::Diag);
        assert_eq!(decode_memory_region(DIAG_END), MemoryRegion::Diag);

        assert_eq!(decode_memory_region(RESERVED_START), MemoryRegion::Reserved);
        assert_eq!(decode_memory_region(RESERVED_END), MemoryRegion::Reserved);
    }

    #[test]
    fn region_bounds_are_contiguous_and_cover_full_space() {
        assert_eq!(ROM_END.wrapping_add(1), RAM_START);
        assert_eq!(RAM_END.wrapping_add(1), MMIO_START);
        assert_eq!(MMIO_END.wrapping_add(1), DIAG_START);
        assert_eq!(DIAG_END.wrapping_add(1), RESERVED_START);
        assert_eq!(RESERVED_END, u16::MAX);
    }

    #[test]
    fn contains_matches_decoder_for_all_addresses() {
        for addr in 0_u16..=u16::MAX {
            let region = decode_memory_region(addr);
            assert!(region.contains(addr));
            assert_eq!(
                MemoryRegion::Rom.contains(addr),
                region == MemoryRegion::Rom
            );
            assert_eq!(
                MemoryRegion::Ram.contains(addr),
                region == MemoryRegion::Ram
            );
            assert_eq!(
                MemoryRegion::Mmio.contains(addr),
                region == MemoryRegion::Mmio
            );
            assert_eq!(
                MemoryRegion::Diag.contains(addr),
                region == MemoryRegion::Diag
            );
            assert_eq!(
                MemoryRegion::Reserved.contains(addr),
                region == MemoryRegion::Reserved
            );
        }
    }

    #[test]
    fn canonical_region_descriptors_match_spec_bounds() {
        assert_eq!(
            FIXED_MEMORY_REGIONS,
            [
                RegionDescriptor {
                    region: MemoryRegion::Rom,
                    start: ROM_START,
                    end: ROM_END
                },
                RegionDescriptor {
                    region: MemoryRegion::Ram,
                    start: RAM_START,
                    end: RAM_END
                },
                RegionDescriptor {
                    region: MemoryRegion::Mmio,
                    start: MMIO_START,
                    end: MMIO_END
                },
                RegionDescriptor {
                    region: MemoryRegion::Diag,
                    start: DIAG_START,
                    end: DIAG_END
                },
                RegionDescriptor {
                    region: MemoryRegion::Reserved,
                    start: RESERVED_START,
                    end: RESERVED_END
                },
            ]
        );
    }
}
