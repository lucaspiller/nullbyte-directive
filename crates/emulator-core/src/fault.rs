use thiserror::Error;

/// Fault classes used for diagnostics aggregation and policy decisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum FaultClass {
    /// Decoder rejected an instruction encoding.
    Decode,
    /// Core memory or fetch policy violation.
    Memory,
    /// MMIO contract width/alignment violation.
    Mmio,
    /// Event queue integration violation.
    Event,
    /// Trap/fault dispatch path violation.
    Dispatch,
    /// Tick budget overrun condition.
    Budget,
    /// Capability-gated operation violation.
    Capability,
}

/// Stable fault taxonomy for section 12 semantics and dispatch escalation paths.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Error)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[repr(u8)]
pub enum FaultCode {
    /// Illegal opcode, addressing mode, or encoding field combination.
    #[error("illegal instruction encoding")]
    IllegalEncoding = 0x01,
    /// Fetch attempted from a region that is not executable.
    #[error("instruction fetch from non-executable region")]
    NonExecutableFetch = 0x02,
    /// Data access targeted a reserved or non-readable location.
    #[error("memory access to reserved or non-readable region")]
    IllegalMemoryAccess = 0x03,
    /// 16-bit access used an odd address.
    #[error("unaligned 16-bit data access")]
    UnalignedDataAccess = 0x04,
    /// MMIO operation used an unsupported access width.
    #[error("mmio access violated width constraints")]
    MmioWidthViolation = 0x05,
    /// MMIO operation used an invalid alignment.
    #[error("mmio access violated alignment constraints")]
    MmioAlignmentViolation = 0x06,
    /// Host attempted to enqueue into a full bounded event queue.
    #[error("event queue overflow")]
    EventQueueOverflow = 0x07,
    /// `ERET` executed without active handler context.
    #[error("eret executed outside handler context")]
    HandlerContextViolation = 0x08,
    /// Operation required a disabled capability bit.
    #[error("capability-gated feature used while disabled")]
    CapabilityViolation = 0x09,
    /// Instruction retirement crossed the tick cycle budget.
    #[error("tick budget exceeded")]
    BudgetOverrun = 0x0A,
    /// `VEC_FAULT` target is invalid for dispatch.
    #[error("fault vector is invalid")]
    InvalidFaultVector = 0x0B,
    /// A second fault happened while handling a fault.
    #[error("fault occurred while already handling a fault")]
    DoubleFault = 0x0C,
}

impl FaultCode {
    /// Converts a fault code to the stable low-byte value stored in `CAUSE`.
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }

    /// Converts the stable low-byte `CAUSE` value back into a fault code.
    #[must_use]
    pub const fn from_u8(code: u8) -> Option<Self> {
        match code {
            0x01 => Some(Self::IllegalEncoding),
            0x02 => Some(Self::NonExecutableFetch),
            0x03 => Some(Self::IllegalMemoryAccess),
            0x04 => Some(Self::UnalignedDataAccess),
            0x05 => Some(Self::MmioWidthViolation),
            0x06 => Some(Self::MmioAlignmentViolation),
            0x07 => Some(Self::EventQueueOverflow),
            0x08 => Some(Self::HandlerContextViolation),
            0x09 => Some(Self::CapabilityViolation),
            0x0A => Some(Self::BudgetOverrun),
            0x0B => Some(Self::InvalidFaultVector),
            0x0C => Some(Self::DoubleFault),
            _ => None,
        }
    }

    /// Returns the diagnostics fault class for this fault code.
    #[must_use]
    pub const fn class(self) -> FaultClass {
        match self {
            Self::IllegalEncoding => FaultClass::Decode,
            Self::NonExecutableFetch | Self::IllegalMemoryAccess | Self::UnalignedDataAccess => {
                FaultClass::Memory
            }
            Self::MmioWidthViolation | Self::MmioAlignmentViolation => FaultClass::Mmio,
            Self::EventQueueOverflow => FaultClass::Event,
            Self::HandlerContextViolation | Self::InvalidFaultVector | Self::DoubleFault => {
                FaultClass::Dispatch
            }
            Self::BudgetOverrun => FaultClass::Budget,
            Self::CapabilityViolation => FaultClass::Capability,
        }
    }

    /// Faults that halt the core immediately instead of entering normal recovery.
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::InvalidFaultVector | Self::DoubleFault)
    }
}

#[cfg(test)]
mod tests {
    use super::{FaultClass, FaultCode};

    #[test]
    fn stable_code_roundtrip_is_bijective_for_defined_values() {
        for code in 0x01u8..=0x0C {
            let fault = FaultCode::from_u8(code).expect("defined taxonomy code");
            assert_eq!(fault.as_u8(), code);
        }
    }

    #[test]
    fn unknown_code_is_rejected() {
        assert!(FaultCode::from_u8(0x00).is_none());
        assert!(FaultCode::from_u8(0xFF).is_none());
    }

    #[test]
    fn terminal_faults_match_escalation_contract() {
        assert!(FaultCode::InvalidFaultVector.is_terminal());
        assert!(FaultCode::DoubleFault.is_terminal());
        assert!(!FaultCode::BudgetOverrun.is_terminal());
    }

    #[test]
    fn class_mapping_matches_fault_taxonomy() {
        assert_eq!(FaultCode::IllegalEncoding.class(), FaultClass::Decode);
        assert_eq!(FaultCode::IllegalMemoryAccess.class(), FaultClass::Memory);
        assert_eq!(FaultCode::MmioWidthViolation.class(), FaultClass::Mmio);
        assert_eq!(FaultCode::EventQueueOverflow.class(), FaultClass::Event);
        assert_eq!(
            FaultCode::HandlerContextViolation.class(),
            FaultClass::Dispatch
        );
        assert_eq!(FaultCode::BudgetOverrun.class(), FaultClass::Budget);
        assert_eq!(
            FaultCode::CapabilityViolation.class(),
            FaultClass::Capability
        );
    }
}
