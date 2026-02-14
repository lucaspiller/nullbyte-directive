//! Instruction and directive encoding (Pass 2).
//!
//! This module implements the encoding phase of assembly: converting parsed
//! instructions and directives into binary bytes suitable for ROM loading.

use emulator_core::OpcodeEncoding;

use crate::parser::{Directive, InstructionSize, Operand, ParsedInstruction, ParsedLine};
use crate::symbols::SymbolTable;

/// Addressing mode bit values for the AM field.
mod am {
    pub const REGISTER_DIRECT: u8 = 0b000;
    pub const REGISTER_INDIRECT: u8 = 0b001;
    pub const SIGN_EXTENDED_DISPLACEMENT: u8 = 0b010;
    pub const ZERO_EXTENDED_DISPLACEMENT: u8 = 0b011;
    pub const IMMEDIATE: u8 = 0b100;
    pub const PC_RELATIVE: u8 = 0b101;
}

/// Error during encoding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodeError {
    /// Kind of error.
    pub kind: EncodeErrorKind,
    /// Source line where the error occurred.
    pub line: usize,
}

/// Classification of encoding errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EncodeErrorKind {
    /// Undefined label reference.
    UndefinedLabel(String),
    /// Displacement out of signed 8-bit range.
    DisplacementOutOfRange(i16),
    /// Immediate value out of 16-bit range.
    ImmediateOutOfRange(i64),
    /// PC-relative offset out of 16-bit range.
    PcRelativeOutOfRange(i32),
    /// Cannot encode instruction.
    InvalidEncoding(String),
}

impl std::fmt::Display for EncodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.kind)
    }
}

impl std::fmt::Display for EncodeErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UndefinedLabel(name) => write!(f, "undefined label: {name}"),
            Self::DisplacementOutOfRange(disp) => {
                write!(f, "displacement out of range: {disp}")
            }
            Self::ImmediateOutOfRange(val) => {
                write!(f, "immediate value out of range: {val}")
            }
            Self::PcRelativeOutOfRange(offset) => {
                write!(f, "PC-relative offset out of range: {offset}")
            }
            Self::InvalidEncoding(msg) => write!(f, "invalid encoding: {msg}"),
        }
    }
}

impl std::error::Error for EncodeError {}

/// Encoded output for a single instruction or directive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedOutput {
    /// The encoded bytes.
    pub bytes: Vec<u8>,
    /// Source line number.
    pub source_line: usize,
}

/// Encodes a primary instruction word.
///
/// Bit layout: `[OP:4][RD:3][RA:3][SUB:3][AM:3]`
#[must_use]
#[allow(clippy::similar_names)]
pub fn encode_primary_word(op: u8, rd: u8, ra: u8, sub: u8, am: u8) -> u16 {
    let op_part = u16::from(op & 0x0F) << 12;
    let rd_part = u16::from(rd & 0x07) << 9;
    let ra_part = u16::from(ra & 0x07) << 6;
    let sub_part = u16::from(sub & 0x07) << 3;
    let am_part = u16::from(am & 0x07);
    op_part | rd_part | ra_part | sub_part | am_part
}

