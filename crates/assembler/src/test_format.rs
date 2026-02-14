//! Parsing for inline test assertion syntax.
//!
//! This module parses `n1test` block content into structured assertions.
//!
//! ## Supported Syntax
//!
//! - Register assertions: `R0 == 0x4000`, `PC != 0x0000`
//! - Memory assertions: `[0x4000] == 0xFF`, `[0x1000] != 0x00`
//! - Comments: `;` to end of line
//! - Literals: decimal, `0x` hex, `0b` binary

#![allow(
    clippy::uninlined_format_args,
    clippy::option_if_let_else,
    clippy::manual_strip,
    clippy::redundant_closure,
    clippy::similar_names,
    clippy::unreadable_literal,
    clippy::use_self
)]

use std::fmt;

/// A parsed assertion from an `n1test` block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Assertion {
    /// Assert register value equals or not-equals expected.
    Register {
        /// The register to check.
        register: Register,
        /// The comparison operator.
        operator: ComparisonOp,
        /// The expected value.
        expected: u16,
    },
    /// Assert memory byte at address equals or not-equals expected.
    Memory {
        /// The memory address to check.
        address: u16,
        /// The comparison operator.
        operator: ComparisonOp,
        /// The expected byte value.
        expected: u8,
    },
}

/// A register that can be asserted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Register {
    /// General-purpose register R0.
    R0,
    /// General-purpose register R1.
    R1,
    /// General-purpose register R2.
    R2,
    /// General-purpose register R3.
    R3,
    /// General-purpose register R4.
    R4,
    /// General-purpose register R5.
    R5,
    /// General-purpose register R6.
    R6,
    /// General-purpose register R7.
    R7,
    /// Program counter.
    PC,
}

/// Comparison operator for assertions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComparisonOp {
    /// Assert equality (`==`).
    Equal,
    /// Assert inequality (`!=`).
    NotEqual,
}

impl fmt::Display for ComparisonOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ComparisonOp::Equal => write!(f, "=="),
            ComparisonOp::NotEqual => write!(f, "!="),
        }
    }
}

/// A parsed test block with its assertions and source location.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedTestBlock {
    /// The parsed assertions in order.
    pub assertions: Vec<Assertion>,
    /// 1-indexed line number where the block starts.
    pub start_line: usize,
    /// 1-indexed line number where the block ends.
    pub end_line: usize,
}

/// Error parsing an assertion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseAssertionError {
    /// The line number (1-indexed) within the test block where the error occurred.
    pub line_in_block: usize,
    /// The problematic text.
    pub text: String,
    /// Description of the error.
    pub message: String,
}

impl fmt::Display for ParseAssertionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "line {}: {} (in '{}')",
            self.line_in_block, self.message, self.text
        )
    }
}

impl std::error::Error for ParseAssertionError {}

/// Parses a test block's content into structured assertions.
///
/// Each non-empty, non-comment line is parsed as an assertion.
/// Returns a list of assertions or the first parse error encountered.
///
/// # Arguments
///
/// * `content` - The raw text content of the test block (without fence lines)
/// * `start_line` - The 1-indexed line number where the block starts in source
/// * `end_line` - The 1-indexed line number where the block ends in source
///
/// # Errors
///
/// Returns `ParseAssertionError` if any line has invalid syntax.
pub fn parse_test_block(
    content: &str,
    start_line: usize,
    end_line: usize,
) -> Result<ParsedTestBlock, ParseAssertionError> {
    let mut assertions = Vec::new();

    for (idx, line) in content.lines().enumerate() {
        let line_num = idx + 1;
        let stripped = strip_comment(line).trim();

        if stripped.is_empty() {
            continue;
        }

        let assertion = parse_assertion(stripped).map_err(|message| ParseAssertionError {
            line_in_block: line_num,
            text: stripped.to_string(),
            message,
        })?;

        assertions.push(assertion);
    }

    Ok(ParsedTestBlock {
        assertions,
        start_line,
        end_line,
    })
}

/// Strips a comment from a line (everything from `;` to end of line).
fn strip_comment(line: &str) -> &str {
    match line.find(';') {
        Some(pos) => &line[..pos],
        None => line,
    }
}

/// Parses a single assertion line.
fn parse_assertion(text: &str) -> Result<Assertion, String> {
    let text = text.trim();

    if text.starts_with('[') {
        parse_memory_assertion(text)
    } else {
        parse_register_assertion(text)
    }
}

