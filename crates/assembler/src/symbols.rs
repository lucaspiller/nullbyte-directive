//! Symbol table and pass-1 address assignment.
//!
//! This module implements the first pass of assembly: walking parsed lines,
//! assigning addresses to each instruction/datum, and building a symbol table
//! of label definitions.

use std::collections::HashMap;

use crate::parser::{Directive, InstructionSize, ParsedLine};

/// A symbol (label) with its assigned address and definition location.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Symbol {
    /// The address assigned to this label.
    pub address: u16,
    /// Source line number where the label was defined.
    pub defined_at: usize,
}

/// Symbol table mapping label names to their definitions.
pub type SymbolTable = HashMap<String, Symbol>;

/// Error during symbol table construction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolError {
    /// Kind of error.
    pub kind: SymbolErrorKind,
    /// Source line where the error occurred.
    pub line: usize,
}

/// Classification of symbol errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SymbolErrorKind {
    /// Duplicate label definition.
    DuplicateLabel {
        /// The label name.
        name: String,
        /// Line of the first definition.
        first_definition: usize,
    },
    /// Address overflow (exceeded 0xFFFF).
    AddressOverflow {
        /// The address that would result.
        address: u32,
    },
    /// `.org` directive would move address backwards.
    OrgBackwards {
        /// Current address.
        current: u16,
        /// Requested address.
        requested: u32,
    },
}

impl std::fmt::Display for SymbolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.kind)
    }
}

impl std::fmt::Display for SymbolErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateLabel {
                name,
                first_definition,
            } => {
                write!(
                    f,
                    "duplicate label '{name}' (first defined at line {first_definition})"
                )
            }
            Self::AddressOverflow { address } => {
                write!(
                    f,
                    "address overflow: 0x{address:05X} exceeds 16-bit address space"
                )
            }
            Self::OrgBackwards { current, requested } => {
                write!(
                    f,
                    ".org would move address backwards: current=0x{current:04X}, requested=0x{requested:04X}"
                )
            }
        }
    }
}

impl std::error::Error for SymbolError {}

/// A line with its assigned address.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddressedLine {
    /// The address where this line's content begins.
    pub address: u16,
    /// The size in bytes of this line's content.
    pub size: u16,
    /// The parsed line content.
    pub parsed: ParsedLine,
    /// Original source line number.
    pub source_line: usize,
}

/// Result of pass-1 address assignment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Assignment {
    /// All lines with their assigned addresses.
    pub lines: Vec<AddressedLine>,
    /// Symbol table of label definitions.
    pub symbols: SymbolTable,
    /// Final address after all content (one past the last byte).
    pub end_address: u16,
}

/// Computes the byte size of a parsed line.
///
/// - Instructions: 2 bytes per word (1 or 2 words based on addressing mode)
/// - `.word`: 2 bytes
/// - `.byte`: 1 byte
/// - `.ascii`: string length in bytes
/// - `.zero`: count bytes
/// - `.org`: 0 bytes (affects position counter only)
/// - Labels/blank: 0 bytes
#[must_use]
pub const fn line_size(parsed: &ParsedLine) -> u16 {
    match parsed {
        ParsedLine::Blank | ParsedLine::Label { .. } => 0,
        ParsedLine::Directive { directive } => directive_size(directive),
        ParsedLine::Instruction { instruction } => match instruction.size {
            InstructionSize::OneWord => 2,
            InstructionSize::TwoWords => 4,
        },
    }
}

#[allow(clippy::cast_possible_truncation)]
const fn directive_size(directive: &Directive) -> u16 {
    match directive {
        Directive::Org(_) | Directive::Include(_) => 0,
        Directive::Word(_) => 2,
        Directive::Byte(_) => 1,
        Directive::Ascii(s) => s.len() as u16,
        Directive::Zero(count) => *count as u16,
    }
}

/// Performs pass-1 address assignment on parsed lines.
///
/// This function walks through all parsed lines, assigns addresses starting
/// at `start_address` (or 0x0000 by default), handles `.org` directives,
/// and builds a symbol table of label definitions.
///
/// # Errors
///
/// Returns a `SymbolError` if:
/// - A label is defined twice
/// - Address overflows 16-bit space
/// - `.org` would move the address backwards
pub fn assign_addresses(
    lines: &[ParsedLine],
    start_address: u16,
) -> Result<Assignment, SymbolError> {
    assign_addresses_with_lines(lines, start_address, &(1..=lines.len()).collect::<Vec<_>>())
}

