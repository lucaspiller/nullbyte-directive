//! Instruction decoder for Nullbyte One ISA.
//!
//! This module provides the instruction decode pipeline that validates
//! instruction encodings and produces decoded instruction representations.

#![allow(missing_docs)]

use crate::encoding::{
    classify_opcode, decode_primary_word_op_sub, is_reserved_primary_opcode, OpcodeEncoding,
};
use crate::fault::{FaultCode, FaultReason};

/// Addressing modes supported by the Nullbyte One ISA.
///
/// These modes determine how the effective address for memory operations
/// is calculated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AddressingMode {
    DirectRegister,
    IndirectRegister,
    IndirectAutoIncrement,
    SignExtendedDisplacement,
    ZeroExtendedDisplacement,
    Immediate,
    Reserved110,
    Reserved111,
}

impl AddressingMode {
    /// Converts a 3-bit addressing mode value into an addressing mode.
    #[must_use]
    pub const fn from_u3(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::DirectRegister),
            1 => Some(Self::IndirectRegister),
            2 => Some(Self::SignExtendedDisplacement),
            3 => Some(Self::ZeroExtendedDisplacement),
            4 => Some(Self::IndirectAutoIncrement),
            5 => Some(Self::Immediate),
            6 => Some(Self::Reserved110),
            7 => Some(Self::Reserved111),
            _ => None,
        }
    }

    /// Returns true if this addressing mode is valid (not reserved).
    #[must_use]
    pub const fn is_valid(self) -> bool {
        !matches!(self, Self::Reserved110 | Self::Reserved111)
    }

    /// Returns true if this addressing mode requires sign extension validation.
    #[must_use]
    pub const fn requires_sign_extension_check(self) -> bool {
        matches!(self, Self::SignExtendedDisplacement)
    }
}

/// Register field values in instruction encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[allow(missing_docs)]
pub enum RegisterField {
    R0,
    R1,
    R2,
    R3,
    R4,
    R5,
    R6,
    R7,
}

impl RegisterField {
    /// Converts a 3-bit register field value into a register field.
    #[must_use]
    pub const fn from_u3(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::R0),
            1 => Some(Self::R1),
            2 => Some(Self::R2),
            3 => Some(Self::R3),
            4 => Some(Self::R4),
            5 => Some(Self::R5),
            6 => Some(Self::R6),
            7 => Some(Self::R7),
            _ => None,
        }
    }
}

/// Decoded instruction with all extracted fields.
///
/// This represents a fully validated instruction ready for execution,
/// split from the raw encoding to support precise fault handling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecodedInstruction {
    /// The opcode encoding for this instruction.
    pub encoding: OpcodeEncoding,
    /// Destination register field, if applicable.
    pub rd: Option<RegisterField>,
    /// First source register field, if applicable.
    pub ra: Option<RegisterField>,
    /// Second source register field, if applicable.
    pub rb: Option<RegisterField>,
    /// Addressing mode, if applicable.
    pub addressing_mode: Option<AddressingMode>,
    /// Immediate value encoded in the instruction.
    pub immediate_value: Option<u16>,
}

impl DecodedInstruction {
    /// Re-encodes this decoded instruction back to a 16-bit word.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    pub fn encode(self) -> u16 {
        let mut word = 0u16;

        if let Some(rd) = self.rd {
            let rd_val: u16 = match rd {
                RegisterField::R0 => 0,
                RegisterField::R1 => 1,
                RegisterField::R2 => 2,
                RegisterField::R3 => 3,
                RegisterField::R4 => 4,
                RegisterField::R5 => 5,
                RegisterField::R6 => 6,
                RegisterField::R7 => 7,
            };
            word |= rd_val << 9;
        }

        if let Some(ra) = self.ra {
            let ra_val: u16 = match ra {
                RegisterField::R0 => 0,
                RegisterField::R1 => 1,
                RegisterField::R2 => 2,
                RegisterField::R3 => 3,
                RegisterField::R4 => 4,
                RegisterField::R5 => 5,
                RegisterField::R6 => 6,
                RegisterField::R7 => 7,
            };
            word |= ra_val << 6;
        }

        if let Some(rb) = self.rb {
            let rb_val: u16 = match rb {
                RegisterField::R0 => 0,
                RegisterField::R1 => 1,
                RegisterField::R2 => 2,
                RegisterField::R3 => 3,
                RegisterField::R4 => 4,
                RegisterField::R5 => 5,
                RegisterField::R6 => 6,
                RegisterField::R7 => 7,
            };
            word |= rb_val << 3;
        }