/// Parses a memory assertion like `[0x4000] == 0xFF`.
fn parse_memory_assertion(text: &str) -> Result<Assertion, String> {
    let close_bracket = text
        .find(']')
        .ok_or_else(|| "expected ']' after address".to_string())?;

    let addr_text = &text[1..close_bracket];
    let address = parse_u16(addr_text)?;

    let rest = text[close_bracket + 1..].trim();

    let (operator, rest) = parse_comparison_op(rest)?;
    let expected = parse_u8(rest.trim())?;

    Ok(Assertion::Memory {
        address,
        operator,
        expected,
    })
}

/// Parses a register assertion like `R0 == 0x4000` or `PC != 0x0000`.
fn parse_register_assertion(text: &str) -> Result<Assertion, String> {
    let parts: Vec<&str> = text.split_whitespace().collect();

    if parts.len() < 3 {
        return Err("expected 'register operator value'".to_string());
    }

    let register = parse_register(parts[0])?;
    let operator = parse_comparison_op(parts[1])?.0;
    let expected = parse_u16(parts[2])?;

    Ok(Assertion::Register {
        register,
        operator,
        expected,
    })
}

/// Parses a register name (R0-R7 or PC).
fn parse_register(text: &str) -> Result<Register, String> {
    let upper = text.to_ascii_uppercase();
    match upper.as_str() {
        "R0" => Ok(Register::R0),
        "R1" => Ok(Register::R1),
        "R2" => Ok(Register::R2),
        "R3" => Ok(Register::R3),
        "R4" => Ok(Register::R4),
        "R5" => Ok(Register::R5),
        "R6" => Ok(Register::R6),
        "R7" => Ok(Register::R7),
        "PC" => Ok(Register::PC),
        _ => Err(format!("unknown register '{}'", text)),
    }
}

/// Parses a comparison operator (`==` or `!=`).
fn parse_comparison_op(text: &str) -> Result<(ComparisonOp, &str), String> {
    let text = text.trim_start();
    if text.starts_with("==") {
        Ok((ComparisonOp::Equal, &text[2..]))
    } else if text.starts_with("!=") {
        Ok((ComparisonOp::NotEqual, &text[2..]))
    } else {
        Err("expected '==' or '!='".to_string())
    }
}

/// Parses an unsigned 16-bit value (decimal, hex, or binary).
fn parse_u16(text: &str) -> Result<u16, String> {
    let text = text.trim();
    if text.is_empty() {
        return Err("expected a value".to_string());
    }

    if let Some(hex) = text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
        u16::from_str_radix(hex, 16).map_err(|_| format!("invalid hex value '{}'", text))
    } else if let Some(bin) = text.strip_prefix("0b").or_else(|| text.strip_prefix("0B")) {
        u16::from_str_radix(bin, 2).map_err(|_| format!("invalid binary value '{}'", text))
    } else {
        text.parse::<u16>()
            .map_err(|_| format!("invalid decimal value '{}'", text))
    }
}