/// Encodes an instruction to bytes.
///
/// Returns a vector of bytes (2 or 4 bytes depending on addressing mode).
///
/// # Errors
///
/// Returns `EncodeError` if:
/// - A label reference cannot be resolved
/// - A displacement is out of signed 8-bit range
/// - An immediate value is out of 16-bit range
/// - A PC-relative offset is out of 16-bit range
#[allow(
    clippy::cast_sign_loss,
    clippy::cast_possible_truncation,
    clippy::missing_panics_doc
)]
pub fn encode_instruction(
    instr: &ParsedInstruction,
    symbols: &SymbolTable,
    pc: u16,
    source_line: usize,
) -> Result<Vec<u8>, EncodeError> {
    let (op, sub, _encoding) = instr.resolution;

    let rd = instr.rd.map_or(0, |r| r.0);

    let (ra, am, extension_word) = match &instr.operand {
        None => (instr.ra.map_or(0, |r| r.0), am::REGISTER_DIRECT, None),
        Some(Operand::Register(_rb)) => (instr.ra.map_or(0, |r| r.0), am::REGISTER_DIRECT, None),
        Some(Operand::Memory(mem)) => {
            let ra = mem.base.0;
            if let Some(disp) = mem.displacement {
                if !(-128..=127).contains(&disp) {
                    return Err(EncodeError {
                        kind: EncodeErrorKind::DisplacementOutOfRange(disp),
                        line: source_line,
                    });
                }
                let disp8 = disp as i8 as u8;
                let ext_high = if disp8 & 0x80 != 0 { 0xFFu8 } else { 0x00u8 };
                let ext = u16::from_be_bytes([ext_high, disp8]);
                (ra, am::SIGN_EXTENDED_DISPLACEMENT, Some(ext))
            } else {
                (ra, am::REGISTER_INDIRECT, None)
            }
        }
        Some(Operand::Immediate(imm)) => {
            let ra = instr.ra.map_or(0, |r| r.0);
            if imm.is_label {
                let label_name = imm.label_name.as_ref().ok_or_else(|| EncodeError {
                    kind: EncodeErrorKind::InvalidEncoding("label reference without name".into()),
                    line: source_line,
                })?;
                let symbol = symbols.get(label_name).ok_or_else(|| EncodeError {
                    kind: EncodeErrorKind::UndefinedLabel(label_name.clone()),
                    line: source_line,
                })?;
                let label_value = symbol.address;
                let pc_next = pc.wrapping_add(if instr.size == InstructionSize::TwoWords {
                    4
                } else {
                    2
                });
                let offset = i32::from(label_value) - i32::from(pc_next);
                if !(-32768..=32767).contains(&offset) {
                    return Err(EncodeError {
                        kind: EncodeErrorKind::PcRelativeOutOfRange(offset),
                        line: source_line,
                    });
                }
                let ext = offset as i16 as u16;
                (ra, am::PC_RELATIVE, Some(ext))
            } else {
                let val = imm.value;
                if !(0..=0xFFFF).contains(&val) {
                    return Err(EncodeError {
                        kind: EncodeErrorKind::ImmediateOutOfRange(val),
                        line: source_line,
                    });
                }
                let ext = val as u16;
                let encoding = instr.resolution.2;
                if encoding == OpcodeEncoding::Load || encoding == OpcodeEncoding::Store {
                    (ra, am::ZERO_EXTENDED_DISPLACEMENT, Some(ext))
                } else {
                    (ra, am::IMMEDIATE, Some(ext))
                }
            }
        }
    };

    let _rb = match &instr.operand {
        Some(Operand::Register(rb)) => rb.0,
        _ => 0,
    };

    let primary = encode_primary_word(op, rd, ra, sub, am);

    let mut bytes = Vec::with_capacity(4);
    bytes.extend_from_slice(&primary.to_be_bytes());

    if let Some(ext) = extension_word {
        bytes.extend_from_slice(&ext.to_be_bytes());
    }

    Ok(bytes)
}

/// Encodes a directive to bytes.
///
/// # Errors
///
/// Returns `EncodeError` if a value is out of range.
#[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
pub fn encode_directive(
    directive: &Directive,
    current_address: u16,
    _source_line: usize,
) -> Result<Vec<u8>, EncodeError> {
    match directive {
        Directive::Org(addr) => {
            let target = *addr as u16;
            if target > current_address {
                let gap = target - current_address;
                Ok(vec![0u8; gap as usize])
            } else {
                Ok(Vec::new())
            }
        }
        Directive::Word(val) => Ok(val.to_be_bytes().to_vec()),
        Directive::Byte(val) => Ok(vec![*val]),
        Directive::Ascii(s) => Ok(s.as_bytes().to_vec()),
        Directive::Zero(count) => Ok(vec![0u8; *count]),
    }
}