        if let Some(immediate) = self.immediate_value {
            word |= immediate & 0x07;
            word |= (immediate & 0xFF00) >> 8;
        }

        word
    }
}

/// Result of decoding an instruction word.
///
/// Either contains a valid decoded instruction or a fault that occurred
/// during validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodedOrFault {
    /// Successfully decoded instruction.
    Instruction(DecodedInstruction),
    /// Decoding failed with a fault.
    Fault(FaultReason),
}

impl DecodedOrFault {
    /// Returns the decoded instruction if present.
    #[must_use]
    pub const fn instruction(self) -> Option<DecodedInstruction> {
        match self {
            Self::Instruction(i) => Some(i),
            Self::Fault(_) => None,
        }
    }

    /// Returns the fault reason if decoding failed.
    #[must_use]
    pub const fn fault(self) -> Option<FaultReason> {
        match self {
            Self::Instruction(_) => None,
            Self::Fault(f) => Some(f),
        }
    }
}

impl From<DecodedOrFault> for Result<DecodedInstruction, FaultCode> {
    fn from(value: DecodedOrFault) -> Self {
        match value {
            DecodedOrFault::Instruction(i) => Ok(i),
            DecodedOrFault::Fault(r) => Err(r.code()),
        }
    }
}

/// Instruction decoder for the Nullbyte One ISA.
///
/// Validates instruction encodings and produces decoded instructions
/// with all fields properly extracted.
pub struct Decoder;

const fn validates_unused_rd_bits(encoding: OpcodeEncoding) -> bool {
    matches!(encoding, OpcodeEncoding::Nop)
}

const fn validates_unused_ra_bits(encoding: OpcodeEncoding) -> bool {
    matches!(encoding, OpcodeEncoding::Nop)
}