/// Parses an unsigned 8-bit value (decimal, hex, or binary).
fn parse_u8(text: &str) -> Result<u8, String> {
    let text = text.trim();
    if text.is_empty() {
        return Err("expected a value".to_string());
    }

    if let Some(hex) = text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
        u8::from_str_radix(hex, 16).map_err(|_| format!("invalid hex value '{}'", text))
    } else if let Some(bin) = text.strip_prefix("0b").or_else(|| text.strip_prefix("0B")) {
        u8::from_str_radix(bin, 2).map_err(|_| format!("invalid binary value '{}'", text))
    } else {
        text.parse::<u8>()
            .map_err(|_| format!("invalid decimal value '{}'", text))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_register_equality() {
        let result = parse_assertion("R0 == 0x4000").unwrap();
        assert_eq!(
            result,
            Assertion::Register {
                register: Register::R0,
                operator: ComparisonOp::Equal,
                expected: 0x4000,
            }
        );
    }

    #[test]
    fn parse_register_inequality() {
        let result = parse_assertion("PC != 0x0000").unwrap();
        assert_eq!(
            result,
            Assertion::Register {
                register: Register::PC,
                operator: ComparisonOp::NotEqual,
                expected: 0x0000,
            }
        );
    }

    #[test]
    fn parse_register_decimal() {
        let result = parse_assertion("R7 == 255").unwrap();
        assert_eq!(
            result,
            Assertion::Register {
                register: Register::R7,
                operator: ComparisonOp::Equal,
                expected: 255,
            }
        );
    }

    #[test]
    fn parse_register_binary() {
        let result = parse_assertion("R3 == 0b10101010").unwrap();
        assert_eq!(
            result,
            Assertion::Register {
                register: Register::R3,
                operator: ComparisonOp::Equal,
                expected: 0b10101010,
            }
        );
    }

    #[test]
    fn parse_memory_equality() {
        let result = parse_assertion("[0x4000] == 0xFF").unwrap();
        assert_eq!(
            result,
            Assertion::Memory {
                address: 0x4000,
                operator: ComparisonOp::Equal,
                expected: 0xFF,
            }
        );
    }

    #[test]
    fn parse_memory_inequality() {
        let result = parse_assertion("[0x1000] != 0x00").unwrap();
        assert_eq!(
            result,
            Assertion::Memory {
                address: 0x1000,
                operator: ComparisonOp::NotEqual,
                expected: 0x00,
            }
        );
    }

    #[test]
    fn parse_memory_decimal() {
        let result = parse_assertion("[16384] == 255").unwrap();
        assert_eq!(
            result,
            Assertion::Memory {
                address: 16384,
                operator: ComparisonOp::Equal,
                expected: 255,
            }
        );
    }

    #[test]
    fn parse_with_comment() {
        let result = parse_assertion("R0 == 0x4000 ; this is a comment").unwrap();
        assert_eq!(
            result,
            Assertion::Register {
                register: Register::R0,
                operator: ComparisonOp::Equal,
                expected: 0x4000,
            }
        );
    }

    #[test]
    fn parse_case_insensitive_register() {
        let result = parse_assertion("r0 == 0x0001").unwrap();
        assert_eq!(
            result,
            Assertion::Register {
                register: Register::R0,
                operator: ComparisonOp::Equal,
                expected: 0x0001,
            }
        );
    }

    #[test]
    fn parse_test_block_success() {
        let content = "R0 == 0x4000\nR1 == 0x00FF\n[0x4000] == 0xFF";
        let result = parse_test_block(content, 3, 6).unwrap();

        assert_eq!(result.start_line, 3);
        assert_eq!(result.end_line, 6);
        assert_eq!(result.assertions.len(), 3);
    }

    #[test]
    fn parse_test_block_with_comments_and_blanks() {
        let content = "; Check initial state\nR0 == 0x4000\n\n; Memory check\n[0x4000] == 0xFF\n";
        let result = parse_test_block(content, 3, 8).unwrap();

        assert_eq!(result.assertions.len(), 2);
    }

    #[test]
    fn parse_test_block_empty() {
        let content = "";
        let result = parse_test_block(content, 3, 4).unwrap();

        assert!(result.assertions.is_empty());
    }

    #[test]
    fn parse_error_unknown_register() {
        let result = parse_assertion("R8 == 0x0001");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("unknown register"));
    }

    #[test]
    fn parse_error_invalid_operator() {
        let result = parse_assertion("R0 >= 0x0001");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("expected '==' or '!='"));
    }

    #[test]
    fn parse_error_invalid_value() {
        let result = parse_assertion("R0 == 0xGGGG");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("invalid"));
    }

    #[test]
    fn parse_error_missing_bracket() {
        let result = parse_assertion("[0x4000 == 0xFF");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("expected ']'"));
    }

    #[test]
    fn parse_test_block_error_line_number() {
        let content = "R0 == 0x4000\nR8 == 0x0001\n[0x4000] == 0xFF";
        let result = parse_test_block(content, 3, 6);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.line_in_block, 2);
        assert!(err.message.contains("unknown register"));
    }

    #[test]
    fn all_registers_parseable() {
        for (reg, name) in [
            (Register::R0, "R0"),
            (Register::R1, "R1"),
            (Register::R2, "R2"),
            (Register::R3, "R3"),
            (Register::R4, "R4"),
            (Register::R5, "R5"),
            (Register::R6, "R6"),
            (Register::R7, "R7"),
            (Register::PC, "PC"),
        ] {
            let result = parse_assertion(&format!("{} == 0x0000", name)).unwrap();
            assert_eq!(
                result,
                Assertion::Register {
                    register: reg,
                    operator: ComparisonOp::Equal,
                    expected: 0x0000,
                }
            );
        }
    }
}