/// Performs pass-1 address assignment with explicit source line numbers.
///
/// This is useful when the parsed lines came from an extracted source (like
/// literate Markdown) and the source line numbers differ from array indices.
///
/// # Errors
///
/// Returns a `SymbolError` if:
/// - A label is defined twice (`DuplicateLabel`)
/// - Address overflows 16-bit space (`AddressOverflow`)
/// - `.org` would move the address backwards (`OrgBackwards`)
#[allow(clippy::cast_possible_truncation)]
pub fn assign_addresses_with_lines(
    lines: &[ParsedLine],
    start_address: u16,
    source_lines: &[usize],
) -> Result<Assignment, SymbolError> {
    let mut symbols = SymbolTable::new();
    let mut addressed = Vec::with_capacity(lines.len());
    let mut pc: u32 = u32::from(start_address);

    for (i, parsed) in lines.iter().enumerate() {
        let source_line = *source_lines.get(i).unwrap_or(&(i + 1));
        let size = u32::from(line_size(parsed));
        let line_address = pc as u16;

        if let ParsedLine::Label { name } = parsed {
            if let Some(existing) = symbols.get(name) {
                return Err(SymbolError {
                    kind: SymbolErrorKind::DuplicateLabel {
                        name: name.clone(),
                        first_definition: existing.defined_at,
                    },
                    line: source_line,
                });
            }
            symbols.insert(
                name.clone(),
                Symbol {
                    address: line_address,
                    defined_at: source_line,
                },
            );
        }

        addressed.push(AddressedLine {
            address: line_address,
            size: size as u16,
            parsed: parsed.clone(),
            source_line,
        });

        if let ParsedLine::Directive {
            directive: Directive::Org(addr),
        } = parsed
        {
            let requested = *addr;
            if requested < pc {
                return Err(SymbolError {
                    kind: SymbolErrorKind::OrgBackwards {
                        current: line_address,
                        requested,
                    },
                    line: source_line,
                });
            }
            pc = requested;
        } else {
            pc += size;
        }

        if pc > 0xFFFF {
            return Err(SymbolError {
                kind: SymbolErrorKind::AddressOverflow { address: pc },
                line: source_line,
            });
        }
    }

    Ok(Assignment {
        lines: addressed,
        symbols,
        end_address: pc as u16,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_line;

    fn parse_lines(source: &[&str]) -> Vec<ParsedLine> {
        source
            .iter()
            .enumerate()
            .map(|(i, s)| parse_line(s, i + 1).unwrap())
            .collect()
    }

    #[test]
    fn empty_source() {
        let lines: Vec<ParsedLine> = vec![];
        let result = assign_addresses(&lines, 0).unwrap();
        assert!(result.lines.is_empty());
        assert!(result.symbols.is_empty());
        assert_eq!(result.end_address, 0);
    }

    #[test]
    fn single_label() {
        let lines = parse_lines(&["start:"]);
        let result = assign_addresses(&lines, 0).unwrap();
        assert_eq!(result.lines.len(), 1);
        assert_eq!(result.symbols.len(), 1);
        assert_eq!(result.symbols["start"].address, 0);
        assert_eq!(result.symbols["start"].defined_at, 1);
        assert_eq!(result.end_address, 0);
    }

    #[test]
    fn label_with_instruction() {
        let lines = parse_lines(&["init:", "NOP"]);
        let result = assign_addresses(&lines, 0).unwrap();
        assert_eq!(result.symbols["init"].address, 0);
        assert_eq!(result.lines[0].address, 0);
        assert_eq!(result.lines[0].size, 0);
        assert_eq!(result.lines[1].address, 0);
        assert_eq!(result.lines[1].size, 2);
        assert_eq!(result.end_address, 2);
    }

    #[test]
    fn multiple_labels_same_address() {
        let lines = parse_lines(&["entry:", "start:", "NOP"]);
        let result = assign_addresses(&lines, 0).unwrap();
        assert_eq!(result.symbols["entry"].address, 0);
        assert_eq!(result.symbols["start"].address, 0);
        assert_eq!(result.end_address, 2);
    }

    #[test]
    fn instruction_sizes() {
        let lines = parse_lines(&[
            "NOP",
            "MOV R0, R1",
            "MOV R0, #42",
            "LOAD R0, [R1]",
            "LOAD R0, [R1 + 10]",
        ]);
        let result = assign_addresses(&lines, 0).unwrap();
        assert_eq!(result.lines[0].size, 2);
        assert_eq!(result.lines[1].size, 2);
        assert_eq!(result.lines[2].size, 4);
        assert_eq!(result.lines[3].size, 2);
        assert_eq!(result.lines[4].size, 4);
        assert_eq!(result.end_address, 14);
    }

    #[test]
    fn directive_sizes() {
        let lines = parse_lines(&[".word 0x1234", ".byte 42", ".ascii \"hi\"", ".zero 8"]);
        let result = assign_addresses(&lines, 0).unwrap();
        assert_eq!(result.lines[0].size, 2);
        assert_eq!(result.lines[1].size, 1);
        assert_eq!(result.lines[2].size, 2);
        assert_eq!(result.lines[3].size, 8);
        assert_eq!(result.end_address, 13);
    }

    #[test]
    fn org_directive_forward() {
        let lines = parse_lines(&["NOP", ".org 0x100", "NOP"]);
        let result = assign_addresses(&lines, 0).unwrap();
        assert_eq!(result.lines[0].address, 0);
        assert_eq!(result.lines[1].address, 2);
        assert_eq!(result.lines[2].address, 0x100);
        assert_eq!(result.end_address, 0x102);
    }

    #[test]
    fn org_directive_backwards_error() {
        let lines = parse_lines(&[".org 0x100", "NOP", ".org 0x50"]);
        let err = assign_addresses(&lines, 0).unwrap_err();
        assert!(matches!(
            err.kind,
            SymbolErrorKind::OrgBackwards {
                current: 0x102,
                requested: 0x50
            }
        ));
        assert_eq!(err.line, 3);
    }

    #[test]
    fn duplicate_label_error() {
        let lines = parse_lines(&["start:", "NOP", "start:"]);
        let err = assign_addresses(&lines, 0).unwrap_err();
        assert!(matches!(
            err.kind,
            SymbolErrorKind::DuplicateLabel {
                name,
                first_definition: 1
            } if name == "start"
        ));
        assert_eq!(err.line, 3);
    }

    #[test]
    fn address_overflow_error() {
        let lines: Vec<&str> = vec!["NOP"; 32767];
        let parsed: Vec<ParsedLine> = parse_lines(&lines);
        let result = assign_addresses(&parsed, 0);
        assert!(result.is_ok());
        let result = result.unwrap();
        assert_eq!(result.end_address, 0xFFFEu16);

        let lines: Vec<&str> = vec!["NOP"; 32768];
        let parsed: Vec<ParsedLine> = parse_lines(&lines);
        let result = assign_addresses(&parsed, 0);
        assert!(result.is_err());
    }

    #[test]
    fn start_address_nonzero() {
        let lines = parse_lines(&["NOP", "loop:", "ADD R0, R1"]);
        let result = assign_addresses(&lines, 0x100).unwrap();
        assert_eq!(result.lines[0].address, 0x100);
        assert_eq!(result.symbols["loop"].address, 0x102);
        assert_eq!(result.end_address, 0x104);
    }

    #[test]
    fn mixed_instructions_and_data() {
        let lines = parse_lines(&[
            "start:",
            "MOV R0, #0x4000",
            "loop:",
            "STORE R0, [R0]",
            ".word 0x1234",
            ".byte 42",
            "HALT",
        ]);
        let result = assign_addresses(&lines, 0).unwrap();
        assert_eq!(result.symbols["start"].address, 0);
        assert_eq!(result.symbols["loop"].address, 4);
        assert_eq!(result.lines[0].address, 0);
        assert_eq!(result.lines[1].address, 0);
        assert_eq!(result.lines[1].size, 4);
        assert_eq!(result.lines[2].address, 4);
        assert_eq!(result.lines[3].address, 4);
        assert_eq!(result.lines[3].size, 2);
        assert_eq!(result.lines[4].address, 6);
        assert_eq!(result.lines[4].size, 2);
        assert_eq!(result.lines[5].address, 8);
        assert_eq!(result.lines[5].size, 1);
        assert_eq!(result.lines[6].address, 9);
        assert_eq!(result.lines[6].size, 2);
        assert_eq!(result.end_address, 11);
    }

    #[test]
    fn with_source_lines() {
        let lines = parse_lines(&["start:", "NOP", "end:"]);
        let source_lines = vec![10, 20, 30];
        let result = assign_addresses_with_lines(&lines, 0, &source_lines).unwrap();
        assert_eq!(result.symbols["start"].defined_at, 10);
        assert_eq!(result.symbols["end"].defined_at, 30);
        assert_eq!(result.lines[1].source_line, 20);
    }

    #[test]
    fn blank_lines_preserved() {
        let lines = parse_lines(&["NOP", "", "", "HALT"]);
        let result = assign_addresses(&lines, 0).unwrap();
        assert_eq!(result.lines.len(), 4);
        assert_eq!(result.lines[0].address, 0);
        assert_eq!(result.lines[1].address, 2);
        assert_eq!(result.lines[2].address, 2);
        assert_eq!(result.lines[3].address, 2);
        assert_eq!(result.end_address, 4);
    }
}