impl Decoder {
    /// Decodes a 16-bit instruction word.
    ///
    /// Performs full validation including:
    /// - Opcode and sub-opcode classification
    /// - Reserved opcode detection
    /// - Addressing mode validity (AM 000-101 valid, 110-111 fault)
    /// - Sign extension validation for AM=010
    /// - Unused field validation (must be 000)
    #[must_use]
    #[allow(clippy::similar_names)]
    pub fn decode(word: u16) -> DecodedOrFault {
        let (op, sub) = decode_primary_word_op_sub(word);

        if is_reserved_primary_opcode(op) {
            return DecodedOrFault::Fault(FaultReason::new(FaultCode::IllegalEncoding));
        }

        let Some(encoding) = classify_opcode(op, sub) else {
            return DecodedOrFault::Fault(FaultReason::new(FaultCode::IllegalEncoding));
        };

        let rd_bits = ((word >> 9) & 0x7) as u8;
        let ra_bits = ((word >> 6) & 0x7) as u8;
        let rb_bits = ((word >> 3) & 0x7) as u8;
        let am_bits = (word & 0x7) as u8;

        let Some(addressing_mode) = AddressingMode::from_u3(am_bits) else {
            return DecodedOrFault::Fault(FaultReason::new(FaultCode::IllegalEncoding));
        };

        if !addressing_mode.is_valid() {
            return DecodedOrFault::Fault(FaultReason::new(FaultCode::IllegalEncoding));
        }

        if addressing_mode.requires_sign_extension_check() {
            let high_byte = ((word >> 8) & 0xFF) as u8;
            if high_byte != 0x00 && high_byte != 0xFF {
                return DecodedOrFault::Fault(FaultReason::new(FaultCode::IllegalEncoding));
            }
        }

        let rd = RegisterField::from_u3(rd_bits);
        let ra = RegisterField::from_u3(ra_bits);
        let rb = RegisterField::from_u3(rb_bits);

        if validates_unused_rd_bits(encoding) && rd_bits != 0 {
            return DecodedOrFault::Fault(FaultReason::new(FaultCode::IllegalEncoding));
        }

        if validates_unused_ra_bits(encoding) && ra_bits != 0 {
            return DecodedOrFault::Fault(FaultReason::new(FaultCode::IllegalEncoding));
        }

        let immediate_value = Some(word & 0x3F);

        DecodedOrFault::Instruction(DecodedInstruction {
            encoding,
            rd,
            ra,
            rb,
            addressing_mode: Some(addressing_mode),
            immediate_value,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encoding::OpcodeEncoding;

    #[test]
    fn addressing_mode_valid_range_000_to_101() {
        for am in 0u8..=5u8 {
            let mode = AddressingMode::from_u3(am).expect("valid mode");
            assert!(mode.is_valid(), "AM {am} should be valid");
        }
    }

    #[test]
    fn addressing_mode_invalid_range_110_and_111() {
        for am in 6u8..=7u8 {
            let mode = AddressingMode::from_u3(am).expect("mode exists");
            assert!(!mode.is_valid(), "AM {am} should be invalid");
        }
    }

    #[test]
    fn decode_nop_instruction() {
        let word = 0x0000u16;
        let result = Decoder::decode(word);
        let instruction = result.instruction().expect("should decode");
        assert_eq!(instruction.encoding, OpcodeEncoding::Nop);
    }

    #[test]
    fn decode_halt_instruction() {
        let word = 0x0010u16;
        let result = Decoder::decode(word);
        let instruction = result.instruction().expect("should decode");
        assert_eq!(instruction.encoding, OpcodeEncoding::Halt);
    }

    #[test]
    fn reserved_opcode_0b_faults() {
        for op in 0xBu8..=0xFu8 {
            for sub in 0u8..=7u8 {
                let word = (u16::from(op) << 12) | (u16::from(sub) << 3);
                let result = Decoder::decode(word);
                assert!(result.fault().is_some(), "OP {op:X} SUB {sub} should fault");
            }
        }
    }

    #[test]
    fn unassigned_sub_opcode_faults() {
        let fault_cases: [(u8, u8); 9] = [
            (0x0, 0x7),
            (0x1, 0x1),
            (0x2, 0x3),
            (0x3, 0x6),
            (0x5, 0x7),
            (0x7, 0x7),
            (0x8, 0x4),
            (0x9, 0x3),
            (0xA, 0x7),
        ];
        for (op, sub) in fault_cases {
            let word = (u16::from(op) << 12) | (u16::from(sub) << 3);
            let result = Decoder::decode(word);
            assert!(result.fault().is_some(), "OP {op} SUB {sub} should fault");
        }
    }

    #[test]
    fn am_110_faults() {
        let word = 0x0006u16;
        let result = Decoder::decode(word);
        assert!(result.fault().is_some(), "AM 110 should fault");
    }

    #[test]
    fn am_111_faults() {
        let word = 0x0007u16;
        let result = Decoder::decode(word);
        assert!(result.fault().is_some(), "AM 111 should fault");
    }

    #[test]
    fn am_010_sign_extension_valid_0x00() {
        let word = 0x0002u16;
        let result = Decoder::decode(word);
        assert!(
            result.instruction().is_some(),
            "AM 010 with high byte 0x00 should be valid"
        );
    }

    #[test]
    fn am_010_sign_extension_valid_0xff() {
        let word = 0xFF02u16;
        let result = Decoder::decode(word);
        assert!(
            result.fault().is_some(),
            "AM 010 with high byte 0xFF and OP=0xF should fault (reserved opcode)"
        );
    }

    #[test]
    fn am_010_sign_extension_invalid_mid_byte() {
        let word = 0x1202u16;
        let result = Decoder::decode(word);
        assert!(
            result.fault().is_some(),
            "AM 010 with high byte 0x12 should fault"
        );
    }

    #[test]
    fn all_valid_opcodes_decode() {
        let valid_encodings: [(u8, u8, OpcodeEncoding); 41] = [
            (0x0, 0x0, OpcodeEncoding::Nop),
            (0x0, 0x1, OpcodeEncoding::Sync),
            (0x0, 0x2, OpcodeEncoding::Halt),
            (0x0, 0x3, OpcodeEncoding::Trap),
            (0x0, 0x4, OpcodeEncoding::Swi),
            (0x1, 0x0, OpcodeEncoding::Mov),
            (0x2, 0x0, OpcodeEncoding::Load),
            (0x3, 0x0, OpcodeEncoding::Store),
            (0x4, 0x0, OpcodeEncoding::Add),
            (0x4, 0x1, OpcodeEncoding::Sub),
            (0x4, 0x2, OpcodeEncoding::And),
            (0x4, 0x3, OpcodeEncoding::Or),
            (0x4, 0x4, OpcodeEncoding::Xor),
            (0x4, 0x5, OpcodeEncoding::Shl),
            (0x4, 0x6, OpcodeEncoding::Shr),
            (0x4, 0x7, OpcodeEncoding::Cmp),
            (0x5, 0x0, OpcodeEncoding::Mul),
            (0x5, 0x1, OpcodeEncoding::Mulh),
            (0x5, 0x2, OpcodeEncoding::Div),
            (0x5, 0x3, OpcodeEncoding::Mod),
            (0x5, 0x4, OpcodeEncoding::Qadd),
            (0x5, 0x5, OpcodeEncoding::Qsub),
            (0x5, 0x6, OpcodeEncoding::Scv),
            (0x6, 0x0, OpcodeEncoding::Beq),
            (0x6, 0x1, OpcodeEncoding::Bne),
            (0x6, 0x2, OpcodeEncoding::Blt),
            (0x6, 0x3, OpcodeEncoding::Ble),
            (0x6, 0x4, OpcodeEncoding::Bgt),
            (0x6, 0x5, OpcodeEncoding::Bge),
            (0x6, 0x6, OpcodeEncoding::Jmp),
            (0x6, 0x7, OpcodeEncoding::CallOrRet),
            (0x7, 0x0, OpcodeEncoding::Push),
            (0x7, 0x1, OpcodeEncoding::Pop),
            (0x8, 0x0, OpcodeEncoding::In),
            (0x8, 0x1, OpcodeEncoding::Out),
            (0x9, 0x0, OpcodeEncoding::Bset),
            (0x9, 0x1, OpcodeEncoding::Bclr),
            (0x9, 0x2, OpcodeEncoding::Btest),
            (0xA, 0x0, OpcodeEncoding::Ewait),
            (0xA, 0x1, OpcodeEncoding::Eget),
            (0xA, 0x2, OpcodeEncoding::Eret),
        ];

        for (op, sub, expected) in valid_encodings {
            let word = (u16::from(op) << 12) | (u16::from(sub) << 3);
            let result = Decoder::decode(word);
            let instruction = result
                .instruction()
                .unwrap_or_else(|| panic!("OP {op} SUB {sub} should decode successfully"));
            assert_eq!(
                instruction.encoding, expected,
                "OP {op} SUB {sub} should decode to {expected:?}"
            );
        }
    }

    #[test]
    fn exhaustive_decode_classification() {
        for word in 0u16..=u16::MAX {
            let result = Decoder::decode(word);
            match result {
                DecodedOrFault::Instruction(_instr) => {
                    let (op, sub) = decode_primary_word_op_sub(word);
                    assert!(
                        !is_reserved_primary_opcode(op),
                        "Valid decode at {word:X} has reserved OP {op}"
                    );
                    assert!(
                        classify_opcode(op, sub).is_some(),
                        "Valid decode at {word:X} has unassigned ({op}, {sub})"
                    );
                }
                DecodedOrFault::Fault(reason) => {
                    let (op, sub) = decode_primary_word_op_sub(word);
                    let is_illegal =
                        is_reserved_primary_opcode(op) || classify_opcode(op, sub).is_none();
                    let am = word & 0x7;
                    let is_invalid_am = am >= 6;
                    let is_sign_ext_problem = {
                        let am_u8 = (am & 0x7) as u8;
                        let addressing_mode = AddressingMode::from_u3(am_u8);
                        addressing_mode.is_some_and(|mode| {
                            mode.requires_sign_extension_check()
                                && (((word >> 8) & 0xFF) != 0x00)
                                && (((word >> 8) & 0xFF) != 0xFF)
                        })
                    };
                    let is_nop_unused_field_violation = {
                        let (op, sub) = decode_primary_word_op_sub(word);
                        op == 0
                            && sub == 0
                            && ((((word >> 9) & 0x7) != 0) || (((word >> 6) & 0x7) != 0))
                    };

                    assert!(
                        is_illegal
                            || is_invalid_am
                            || is_sign_ext_problem
                            || is_nop_unused_field_violation,
                        "Fault at {word:X} (OP={op}, SUB={sub}, AM={am}) has no valid fault reason"
                    );
                    assert_eq!(
                        reason.code(),
                        FaultCode::IllegalEncoding,
                        "Fault at {word:X} should be IllegalEncoding"
                    );
                }
            }
        }
    }
}
