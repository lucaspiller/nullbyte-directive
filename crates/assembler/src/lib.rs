//! Nullbyte Directive assembler library.

use emulator_core as _;

/// Top-level two-pass assembler pipeline.
pub mod assembler;
/// Instruction and directive encoding.
pub mod encoder;
/// Structured parse/assembly error types.
pub mod errors;
/// Include expansion (Pass 0).
pub mod include;
/// Mnemonic resolution against emulator opcode encoding tables.
pub mod mnemonic;
/// Assembly parser for instructions, labels, and directives.
pub mod parser;
/// Source loading and literate Markdown extraction.
pub mod source;
/// Symbol table and pass-1 address assignment.
pub mod symbols;
/// Inline test format parsing (`n1test` blocks).
pub mod test_format;
/// HALT-driven test execution engine.
pub mod test_runner;
