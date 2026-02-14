//! Assembly source line parser for instructions, labels, and directives.
//!
//! This module implements Pass 1 frontend parsing: converting raw source lines
//! into structured `ParsedLine` items ready for symbol table construction and
//! encoding.

use emulator_core::OpcodeEncoding;

use crate::mnemonic::{resolve_mnemonic_with_operand_form, MnemonicResolution};

/// A parsed register operand (R0-R7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Register(pub u8);

impl Register {
    #[allow(clippy::match_bool)]
    const fn new(n: u8) -> Option<Self> {
        match n <= 7 {
            true => Some(Self(n)),
            false => None,
        }
    }
}

/// An immediate or address value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Immediate {
    /// The numeric value (0 for unresolved labels).
    pub value: i64,
    /// Whether this is a label reference (resolved in pass 2).
    pub is_label: bool,
    /// The label name if this is a label reference.
    pub label_name: Option<String>,
}

/// A memory operand with optional displacement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryOperand {
    /// Base register for addressing.
    pub base: Register,
    /// Optional signed displacement (-128 to +127).
    pub displacement: Option<i16>,
}

/// Parsed operand forms.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Operand {
    /// Register direct (R0-R7).
    Register(Register),
    /// Immediate value or label reference.
    Immediate(Immediate),
    /// Memory operand with optional displacement.
    Memory(MemoryOperand),
}

/// Instruction size in words (1 or 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstructionSize {
    /// Single word instruction (no extension).
    OneWord,
    /// Two word instruction (requires extension word).
    TwoWords,
}

/// A parsed instruction with all operands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedInstruction {
    /// The mnemonic string as written.
    pub mnemonic: String,
    /// Resolved (OP, SUB, `OpcodeEncoding`) from the mnemonic table.
    pub resolution: MnemonicResolution,
    /// Destination register (RD field).
    pub rd: Option<Register>,
    /// Source/Address register (RA field).
    pub ra: Option<Register>,
    /// The third operand (RB, immediate, or memory).
    pub operand: Option<Operand>,
    /// Determined size (1 or 2 words).
    pub size: InstructionSize,
}

/// A parsed data directive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Directive {
    /// `.org addr` - set output position.
    Org(u32),
    /// `.word val` - emit 16-bit value (big-endian).
    Word(u16),
    /// `.byte val` - emit 8-bit value.
    Byte(u8),
    /// `.ascii "str"` - emit ASCII bytes.
    Ascii(String),
    /// `.zero count` - emit N zero bytes.
    Zero(usize),
    /// `.include "path"` - include another source file.
    Include(String),
}

/// A single parsed source line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParsedLine {
    /// Empty or comment-only line.
    Blank,
    /// Label definition.
    Label {
        /// Label name.
        name: String,
    },
    /// Data directive.
    Directive {
        /// The parsed directive.
        directive: Directive,
    },
    /// Instruction line.
    Instruction {
        /// The parsed instruction.
        instruction: ParsedInstruction,
    },
}

/// Source location for error reporting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceLocation {
    /// 1-indexed line number.
    pub line: usize,
    /// 1-indexed column number.
    pub column: usize,
}

/// Parse error with source location.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    /// Location of the error.
    pub location: SourceLocation,
    /// Kind of parse error.
    pub kind: ParseErrorKind,
}

/// Classification of parse errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseErrorKind {
    /// Unknown or invalid mnemonic.
    UnknownMnemonic(String),
    /// Invalid register name (not R0-R7).
    InvalidRegister(String),
    /// Duplicate label definition.
    DuplicateLabel(String),
    /// Malformed immediate value.
    InvalidImmediate(String),
    /// Displacement out of signed 8-bit range.
    InvalidDisplacement(String),
    /// Unknown directive name.
    InvalidDirective(String),
    /// Invalid value for directive.
    InvalidDirectiveValue(String),
    /// General syntax error.
    InvalidSyntax(String),
    /// String literal missing closing quote.
    UnterminatedString,
    /// Operand provided where none expected.
    UnexpectedOperand,
    /// Required operand missing.
    MissingOperand,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.kind)
    }
}

