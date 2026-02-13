//! Deterministic memory access policy helpers by fixed architectural region.

use crate::{decode_memory_region, FaultCode, MemoryRegion};

/// Canonical byte width for architectural 16-bit accesses.
pub const WORD_ACCESS_BYTES: u8 = 2;

/// Validates instruction fetch legality for a 16-bit architectural address.
///
/// Fetch is legal only from ROM and RAM. Fetches from MMIO/DIAG/reserved
/// regions deterministically fault with [`FaultCode::NonExecutableFetch`].
///
/// # Errors
///
/// Returns [`FaultCode::NonExecutableFetch`] when `addr` is outside executable
/// ROM/RAM regions.
pub const fn validate_fetch_access(addr: u16) -> Result<(), FaultCode> {
    match decode_memory_region(addr) {
        MemoryRegion::Rom | MemoryRegion::Ram => Ok(()),
        MemoryRegion::Mmio | MemoryRegion::Diag | MemoryRegion::Reserved => {
            Err(FaultCode::NonExecutableFetch)
        }
    }
}

/// Validates architectural write legality for a 16-bit address.
///
/// Writes are legal only to RAM and MMIO. Writes to ROM/DIAG/reserved regions
/// deterministically fault with [`FaultCode::IllegalMemoryAccess`].
///
/// # Errors
///
/// Returns [`FaultCode::IllegalMemoryAccess`] when `addr` is not writable by
/// architectural policy.
pub const fn validate_write_access(addr: u16) -> Result<(), FaultCode> {
    match decode_memory_region(addr) {
        MemoryRegion::Ram | MemoryRegion::Mmio => Ok(()),
        MemoryRegion::Rom | MemoryRegion::Diag | MemoryRegion::Reserved => {
            Err(FaultCode::IllegalMemoryAccess)
        }
    }
}

/// Validates alignment for 16-bit architectural data memory accesses.
///
/// # Errors
///
/// Returns [`FaultCode::UnalignedDataAccess`] when `addr` is odd.
pub const fn validate_word_alignment(addr: u16) -> Result<(), FaultCode> {
    if addr & 1 == 0 {
        Ok(())
    } else {
        Err(FaultCode::UnalignedDataAccess)
    }
}

/// Validates width for MMIO operations that must be exactly 16-bit.
///
/// # Errors
///
/// Returns [`FaultCode::MmioWidthViolation`] when `width_bytes != 2`.
pub const fn validate_mmio_width(width_bytes: u8) -> Result<(), FaultCode> {
    if width_bytes == WORD_ACCESS_BYTES {
        Ok(())
    } else {
        Err(FaultCode::MmioWidthViolation)
    }
}

