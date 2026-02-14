//! Nullbyte Directive assembler library.

use emulator_core as _;

/// Top-level two-pass assembler pipeline.
pub mod assembler;
/// Instruction and directive encoding.
pub mod encoder;
/// Structured parse/assembly error types.
pub mod errors;
/// Mnemonic resolution against emulator opcode encoding tables.
pub mod mnemonic;
/// Assembly parser for instructions, labels, and directives.
pub mod parser;
/// Source loading and literate Markdown extraction.
pub mod source;
/// Symbol table and pass-1 address assignment.
pub mod symbols;