/// Encodes a parsed line to bytes.
///
/// # Errors
///
/// Returns `EncodeError` if encoding fails.
pub fn encode_line(
    parsed: &ParsedLine,
    symbols: &SymbolTable,
    current_address: u16,
    source_line: usize,
) -> Result<Vec<u8>, EncodeError> {
    match parsed {
        ParsedLine::Blank | ParsedLine::Label { .. } => Ok(Vec::new()),
        ParsedLine::Directive { directive } => {
            encode_directive(directive, current_address, source_line)
        }
        ParsedLine::Instruction { instruction } => {
            encode_instruction(instruction, symbols, current_address, source_line)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_line;
    use emulator_core::{DecodedOrFault, Decoder};

    #[test]
    fn encode_primary_word_layout() {
        let word = encode_primary_word(0x4, 0x1, 0x2, 0x3, 0x5);
        assert_eq!((word >> 12) & 0xF, 0x4);
        assert_eq!((word >> 9) & 0x7, 0x1);
        assert_eq!((word >> 6) & 0x7, 0x2);
        assert_eq!((word >> 3) & 0x7, 0x3);
        assert_eq!(word & 0x7, 0x5);
    }

    #[test]
    fn encode_nop() {
        let parsed = parse_line("NOP", 1).unwrap();
        let symbols = SymbolTable::new();
        let bytes = encode_line(&parsed, &symbols, 0, 1).unwrap();
        assert_eq!(bytes, &[0x00, 0x00]);
    }

    #[test]
    fn encode_halt() {
        let parsed = parse_line("HALT", 1).unwrap();
        let symbols = SymbolTable::new();
        let bytes = encode_line(&parsed, &symbols, 0, 1).unwrap();
        assert_eq!(bytes, &[0x00, 0x10]);
    }

    #[test]
    fn encode_mov_register() {
        let parsed = parse_line("MOV R0, R1", 1).unwrap();
        let symbols = SymbolTable::new();
        let bytes = encode_line(&parsed, &symbols, 0, 1).unwrap();
        assert_eq!(bytes.len(), 2);
        let word = u16::from_be_bytes([bytes[0], bytes[1]]);
        assert_eq!((word >> 12) & 0xF, 0x1);
        assert_eq!((word >> 9) & 0x7, 0x0);
        assert_eq!(word & 0x7, u16::from(am::REGISTER_DIRECT));
    }

    #[test]
    fn encode_mov_immediate() {
        let parsed = parse_line("MOV R1, #0x1234", 1).unwrap();
        let symbols = SymbolTable::new();
        let bytes = encode_line(&parsed, &symbols, 0, 1).unwrap();
        assert_eq!(bytes.len(), 4);
        let primary = u16::from_be_bytes([bytes[0], bytes[1]]);
        let extension = u16::from_be_bytes([bytes[2], bytes[3]]);
        assert_eq!((primary >> 12) & 0xF, 0x1);
        assert_eq!((primary >> 9) & 0x7, 0x1);
        assert_eq!(primary & 0x7, u16::from(am::IMMEDIATE));
        assert_eq!(extension, 0x1234);
    }

    #[test]
    fn encode_load_indirect() {
        let parsed = parse_line("LOAD R2, [R3]", 1).unwrap();
        let symbols = SymbolTable::new();
        let bytes = encode_line(&parsed, &symbols, 0, 1).unwrap();
        assert_eq!(bytes.len(), 2);
        let word = u16::from_be_bytes([bytes[0], bytes[1]]);
        assert_eq!((word >> 12) & 0xF, 0x2);
        assert_eq!((word >> 9) & 0x7, 0x2);
        assert_eq!(word & 0x7, u16::from(am::REGISTER_INDIRECT));
    }

    #[test]
    fn encode_load_displacement() {
        let parsed = parse_line("LOAD R0, [R1 + 10]", 1).unwrap();
        let symbols = SymbolTable::new();
        let bytes = encode_line(&parsed, &symbols, 0, 1).unwrap();
        assert_eq!(bytes.len(), 4);
        let primary = u16::from_be_bytes([bytes[0], bytes[1]]);
        let extension = u16::from_be_bytes([bytes[2], bytes[3]]);
        assert_eq!((primary >> 12) & 0xF, 0x2);
        assert_eq!(primary & 0x7, u16::from(am::SIGN_EXTENDED_DISPLACEMENT));
        assert_eq!(extension, 0x000A);
    }

    #[test]
    fn encode_load_negative_displacement() {
        let parsed = parse_line("LOAD R0, [R1 - 5]", 1).unwrap();
        let symbols = SymbolTable::new();
        let bytes = encode_line(&parsed, &symbols, 0, 1).unwrap();
        assert_eq!(bytes.len(), 4);
        let primary = u16::from_be_bytes([bytes[0], bytes[1]]);
        let extension = u16::from_be_bytes([bytes[2], bytes[3]]);
        assert_eq!(primary & 0x7, u16::from(am::SIGN_EXTENDED_DISPLACEMENT));
        assert_eq!(extension, 0xFFFB);
    }

    #[test]
    fn encode_store_indirect() {
        let parsed = parse_line("STORE R3, [R4]", 1).unwrap();
        let symbols = SymbolTable::new();
        let bytes = encode_line(&parsed, &symbols, 0, 1).unwrap();
        assert_eq!(bytes.len(), 2);
        let word = u16::from_be_bytes([bytes[0], bytes[1]]);
        assert_eq!((word >> 12) & 0xF, 0x3);
        assert_eq!(word & 0x7, u16::from(am::REGISTER_INDIRECT));
    }

    #[test]
    fn encode_add_registers() {
        let parsed = parse_line("ADD R0, R1, R2", 1).unwrap();
        let symbols = SymbolTable::new();
        let bytes = encode_line(&parsed, &symbols, 0, 1).unwrap();
        assert_eq!(bytes.len(), 2);
        let word = u16::from_be_bytes([bytes[0], bytes[1]]);
        assert_eq!((word >> 12) & 0xF, 0x4);
        assert_eq!((word >> 9) & 0x7, 0x0);
        assert_eq!((word >> 6) & 0x7, 0x1);
        assert_eq!((word >> 3) & 0x7, 0x0);
        assert_eq!(word & 0x7, u16::from(am::REGISTER_DIRECT));
    }

    #[test]
    fn encode_jmp_label() {
        let mut symbols = SymbolTable::new();
        symbols.insert(
            "target".to_string(),
            crate::symbols::Symbol {
                address: 0x0010,
                defined_at: 1,
            },
        );

        let parsed = parse_line("JMP #target", 1).unwrap();
        let bytes = encode_line(&parsed, &symbols, 0x0000, 1).unwrap();
        assert_eq!(bytes.len(), 4);
        let primary = u16::from_be_bytes([bytes[0], bytes[1]]);
        let extension = u16::from_be_bytes([bytes[2], bytes[3]]);
        assert_eq!((primary >> 12) & 0xF, 0x6);
        assert_eq!((primary >> 3) & 0x7, 0x6);
        assert_eq!(primary & 0x7, u16::from(am::PC_RELATIVE));
        assert_eq!(extension, 0x000C);
    }

    #[test]
    fn encode_beq_forward() {
        let mut symbols = SymbolTable::new();
        symbols.insert(
            "forward".to_string(),
            crate::symbols::Symbol {
                address: 0x0100,
                defined_at: 1,
            },
        );

        let parsed = parse_line("BEQ #forward", 1).unwrap();
        let bytes = encode_line(&parsed, &symbols, 0x0000, 1).unwrap();
        assert_eq!(bytes.len(), 4);
        let extension = u16::from_be_bytes([bytes[2], bytes[3]]);
        assert_eq!(extension, 0x00FC);
    }

    #[test]
    fn encode_directive_word() {
        let parsed = parse_line(".word 0x1234", 1).unwrap();
        let symbols = SymbolTable::new();
        let bytes = encode_line(&parsed, &symbols, 0, 1).unwrap();
        assert_eq!(bytes, &[0x12, 0x34]);
    }

    #[test]
    fn encode_directive_byte() {
        let parsed = parse_line(".byte 0x42", 1).unwrap();
        let symbols = SymbolTable::new();
        let bytes = encode_line(&parsed, &symbols, 0, 1).unwrap();
        assert_eq!(bytes, &[0x42]);
    }

    #[test]
    fn encode_directive_ascii() {
        let parsed = parse_line(".ascii \"AB\"", 1).unwrap();
        let symbols = SymbolTable::new();
        let bytes = encode_line(&parsed, &symbols, 0, 1).unwrap();
        assert_eq!(bytes, &[0x41, 0x42]);
    }

    #[test]
    fn encode_directive_zero() {
        let parsed = parse_line(".zero 4", 1).unwrap();
        let symbols = SymbolTable::new();
        let bytes = encode_line(&parsed, &symbols, 0, 1).unwrap();
        assert_eq!(bytes, &[0, 0, 0, 0]);
    }

    #[test]
    fn encode_directive_org_forward() {
        let parsed = parse_line(".org 0x100", 1).unwrap();
        let symbols = SymbolTable::new();
        let bytes = encode_line(&parsed, &symbols, 0x50, 1).unwrap();
        assert_eq!(bytes.len(), 0xB0);
        assert!(bytes.iter().all(|&b| b == 0));
    }

    #[test]
    fn roundtrip_nop_through_decoder() {
        let parsed = parse_line("NOP", 1).unwrap();
        let symbols = SymbolTable::new();
        let bytes = encode_line(&parsed, &symbols, 0, 1).unwrap();
        let word = u16::from_be_bytes([bytes[0], bytes[1]]);

        let decoded = Decoder::decode(word);
        match decoded {
            DecodedOrFault::Instruction(instr) => {
                assert_eq!(instr.encoding, OpcodeEncoding::Nop);
            }
            DecodedOrFault::Fault(_) => panic!("NOP should decode successfully"),
        }
    }

    #[test]
    fn roundtrip_mov_register_through_decoder() {
        let parsed = parse_line("MOV R2, R5", 1).unwrap();
        let symbols = SymbolTable::new();
        let bytes = encode_line(&parsed, &symbols, 0, 1).unwrap();
        let word = u16::from_be_bytes([bytes[0], bytes[1]]);

        let decoded = Decoder::decode(word);
        match decoded {
            DecodedOrFault::Instruction(instr) => {
                assert_eq!(instr.encoding, OpcodeEncoding::Mov);
                assert_eq!(instr.rd, Some(emulator_core::decoder::RegisterField::R2));
            }
            DecodedOrFault::Fault(_) => panic!("MOV should decode successfully"),
        }
    }

    #[test]
    fn roundtrip_add_through_decoder() {
        let parsed = parse_line("ADD R1, R2, R3", 1).unwrap();
        let symbols = SymbolTable::new();
        let bytes = encode_line(&parsed, &symbols, 0, 1).unwrap();
        let word = u16::from_be_bytes([bytes[0], bytes[1]]);

        let decoded = Decoder::decode(word);
        match decoded {
            DecodedOrFault::Instruction(instr) => {
                assert_eq!(instr.encoding, OpcodeEncoding::Add);
                assert_eq!(instr.rd, Some(emulator_core::decoder::RegisterField::R1));
                assert_eq!(instr.ra, Some(emulator_core::decoder::RegisterField::R2));
            }
            DecodedOrFault::Fault(_) => panic!("ADD should decode successfully"),
        }
    }

    #[test]
    fn roundtrip_halt_through_decoder() {
        let parsed = parse_line("HALT", 1).unwrap();
        let symbols = SymbolTable::new();
        let bytes = encode_line(&parsed, &symbols, 0, 1).unwrap();
        let word = u16::from_be_bytes([bytes[0], bytes[1]]);

        let decoded = Decoder::decode(word);
        match decoded {
            DecodedOrFault::Instruction(instr) => {
                assert_eq!(instr.encoding, OpcodeEncoding::Halt);
            }
            DecodedOrFault::Fault(_) => panic!("HALT should decode successfully"),
        }
    }

    #[test]
    fn error_displacement_out_of_range() {
        let parsed = parse_line("LOAD R0, [R1 + 200]", 1).unwrap();
        let symbols = SymbolTable::new();
        let result = encode_line(&parsed, &symbols, 0, 1);
        assert!(matches!(
            result,
            Err(EncodeError {
                kind: EncodeErrorKind::DisplacementOutOfRange(_),
                ..
            })
        ));
    }

    #[test]
    fn error_undefined_label() {
        let parsed = parse_line("JMP #nonexistent", 1).unwrap();
        let symbols = SymbolTable::new();
        let result = encode_line(&parsed, &symbols, 0, 1);
        assert!(matches!(
            result,
            Err(EncodeError {
                kind: EncodeErrorKind::UndefinedLabel(_),
                ..
            })
        ));
    }

    #[test]
    fn encode_all_branch_instructions() {
        let mnemonics = ["BEQ", "BNE", "BLT", "BLE", "BGT", "BGE"];
        let mut symbols = SymbolTable::new();
        symbols.insert(
            "target".to_string(),
            crate::symbols::Symbol {
                address: 0x0010,
                defined_at: 1,
            },
        );

        for mnemonic in mnemonics {
            let parsed = parse_line(&format!("{mnemonic} #target"), 1).unwrap();
            let bytes = encode_line(&parsed, &symbols, 0x0000, 1).unwrap();
            assert_eq!(bytes.len(), 4, "{mnemonic} should have extension word");
            let primary = u16::from_be_bytes([bytes[0], bytes[1]]);
            assert_eq!((primary >> 12) & 0xF, 0x6, "{mnemonic} should have OP=6");
            assert_eq!(
                primary & 0x7,
                u16::from(am::PC_RELATIVE),
                "{mnemonic} should use PC-relative"
            );
        }
    }

    #[test]
    fn encode_push_pop() {
        let parsed = parse_line("PUSH R3", 1).unwrap();
        let symbols = SymbolTable::new();
        let bytes = encode_line(&parsed, &symbols, 0, 1).unwrap();
        assert_eq!(bytes.len(), 2);
        let word = u16::from_be_bytes([bytes[0], bytes[1]]);
        assert_eq!((word >> 12) & 0xF, 0x7);
        assert_eq!((word >> 3) & 0x7, 0x0);

        let parsed = parse_line("POP R5", 1).unwrap();
        let bytes = encode_line(&parsed, &symbols, 0, 1).unwrap();
        let word = u16::from_be_bytes([bytes[0], bytes[1]]);
        assert_eq!((word >> 12) & 0xF, 0x7);
        assert_eq!((word >> 3) & 0x7, 0x1);
    }

    #[test]
    fn encode_load_absolute_address() {
        let parsed = parse_line("LOAD R0, #0x4000", 1).unwrap();
        let symbols = SymbolTable::new();
        let bytes = encode_line(&parsed, &symbols, 0, 1).unwrap();
        assert_eq!(bytes.len(), 4);
        let primary = u16::from_be_bytes([bytes[0], bytes[1]]);
        let extension = u16::from_be_bytes([bytes[2], bytes[3]]);
        assert_eq!((primary >> 12) & 0xF, 0x2);
        assert_eq!(primary & 0x7, u16::from(am::ZERO_EXTENDED_DISPLACEMENT));
        assert_eq!(extension, 0x4000);
    }

    #[test]
    fn encode_store_absolute_address() {
        let parsed = parse_line("STORE R1, #0x5000", 1).unwrap();
        let symbols = SymbolTable::new();
        let bytes = encode_line(&parsed, &symbols, 0, 1).unwrap();
        assert_eq!(bytes.len(), 4);
        let primary = u16::from_be_bytes([bytes[0], bytes[1]]);
        let extension = u16::from_be_bytes([bytes[2], bytes[3]]);
        assert_eq!((primary >> 12) & 0xF, 0x3);
        assert_eq!(primary & 0x7, u16::from(am::ZERO_EXTENDED_DISPLACEMENT));
        assert_eq!(extension, 0x5000);
    }

    #[test]
    fn roundtrip_load_indirect_through_decoder() {
        let parsed = parse_line("LOAD R4, [R5]", 1).unwrap();
        let symbols = SymbolTable::new();
        let bytes = encode_line(&parsed, &symbols, 0, 1).unwrap();
        let word = u16::from_be_bytes([bytes[0], bytes[1]]);

        let decoded = Decoder::decode(word);
        match decoded {
            DecodedOrFault::Instruction(instr) => {
                assert_eq!(instr.encoding, OpcodeEncoding::Load);
                assert_eq!(instr.rd, Some(emulator_core::decoder::RegisterField::R4));
                assert_eq!(instr.ra, Some(emulator_core::decoder::RegisterField::R5));
            }
            DecodedOrFault::Fault(_) => panic!("LOAD should decode successfully"),
        }
    }

    #[test]
    fn roundtrip_jmp_through_decoder() {
        let mut symbols = SymbolTable::new();
        symbols.insert(
            "dest".to_string(),
            crate::symbols::Symbol {
                address: 0x0020,
                defined_at: 1,
            },
        );

        let parsed = parse_line("JMP #dest", 1).unwrap();
        let bytes = encode_line(&parsed, &symbols, 0x0000, 1).unwrap();
        let primary = u16::from_be_bytes([bytes[0], bytes[1]]);

        let decoded = Decoder::decode(primary);
        match decoded {
            DecodedOrFault::Instruction(instr) => {
                assert_eq!(instr.encoding, OpcodeEncoding::Jmp);
            }
            DecodedOrFault::Fault(_) => panic!("JMP should decode successfully"),
        }
    }

    #[test]
    fn encode_call_ret() {
        let mut symbols = SymbolTable::new();
        symbols.insert(
            "subroutine".to_string(),
            crate::symbols::Symbol {
                address: 0x0100,
                defined_at: 1,
            },
        );

        let parsed = parse_line("CALL #subroutine", 1).unwrap();
        let bytes = encode_line(&parsed, &symbols, 0x0000, 1).unwrap();
        assert_eq!(bytes.len(), 4);
        let primary = u16::from_be_bytes([bytes[0], bytes[1]]);
        assert_eq!((primary >> 12) & 0xF, 0x6);
        assert_eq!((primary >> 3) & 0x7, 0x7);
        assert_eq!(primary & 0x7, u16::from(am::PC_RELATIVE));

        let parsed = parse_line("RET", 1).unwrap();
        let bytes = encode_line(&parsed, &symbols, 0, 1).unwrap();
        assert_eq!(bytes.len(), 2);
        let word = u16::from_be_bytes([bytes[0], bytes[1]]);
        assert_eq!((word >> 12) & 0xF, 0x6);
        assert_eq!((word >> 3) & 0x7, 0x7);
    }
}