/// Validates alignment for MMIO operations that must be 16-bit aligned.
///
/// # Errors
///
/// Returns [`FaultCode::MmioAlignmentViolation`] when `addr` is odd.
pub const fn validate_mmio_alignment(addr: u16) -> Result<(), FaultCode> {
    if addr & 1 == 0 {
        Ok(())
    } else {
        Err(FaultCode::MmioAlignmentViolation)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        validate_fetch_access, validate_mmio_alignment, validate_mmio_width,
        validate_word_alignment, validate_write_access, FaultCode, DIAG_END, DIAG_START, MMIO_END,
        MMIO_START, RAM_END, RAM_START, RESERVED_END, RESERVED_START, ROM_END, ROM_START,
        WORD_ACCESS_BYTES,
    };

    #[test]
    fn fetch_legality_matches_region_policy() {
        assert_eq!(validate_fetch_access(ROM_START), Ok(()));
        assert_eq!(validate_fetch_access(ROM_END), Ok(()));
        assert_eq!(validate_fetch_access(RAM_START), Ok(()));
        assert_eq!(validate_fetch_access(RAM_END), Ok(()));

        assert_eq!(
            validate_fetch_access(MMIO_START),
            Err(FaultCode::NonExecutableFetch)
        );
        assert_eq!(
            validate_fetch_access(MMIO_END),
            Err(FaultCode::NonExecutableFetch)
        );
        assert_eq!(
            validate_fetch_access(DIAG_START),
            Err(FaultCode::NonExecutableFetch)
        );
        assert_eq!(
            validate_fetch_access(DIAG_END),
            Err(FaultCode::NonExecutableFetch)
        );
        assert_eq!(
            validate_fetch_access(RESERVED_START),
            Err(FaultCode::NonExecutableFetch)
        );
        assert_eq!(
            validate_fetch_access(RESERVED_END),
            Err(FaultCode::NonExecutableFetch)
        );
    }

    #[test]
    fn write_legality_matches_region_policy() {
        assert_eq!(
            validate_write_access(ROM_START),
            Err(FaultCode::IllegalMemoryAccess)
        );
        assert_eq!(
            validate_write_access(ROM_END),
            Err(FaultCode::IllegalMemoryAccess)
        );
        assert_eq!(validate_write_access(RAM_START), Ok(()));
        assert_eq!(validate_write_access(RAM_END), Ok(()));
        assert_eq!(validate_write_access(MMIO_START), Ok(()));
        assert_eq!(validate_write_access(MMIO_END), Ok(()));
        assert_eq!(
            validate_write_access(DIAG_START),
            Err(FaultCode::IllegalMemoryAccess)
        );
        assert_eq!(
            validate_write_access(DIAG_END),
            Err(FaultCode::IllegalMemoryAccess)
        );
        assert_eq!(
            validate_write_access(RESERVED_START),
            Err(FaultCode::IllegalMemoryAccess)
        );
        assert_eq!(
            validate_write_access(RESERVED_END),
            Err(FaultCode::IllegalMemoryAccess)
        );
    }

    #[test]
    fn fetch_fault_outcome_is_deterministic_for_non_executable_regions() {
        for addr in MMIO_START..=RESERVED_END {
            assert_eq!(
                validate_fetch_access(addr),
                Err(FaultCode::NonExecutableFetch)
            );
        }
    }

    #[test]
    fn write_fault_outcome_is_deterministic_for_non_writable_regions() {
        for addr in ROM_START..=RESERVED_END {
            let is_writable =
                (RAM_START..=RAM_END).contains(&addr) || (MMIO_START..=MMIO_END).contains(&addr);
            if is_writable {
                assert_eq!(validate_write_access(addr), Ok(()));
            } else {
                assert_eq!(
                    validate_write_access(addr),
                    Err(FaultCode::IllegalMemoryAccess)
                );
            }
        }
    }

    #[test]
    fn word_alignment_rejects_odd_addresses() {
        assert_eq!(validate_word_alignment(0x0000), Ok(()));
        assert_eq!(validate_word_alignment(0x0002), Ok(()));
        assert_eq!(
            validate_word_alignment(0x0001),
            Err(FaultCode::UnalignedDataAccess)
        );
        assert_eq!(
            validate_word_alignment(0xFFFF),
            Err(FaultCode::UnalignedDataAccess)
        );
    }

    #[test]
    fn word_alignment_outcome_is_deterministic_for_all_addresses() {
        for addr in 0_u16..=u16::MAX {
            if addr & 1 == 0 {
                assert_eq!(validate_word_alignment(addr), Ok(()));
            } else {
                assert_eq!(
                    validate_word_alignment(addr),
                    Err(FaultCode::UnalignedDataAccess)
                );
            }
        }
    }

    #[test]
    fn mmio_width_rejects_non_word_widths() {
        assert_eq!(validate_mmio_width(WORD_ACCESS_BYTES), Ok(()));
        assert_eq!(validate_mmio_width(0), Err(FaultCode::MmioWidthViolation));
        assert_eq!(validate_mmio_width(1), Err(FaultCode::MmioWidthViolation));
        assert_eq!(validate_mmio_width(3), Err(FaultCode::MmioWidthViolation));
        assert_eq!(validate_mmio_width(4), Err(FaultCode::MmioWidthViolation));
    }

    #[test]
    fn mmio_alignment_rejects_odd_addresses() {
        assert_eq!(validate_mmio_alignment(0xE000), Ok(()));
        assert_eq!(
            validate_mmio_alignment(0xE001),
            Err(FaultCode::MmioAlignmentViolation)
        );
    }
}