impl std::fmt::Display for ParseErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownMnemonic(m) => write!(f, "unknown mnemonic: {m}"),
            Self::InvalidRegister(r) => write!(f, "invalid register: {r}"),
            Self::DuplicateLabel(l) => write!(f, "duplicate label: {l}"),
            Self::InvalidImmediate(v) => write!(f, "invalid immediate value: {v}"),
            Self::InvalidDisplacement(d) => write!(f, "displacement out of range: {d}"),
            Self::InvalidDirective(d) => write!(f, "unknown directive: {d}"),
            Self::InvalidDirectiveValue(v) => write!(f, "invalid directive value: {v}"),
            Self::InvalidSyntax(s) => write!(f, "invalid syntax: {s}"),
            Self::UnterminatedString => write!(f, "unterminated string literal"),
            Self::UnexpectedOperand => write!(f, "unexpected operand"),
            Self::MissingOperand => write!(f, "missing operand"),
        }
    }
}

impl std::error::Error for ParseError {}

/// Result of parsing a single line.
pub type ParseResult = Result<ParsedLine, ParseError>;

/// Parses a source line into a `ParsedLine`.
///
/// # Errors
///
/// Returns a `ParseError` if the line contains invalid syntax, unknown
/// mnemonics, malformed operands, or other parse-time errors.
#[allow(clippy::too_many_lines)]
pub fn parse_line(line: &str, line_number: usize) -> ParseResult {
    let stripped = strip_comment(line);
    let trimmed = stripped.trim();

    if trimmed.is_empty() {
        return Ok(ParsedLine::Blank);
    }

    if let Some((label, rest)) = split_label(trimmed) {
        if rest.trim().is_empty() {
            return Ok(ParsedLine::Label { name: label });
        }
        let directive_or_instruction = parse_directive_or_instruction(rest.trim(), line_number)?;
        match directive_or_instruction {
            ParsedLine::Directive { directive } => {
                return Ok(ParsedLine::Directive { directive });
            }
            ParsedLine::Instruction { instruction } => {
                return Ok(ParsedLine::Instruction { instruction });
            }
            _ => {}
        }
    }

    parse_directive_or_instruction(trimmed, line_number)
}

fn strip_comment(line: &str) -> &str {
    line.find(';').map_or(line, |pos| &line[..pos])
}

fn split_label(text: &str) -> Option<(String, &str)> {
    let colon_pos = text.find(':')?;
    let label = text[..colon_pos].trim();
    is_valid_label(label).then(|| (label.to_string(), &text[colon_pos + 1..]))
}

fn is_valid_label(s: &str) -> bool {
    let mut chars = s.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !first.is_ascii_alphabetic() && first != '_' {
        return false;
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn parse_directive_or_instruction(text: &str, line_number: usize) -> ParseResult {
    if text.starts_with('.') {
        parse_directive(text, line_number)
    } else {
        parse_instruction(text, line_number)
    }
}

fn parse_directive(text: &str, line_number: usize) -> ParseResult {
    let without_dot = &text[1..];
    let (name, args) = split_directive(without_dot);

    let directive = match name.to_ascii_lowercase().as_str() {
        "org" => {
            let addr = parse_u32_value(args, line_number)?;
            Directive::Org(addr)
        }
        "word" => {
            let val = parse_u16_value(args, line_number)?;
            Directive::Word(val)
        }
        "byte" => {
            let val = parse_u8_value(args, line_number)?;
            Directive::Byte(val)
        }
        "ascii" => {
            let s = parse_string_literal(args, line_number)?;
            Directive::Ascii(s)
        }
        "zero" => {
            let count = parse_usize_value(args, line_number)?;
            Directive::Zero(count)
        }
        "include" => {
            let path = parse_include_path(args, line_number)?;
            Directive::Include(path)
        }
        _ => {
            return Err(ParseError {
                location: SourceLocation {
                    line: line_number,
                    column: 1,
                },
                kind: ParseErrorKind::InvalidDirective(name.to_string()),
            });
        }
    };

    Ok(ParsedLine::Directive { directive })
}

fn split_directive(text: &str) -> (&str, &str) {
    text.find(|c: char| c.is_whitespace())
        .map_or((text, ""), |pos| (&text[..pos], text[pos..].trim()))
}

fn parse_u32_value(s: &str, line: usize) -> Result<u32, ParseError> {
    parse_numeric_value(s, line).and_then(|v| {
        u32::try_from(v).map_err(|_| ParseError {
            location: SourceLocation { line, column: 1 },
            kind: ParseErrorKind::InvalidDirectiveValue(s.to_string()),
        })
    })
}

fn parse_u16_value(s: &str, line: usize) -> Result<u16, ParseError> {
    parse_numeric_value(s, line).and_then(|v| {
        u16::try_from(v).map_err(|_| ParseError {
            location: SourceLocation { line, column: 1 },
            kind: ParseErrorKind::InvalidDirectiveValue(s.to_string()),
        })
    })
}

fn parse_u8_value(s: &str, line: usize) -> Result<u8, ParseError> {
    parse_numeric_value(s, line).and_then(|v| {
        u8::try_from(v).map_err(|_| ParseError {
            location: SourceLocation { line, column: 1 },
            kind: ParseErrorKind::InvalidDirectiveValue(s.to_string()),
        })
    })
}

fn parse_usize_value(s: &str, line: usize) -> Result<usize, ParseError> {
    parse_numeric_value(s, line).and_then(|v| {
        usize::try_from(v).map_err(|_| ParseError {
            location: SourceLocation { line, column: 1 },
            kind: ParseErrorKind::InvalidDirectiveValue(s.to_string()),
        })
    })
}

fn parse_string_literal(s: &str, line: usize) -> Result<String, ParseError> {
    let trimmed = s.trim();
    if !trimmed.starts_with('"') {
        return Err(ParseError {
            location: SourceLocation { line, column: 1 },
            kind: ParseErrorKind::InvalidDirectiveValue("expected string literal".into()),
        });
    }

    let end_quote = trimmed[1..].find('"');
    end_quote.map_or(
        Err(ParseError {
            location: SourceLocation { line, column: 1 },
            kind: ParseErrorKind::UnterminatedString,
        }),
        |pos| Ok(trimmed[1..=pos].to_string()),
    )
}

fn parse_include_path(s: &str, line: usize) -> Result<String, ParseError> {
    parse_string_literal(s, line)
}

fn parse_instruction(text: &str, line_number: usize) -> ParseResult {
    let tokens = tokenize(text);
    if tokens.is_empty() {
        return Err(ParseError {
            location: SourceLocation {
                line: line_number,
                column: 1,
            },
            kind: ParseErrorKind::InvalidSyntax("empty instruction".into()),
        });
    }

    let mnemonic = &tokens[0];
    let operand_tokens = &tokens[1..];

    let has_operand = !operand_tokens.is_empty();
    let resolution =
        resolve_mnemonic_with_operand_form(mnemonic, has_operand).ok_or_else(|| ParseError {
            location: SourceLocation {
                line: line_number,
                column: 1,
            },
            kind: ParseErrorKind::UnknownMnemonic(mnemonic.clone()),
        })?;

    let (rd, ra, operand) = parse_operands(operand_tokens, resolution.2, line_number)?;

    let size = determine_instruction_size(operand.as_ref());

    Ok(ParsedLine::Instruction {
        instruction: ParsedInstruction {
            mnemonic: mnemonic.clone(),
            resolution,
            rd,
            ra,
            operand,
            size,
        },
    })
}

fn tokenize(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_bracket = false;
    let mut in_string = false;

    for ch in text.chars() {
        match ch {
            '"' if !in_bracket => {
                in_string = !in_string;
                current.push(ch);
            }
            '[' if !in_string => {
                in_bracket = true;
                current.push(ch);
            }
            ']' if !in_string => {
                in_bracket = false;
                current.push(ch);
            }
            ',' | ' ' | '\t' if !in_bracket && !in_string => {
                if !current.is_empty() {
                    tokens.push(current.clone());
                    current.clear();
                }
            }
            _ => {
                current.push(ch);
            }
        }
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    tokens
}

type OperandResult = Result<(Option<Register>, Option<Register>, Option<Operand>), ParseError>;

#[allow(clippy::too_many_lines)]
fn parse_operands(
    tokens: &[String],
    encoding: OpcodeEncoding,
    line_number: usize,
) -> OperandResult {
    if tokens.is_empty() {
        return Ok((None, None, None));
    }

    match encoding {
        OpcodeEncoding::Nop
        | OpcodeEncoding::Sync
        | OpcodeEncoding::Halt
        | OpcodeEncoding::Trap
        | OpcodeEncoding::Swi => {
            if !tokens.is_empty() {
                return Err(ParseError {
                    location: SourceLocation {
                        line: line_number,
                        column: 1,
                    },
                    kind: ParseErrorKind::UnexpectedOperand,
                });
            }
            Ok((None, None, None))
        }
        OpcodeEncoding::Push | OpcodeEncoding::Pop | OpcodeEncoding::Eget => {
            let reg = parse_register(tokens[0].as_str(), line_number)?;
            Ok((Some(reg), None, None))
        }
        OpcodeEncoding::Jmp
        | OpcodeEncoding::Beq
        | OpcodeEncoding::Bne
        | OpcodeEncoding::Blt
        | OpcodeEncoding::Ble
        | OpcodeEncoding::Bgt
        | OpcodeEncoding::Bge => {
            let operand = parse_operand(&tokens[0], line_number)?;
            Ok((None, None, Some(operand)))
        }
        OpcodeEncoding::CallOrRet => {
            if tokens.is_empty() {
                Ok((None, None, None))
            } else {
                let operand = parse_operand(&tokens[0], line_number)?;
                Ok((None, None, Some(operand)))
            }
        }
        OpcodeEncoding::Mov | OpcodeEncoding::Load | OpcodeEncoding::Store => {
            let rd = parse_register(tokens[0].as_str(), line_number)?;
            let operand = if tokens.len() > 1 {
                Some(parse_operand(&tokens[1], line_number)?)
            } else {
                None
            };
            Ok((Some(rd), None, operand))
        }
        OpcodeEncoding::In => {
            let rd = parse_register(tokens[0].as_str(), line_number)?;
            let ra = if tokens.len() > 1 {
                Some(parse_register(tokens[1].as_str(), line_number)?)
            } else {
                None
            };
            Ok((Some(rd), ra, None))
        }
        OpcodeEncoding::Out => {
            let ra = parse_register(tokens[0].as_str(), line_number)?;
            let rd = if tokens.len() > 1 {
                Some(parse_register(tokens[1].as_str(), line_number)?)
            } else {
                None
            };
            Ok((rd, Some(ra), None))
        }
        OpcodeEncoding::Bset | OpcodeEncoding::Bclr | OpcodeEncoding::Btest => {
            let ra = parse_register(tokens[0].as_str(), line_number)?;
            if tokens.len() > 1 {
                let operand = parse_operand(&tokens[1], line_number)?;
                Ok((None, Some(ra), Some(operand)))
            } else {
                Ok((None, Some(ra), None))
            }
        }
        OpcodeEncoding::Ewait | OpcodeEncoding::Eret => Ok((None, None, None)),
        OpcodeEncoding::Add
        | OpcodeEncoding::Sub
        | OpcodeEncoding::And
        | OpcodeEncoding::Or
        | OpcodeEncoding::Xor
        | OpcodeEncoding::Shl
        | OpcodeEncoding::Shr
        | OpcodeEncoding::Cmp
        | OpcodeEncoding::Mul
        | OpcodeEncoding::Mulh
        | OpcodeEncoding::Div
        | OpcodeEncoding::Mod
        | OpcodeEncoding::Qadd
        | OpcodeEncoding::Qsub
        | OpcodeEncoding::Scv => {
            let rd = parse_register(tokens[0].as_str(), line_number)?;
            let ra = if tokens.len() > 1 {
                Some(parse_register(tokens[1].as_str(), line_number)?)
            } else {
                None
            };
            let operand = if tokens.len() > 2 {
                Some(parse_operand(&tokens[2], line_number)?)
            } else {
                None
            };
            Ok((Some(rd), ra, operand))
        }
    }
}

fn parse_register(s: &str, line_number: usize) -> Result<Register, ParseError> {
    let upper = s.to_ascii_uppercase();
    if let Some(num_str) = upper.strip_prefix('R') {
        if let Ok(num) = num_str.parse::<u8>() {
            return Register::new(num).ok_or_else(|| ParseError {
                location: SourceLocation {
                    line: line_number,
                    column: 1,
                },
                kind: ParseErrorKind::InvalidRegister(s.to_string()),
            });
        }
    }
    Err(ParseError {
        location: SourceLocation {
            line: line_number,
            column: 1,
        },
        kind: ParseErrorKind::InvalidRegister(s.to_string()),
    })
}

fn parse_operand(s: &str, line_number: usize) -> Result<Operand, ParseError> {
    if s.starts_with('[') && s.ends_with(']') {
        return parse_memory_operand(s, line_number);
    }

    if let Some(stripped) = s.strip_prefix('#') {
        return parse_immediate(stripped, line_number);
    }

    parse_register(s, line_number).map(Operand::Register)
}

fn parse_memory_operand(s: &str, line_number: usize) -> Result<Operand, ParseError> {
    let inner = &s[1..s.len() - 1];
    let inner = inner.trim();

    if let Some(plus_pos) = inner.find('+') {
        let ra_str = inner[..plus_pos].trim();
        let disp_str = inner[plus_pos + 1..].trim();
        let base = parse_register(ra_str, line_number)?;
        let disp = parse_displacement(disp_str, line_number)?;
        Ok(Operand::Memory(MemoryOperand {
            base,
            displacement: Some(disp),
        }))
    } else if let Some(minus_pos) = inner.find('-') {
        let ra_str = inner[..minus_pos].trim();
        let disp_str = inner[minus_pos + 1..].trim();
        let base = parse_register(ra_str, line_number)?;
        let disp_val = parse_numeric_value(disp_str, line_number)?;
        let negated = disp_val
            .checked_neg()
            .filter(|&v| v >= i64::from(i16::MIN))
            .and_then(|v| i16::try_from(v).ok())
            .ok_or_else(|| ParseError {
                location: SourceLocation {
                    line: line_number,
                    column: 1,
                },
                kind: ParseErrorKind::InvalidDisplacement(disp_str.to_string()),
            })?;
        Ok(Operand::Memory(MemoryOperand {
            base,
            displacement: Some(negated),
        }))
    } else {
        let base = parse_register(inner, line_number)?;
        Ok(Operand::Memory(MemoryOperand {
            base,
            displacement: None,
        }))
    }
}

fn parse_displacement(s: &str, line_number: usize) -> Result<i16, ParseError> {
    let val = parse_numeric_value(s, line_number)?;
    i16::try_from(val).map_err(|_| ParseError {
        location: SourceLocation {
            line: line_number,
            column: 1,
        },
        kind: ParseErrorKind::InvalidDisplacement(s.to_string()),
    })
}

fn parse_immediate(s: &str, line_number: usize) -> Result<Operand, ParseError> {
    if is_valid_label(s) {
        return Ok(Operand::Immediate(Immediate {
            value: 0,
            is_label: true,
            label_name: Some(s.to_string()),
        }));
    }

    let val = parse_numeric_value(s, line_number)?;
    Ok(Operand::Immediate(Immediate {
        value: val,
        is_label: false,
        label_name: None,
    }))
}

#[allow(clippy::option_if_let_else)]
fn parse_numeric_value(s: &str, line_number: usize) -> Result<i64, ParseError> {
    let s = s.trim();
    let err = || ParseError {
        location: SourceLocation {
            line: line_number,
            column: 1,
        },
        kind: ParseErrorKind::InvalidImmediate(s.to_string()),
    };

    match s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        Some(hex) => i64::from_str_radix(hex, 16).map_err(|_| err()),
        None => match s.strip_prefix("0b").or_else(|| s.strip_prefix("0B")) {
            Some(bin) => i64::from_str_radix(bin, 2).map_err(|_| err()),
            None => s.parse::<i64>().map_err(|_| err()),
        },
    }
}

const fn determine_instruction_size(operand: Option<&Operand>) -> InstructionSize {
    match operand {
        None | Some(Operand::Register(_)) => InstructionSize::OneWord,
        Some(Operand::Memory(mem)) => {
            if mem.displacement.is_some() {
                InstructionSize::TwoWords
            } else {
                InstructionSize::OneWord
            }
        }
        Some(Operand::Immediate(_)) => InstructionSize::TwoWords,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_blank_line() {
        assert_eq!(parse_line("", 1), Ok(ParsedLine::Blank));
        assert_eq!(parse_line("   ", 1), Ok(ParsedLine::Blank));
        assert_eq!(parse_line("; comment", 1), Ok(ParsedLine::Blank));
        assert_eq!(parse_line("  ; comment only  ", 1), Ok(ParsedLine::Blank));
    }

    #[test]
    fn parse_label_only() {
        let result = parse_line("start:", 1);
        assert_eq!(
            result,
            Ok(ParsedLine::Label {
                name: "start".into()
            })
        );

        let result = parse_line("  loop:  ", 1);
        assert_eq!(
            result,
            Ok(ParsedLine::Label {
                name: "loop".into()
            })
        );
    }

    #[test]
    fn parse_label_with_instruction() {
        let result = parse_line("init: MOV R0, #1", 1);
        match result {
            Ok(ParsedLine::Instruction { instruction }) => {
                assert_eq!(instruction.mnemonic, "MOV");
                assert!(instruction.rd.is_some());
            }
            _ => panic!("expected instruction"),
        }
    }

    #[test]
    fn parse_nop() {
        let result = parse_line("NOP", 1);
        match result {
            Ok(ParsedLine::Instruction { instruction }) => {
                assert_eq!(instruction.mnemonic, "NOP");
                assert_eq!(instruction.size, InstructionSize::OneWord);
                assert!(instruction.rd.is_none());
                assert!(instruction.operand.is_none());
            }
            _ => panic!("expected instruction"),
        }
    }

    #[test]
    fn parse_halt() {
        let result = parse_line("HALT", 1);
        match result {
            Ok(ParsedLine::Instruction { instruction }) => {
                assert_eq!(instruction.mnemonic, "HALT");
                assert_eq!(instruction.size, InstructionSize::OneWord);
            }
            _ => panic!("expected instruction"),
        }
    }

    #[test]
    fn parse_mov_immediate() {
        let result = parse_line("MOV R0, #42", 1);
        match result {
            Ok(ParsedLine::Instruction { instruction }) => {
                assert_eq!(instruction.mnemonic, "MOV");
                assert_eq!(instruction.rd, Some(Register(0)));
                match instruction.operand {
                    Some(Operand::Immediate(imm)) => {
                        assert_eq!(imm.value, 42);
                        assert!(!imm.is_label);
                    }
                    _ => panic!("expected immediate operand"),
                }
                assert_eq!(instruction.size, InstructionSize::TwoWords);
            }
            _ => panic!("expected instruction"),
        }
    }

    #[test]
    fn parse_mov_hex_immediate() {
        let result = parse_line("MOV R1, #0x4000", 1);
        match result {
            Ok(ParsedLine::Instruction { instruction }) => {
                assert_eq!(instruction.rd, Some(Register(1)));
                match instruction.operand {
                    Some(Operand::Immediate(imm)) => {
                        assert_eq!(imm.value, 0x4000);
                    }
                    _ => panic!("expected immediate"),
                }
            }
            _ => panic!("expected instruction"),
        }
    }

    #[test]
    fn parse_mov_binary_immediate() {
        let result = parse_line("MOV R2, #0b1010", 1);
        match result {
            Ok(ParsedLine::Instruction { instruction }) => {
                assert_eq!(instruction.rd, Some(Register(2)));
                match instruction.operand {
                    Some(Operand::Immediate(imm)) => {
                        assert_eq!(imm.value, 0b1010);
                    }
                    _ => panic!("expected immediate"),
                }
            }
            _ => panic!("expected instruction"),
        }
    }

    #[test]
    fn parse_mov_register() {
        let result = parse_line("MOV R0, R1", 1);
        match result {
            Ok(ParsedLine::Instruction { instruction }) => {
                assert_eq!(instruction.rd, Some(Register(0)));
                match instruction.operand {
                    Some(Operand::Register(reg)) => assert_eq!(reg, Register(1)),
                    _ => panic!("expected register operand"),
                }
                assert_eq!(instruction.size, InstructionSize::OneWord);
            }
            _ => panic!("expected instruction"),
        }
    }

    #[test]
    fn parse_add_three_registers() {
        let result = parse_line("ADD R0, R1, R2", 1);
        match result {
            Ok(ParsedLine::Instruction { instruction }) => {
                assert_eq!(instruction.mnemonic, "ADD");
                assert_eq!(instruction.rd, Some(Register(0)));
                assert_eq!(instruction.ra, Some(Register(1)));
                match instruction.operand {
                    Some(Operand::Register(reg)) => assert_eq!(reg, Register(2)),
                    _ => panic!("expected register operand"),
                }
            }
            _ => panic!("expected instruction"),
        }
    }

    #[test]
    fn parse_store_indirect() {
        let result = parse_line("STORE R3, [R1]", 1);
        match result {
            Ok(ParsedLine::Instruction { instruction }) => {
                assert_eq!(instruction.mnemonic, "STORE");
                assert_eq!(instruction.rd, Some(Register(3)));
                match instruction.operand {
                    Some(Operand::Memory(mem)) => {
                        assert_eq!(mem.base, Register(1));
                        assert!(mem.displacement.is_none());
                    }
                    _ => panic!("expected memory operand"),
                }
                assert_eq!(instruction.size, InstructionSize::OneWord);
            }
            _ => panic!("expected instruction"),
        }
    }

    #[test]
    fn parse_load_with_displacement() {
        let result = parse_line("LOAD R0, [R1 + 10]", 1);
        match result {
            Ok(ParsedLine::Instruction { instruction }) => {
                assert_eq!(instruction.mnemonic, "LOAD");
                assert_eq!(instruction.rd, Some(Register(0)));
                match instruction.operand {
                    Some(Operand::Memory(mem)) => {
                        assert_eq!(mem.base, Register(1));
                        assert_eq!(mem.displacement, Some(10));
                    }
                    _ => panic!("expected memory operand"),
                }
                assert_eq!(instruction.size, InstructionSize::TwoWords);
            }
            _ => panic!("expected instruction"),
        }
    }

    #[test]
    fn parse_load_with_negative_displacement() {
        let result = parse_line("LOAD R0, [R1 - 5]", 1);
        match result {
            Ok(ParsedLine::Instruction { instruction }) => match instruction.operand {
                Some(Operand::Memory(mem)) => {
                    assert_eq!(mem.displacement, Some(-5));
                }
                _ => panic!("expected memory operand"),
            },
            _ => panic!("expected instruction"),
        }
    }

    #[test]
    fn parse_jmp_label() {
        let result = parse_line("JMP #main", 1);
        match result {
            Ok(ParsedLine::Instruction { instruction }) => {
                assert_eq!(instruction.mnemonic, "JMP");
                match instruction.operand {
                    Some(Operand::Immediate(imm)) => {
                        assert!(imm.is_label);
                    }
                    _ => panic!("expected immediate/label operand"),
                }
            }
            _ => panic!("expected instruction"),
        }
    }

    #[test]
    fn parse_call() {
        let result = parse_line("CALL #subroutine", 1);
        match result {
            Ok(ParsedLine::Instruction { instruction }) => {
                assert_eq!(instruction.mnemonic, "CALL");
                assert_eq!(instruction.resolution.2, OpcodeEncoding::CallOrRet);
            }
            _ => panic!("expected instruction"),
        }
    }

    #[test]
    fn parse_ret() {
        let result = parse_line("RET", 1);
        match result {
            Ok(ParsedLine::Instruction { instruction }) => {
                assert_eq!(instruction.mnemonic, "RET");
                assert_eq!(instruction.resolution.2, OpcodeEncoding::CallOrRet);
                assert!(instruction.operand.is_none());
            }
            _ => panic!("expected instruction"),
        }
    }

    #[test]
    fn parse_push() {
        let result = parse_line("PUSH R0", 1);
        match result {
            Ok(ParsedLine::Instruction { instruction }) => {
                assert_eq!(instruction.mnemonic, "PUSH");
                assert_eq!(instruction.rd, Some(Register(0)));
            }
            _ => panic!("expected instruction"),
        }
    }

    #[test]
    fn parse_pop() {
        let result = parse_line("POP R7", 1);
        match result {
            Ok(ParsedLine::Instruction { instruction }) => {
                assert_eq!(instruction.mnemonic, "POP");
                assert_eq!(instruction.rd, Some(Register(7)));
            }
            _ => panic!("expected instruction"),
        }
    }

    #[test]
    fn parse_directive_org() {
        let result = parse_line(".org 0x100", 1);
        match result {
            Ok(ParsedLine::Directive { directive }) => {
                assert_eq!(directive, Directive::Org(0x100));
            }
            _ => panic!("expected directive"),
        }
    }

    #[test]
    fn parse_directive_word() {
        let result = parse_line(".word 0x1234", 1);
        match result {
            Ok(ParsedLine::Directive { directive }) => {
                assert_eq!(directive, Directive::Word(0x1234));
            }
            _ => panic!("expected directive"),
        }
    }

    #[test]
    fn parse_directive_byte() {
        let result = parse_line(".byte 255", 1);
        match result {
            Ok(ParsedLine::Directive { directive }) => {
                assert_eq!(directive, Directive::Byte(255));
            }
            _ => panic!("expected directive"),
        }
    }

    #[test]
    fn parse_directive_ascii() {
        let result = parse_line(".ascii \"hello\"", 1);
        match result {
            Ok(ParsedLine::Directive { directive }) => {
                assert_eq!(directive, Directive::Ascii("hello".into()));
            }
            _ => panic!("expected directive"),
        }
    }

    #[test]
    fn parse_directive_zero() {
        let result = parse_line(".zero 16", 1);
        match result {
            Ok(ParsedLine::Directive { directive }) => {
                assert_eq!(directive, Directive::Zero(16));
            }
            _ => panic!("expected directive"),
        }
    }

    #[test]
    fn parse_directive_include() {
        let result = parse_line(".include \"math.n1\"", 1);
        match result {
            Ok(ParsedLine::Directive { directive }) => {
                assert_eq!(directive, Directive::Include("math.n1".into()));
            }
            _ => panic!("expected directive"),
        }
    }

    #[test]
    fn parse_directive_include_with_path() {
        let result = parse_line(".include \"lib/utils.n1.md\"", 1);
        match result {
            Ok(ParsedLine::Directive { directive }) => {
                assert_eq!(directive, Directive::Include("lib/utils.n1.md".into()));
            }
            _ => panic!("expected directive"),
        }
    }

    #[test]
    fn parse_comment_stripped() {
        let result = parse_line("MOV R0, #1 ; this is a comment", 1);
        match result {
            Ok(ParsedLine::Instruction { instruction }) => {
                assert_eq!(instruction.mnemonic, "MOV");
            }
            _ => panic!("expected instruction"),
        }
    }

    #[test]
    fn error_unknown_mnemonic() {
        let result = parse_line("NOTREAL R0", 1);
        assert!(matches!(
            result,
            Err(ParseError {
                kind: ParseErrorKind::UnknownMnemonic(_),
                ..
            })
        ));
    }

    #[test]
    fn error_invalid_register() {
        let result = parse_line("MOV R8, #1", 1);
        assert!(matches!(
            result,
            Err(ParseError {
                kind: ParseErrorKind::InvalidRegister(_),
                ..
            })
        ));
    }

    #[test]
    fn error_invalid_immediate() {
        let result = parse_line("MOV R0, #123abc", 1);
        assert!(matches!(
            result,
            Err(ParseError {
                kind: ParseErrorKind::InvalidImmediate(_),
                ..
            })
        ));
    }

    #[test]
    fn error_unknown_directive() {
        let result = parse_line(".bogus 123", 1);
        assert!(matches!(
            result,
            Err(ParseError {
                kind: ParseErrorKind::InvalidDirective(_),
                ..
            })
        ));
    }

    #[test]
    fn case_insensitive_mnemonic() {
        let result = parse_line("mov r0, #1", 1);
        match result {
            Ok(ParsedLine::Instruction { instruction }) => {
                assert_eq!(instruction.mnemonic, "mov");
                assert!(instruction.resolution.0 == 0x1);
            }
            _ => panic!("expected instruction"),
        }
    }

    #[test]
    fn case_insensitive_register() {
        let result = parse_line("MOV r0, R1", 1);
        match result {
            Ok(ParsedLine::Instruction { instruction }) => {
                assert_eq!(instruction.rd, Some(Register(0)));
                match instruction.operand {
                    Some(Operand::Register(reg)) => assert_eq!(reg, Register(1)),
                    _ => panic!("expected register"),
                }
            }
            _ => panic!("expected instruction"),
        }
    }

    #[test]
    fn branch_with_label() {
        let result = parse_line("BEQ #target", 1);
        match result {
            Ok(ParsedLine::Instruction { instruction }) => {
                assert_eq!(instruction.mnemonic, "BEQ");
                match instruction.operand {
                    Some(Operand::Immediate(imm)) => {
                        assert!(imm.is_label);
                    }
                    _ => panic!("expected label reference"),
                }
            }
            _ => panic!("expected instruction"),
        }
    }

    #[test]
    fn label_with_underscore() {
        let result = parse_line("my_label:", 1);
        assert_eq!(
            result,
            Ok(ParsedLine::Label {
                name: "my_label".into()
            })
        );
    }

    #[test]
    fn label_starts_with_underscore() {
        let result = parse_line("_private:", 1);
        assert_eq!(
            result,
            Ok(ParsedLine::Label {
                name: "_private".into()
            })
        );
    }

    #[test]
    fn parse_xor_three_registers() {
        let result = parse_line("XOR R3, R3, R2", 1);
        match result {
            Ok(ParsedLine::Instruction { instruction }) => {
                assert_eq!(instruction.mnemonic, "XOR");
                assert_eq!(instruction.rd, Some(Register(3)));
                assert_eq!(instruction.ra, Some(Register(3)));
            }
            _ => panic!("expected instruction"),
        }
    }

    #[test]
    fn error_malformed_operand_unclosed_bracket() {
        let result = parse_line("LOAD R0, [R1", 1);
        assert!(result.is_err());
    }

    #[test]
    fn error_malformed_operand_invalid_displacement() {
        let result = parse_line("LOAD R0, [R1 + abc]", 1);
        assert!(result.is_err());
    }

    #[test]
    fn error_unexpected_operand_for_halt() {
        let result = parse_line("HALT R0", 1);
        assert!(matches!(
            result,
            Err(ParseError {
                kind: ParseErrorKind::UnexpectedOperand,
                ..
            })
        ));
    }
}
