//! Top-level assembler pipeline.
//!
//! This module implements the full assembly pipeline, wiring together all
//! phases:
//!
//! 1. **Pass 0**: Include expansion (`include::expand_includes`)
//! 2. **Pass 1**: Parsing and symbol table construction
//! 3. **Pass 2**: Encoding to binary output
//!
//! The main entry point is [`assemble`], which takes a source file path and
//! returns the assembled binary plus collected test blocks.

use std::path::Path;

use crate::encoder::{encode_line, EncodeError};
use crate::include::{
    expand_includes, format_include_chain, ExpandedLine, ExpandedTestBlock, IncludeError,
};
use crate::parser::{parse_line, ParsedLine};
use crate::source::TestBlock;
use crate::symbols::{assign_addresses_with_lines, Assignment, SymbolError};

/// ROM region end address (inclusive) for address validation warnings.
const ROM_END: u16 = 0x3FFF;

/// Assembly error with source location context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssembleError {
    /// Kind of error.
    pub kind: AssembleErrorKind,
    /// Source location if available.
    pub location: Option<SourceLocation>,
}

/// Source location for error reporting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceLocation {
    /// File path.
    pub file: String,
    /// 1-indexed line number.
    pub line: usize,
    /// Include chain (outermost first).
    pub include_chain: String,
}

/// Classification of assembly errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssembleErrorKind {
    /// Include expansion failed.
    Include(IncludeError),
    /// Parse error.
    Parse(String),
    /// Symbol table error.
    Symbol(SymbolError),
    /// Encoding error.
    Encode(EncodeError),
    /// I/O error reading source file.
    Io(String),
}

impl std::fmt::Display for AssembleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.kind {
            AssembleErrorKind::Include(e) => write!(f, "include error: {e}"),
            AssembleErrorKind::Parse(msg) => write!(f, "parse error: {msg}"),
            AssembleErrorKind::Symbol(e) => write!(f, "{e}"),
            AssembleErrorKind::Encode(e) => write!(f, "{e}"),
            AssembleErrorKind::Io(msg) => write!(f, "I/O error: {msg}"),
        }
    }
}

impl std::error::Error for AssembleError {}

/// A warning generated during assembly (non-fatal).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssembleWarning {
    /// Kind of warning.
    pub kind: AssembleWarningKind,
    /// Source location if available.
    pub location: Option<SourceLocation>,
}

/// Classification of assembly warnings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssembleWarningKind {
    /// Code placed outside ROM region.
    OutsideRom {
        /// Address of the instruction/data.
        address: u16,
    },
}

impl std::fmt::Display for AssembleWarning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.kind {
            AssembleWarningKind::OutsideRom { address } => {
                write!(
                    f,
                    "code at address 0x{address:04X} is outside ROM region (0x0000-0x3FFF)"
                )
            }
        }
    }
}

/// Result of assembly containing binary output and metadata.
#[derive(Debug, Clone)]
pub struct AssembleResult {
    /// Assembled binary bytes.
    pub binary: Vec<u8>,
    /// Collected test blocks in document order.
    pub test_blocks: Vec<TestBlockContext>,
    /// Warnings generated during assembly.
    pub warnings: Vec<AssembleWarning>,
    /// Address-to-source mapping for listing generation.
    pub listing: Vec<ListingEntry>,
}

/// A test block with its include context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestBlockContext {
    /// The test block content.
    pub block: TestBlock,
    /// Include chain description for error reporting.
    pub include_context: String,
}

/// An entry in the address-to-source listing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListingEntry {
    /// Address of this entry.
    pub address: u16,
    /// Bytes at this address.
    pub bytes: Vec<u8>,
    /// Source line text.
    pub source: String,
    /// Source location for error reporting.
    pub location: String,
}

/// Assembles a source file into binary output.
///
/// This is the main entry point for the assembler. It performs all three
/// passes and returns the assembled binary plus metadata.
///
/// # Arguments
///
/// * `path` - Path to the source file (`.n1` or `.n1.md`)
///
/// # Errors
///
/// Returns `AssembleError` if any phase fails:
/// - Include expansion fails (file not found, circular include)
/// - Parsing fails (invalid syntax, unknown mnemonic)
/// - Symbol table construction fails (duplicate label, address overflow)
/// - Encoding fails (undefined label, displacement out of range)
///
/// # Warnings
///
/// The returned `AssembleResult` may contain warnings for non-fatal issues
/// such as code placed outside the ROM region.
#[allow(clippy::result_large_err)]
pub fn assemble(path: &Path) -> Result<AssembleResult, AssembleError> {
    let expanded = expand_includes(path).map_err(|e| AssembleError {
        kind: AssembleErrorKind::Include(e),
        location: None,
    })?;

    let parsed = parse_expanded_lines(&expanded.lines)?;

    let source_lines: Vec<usize> = parsed.iter().map(|p| p.source_line).collect();
    let parsed_lines: Vec<ParsedLine> = parsed.iter().map(|p| p.parsed.clone()).collect();

    let assignment = assign_addresses_with_lines(&parsed_lines, 0, &source_lines).map_err(|e| {
        AssembleError {
            kind: AssembleErrorKind::Symbol(e),
            location: None,
        }
    })?;

    let (binary, warnings, listing) = encode_pass2(&assignment, &expanded.lines)?;

    let test_blocks = expanded
        .test_blocks
        .into_iter()
        .map(|etb| {
            let include_context = format_include_chain_for_test(&etb);
            TestBlockContext {
                block: etb.block,
                include_context,
            }
        })
        .collect();

    Ok(AssembleResult {
        binary,
        test_blocks,
        warnings,
        listing,
    })
}

/// Parsed line with source location context.
struct ParsedWithContext {
    parsed: ParsedLine,
    source_line: usize,
}

#[allow(clippy::result_large_err)]
fn parse_expanded_lines(lines: &[ExpandedLine]) -> Result<Vec<ParsedWithContext>, AssembleError> {
    let mut result = Vec::with_capacity(lines.len());

    for expanded in lines {
        let parsed =
            parse_line(&expanded.text, expanded.original_line).map_err(|e| AssembleError {
                kind: AssembleErrorKind::Parse(e.to_string()),
                location: Some(SourceLocation {
                    file: expanded.file_path.to_string_lossy().to_string(),
                    line: expanded.original_line,
                    include_chain: format_include_chain(expanded),
                }),
            })?;

        result.push(ParsedWithContext {
            parsed,
            source_line: expanded.original_line,
        });
    }

    Ok(result)
}

#[allow(
    clippy::result_large_err,
    clippy::type_complexity,
    clippy::cast_possible_truncation
)]
fn encode_pass2(
    assignment: &Assignment,
    expanded_lines: &[ExpandedLine],
) -> Result<(Vec<u8>, Vec<AssembleWarning>, Vec<ListingEntry>), AssembleError> {
    let mut binary = Vec::new();
    let mut warnings = Vec::new();
    let mut listing = Vec::new();

    for addressed in &assignment.lines {
        let expanded = expanded_lines
            .iter()
            .find(|el| el.original_line == addressed.source_line)
            .cloned()
            .unwrap_or_else(|| ExpandedLine {
                text: String::new(),
                original_line: addressed.source_line,
                file_path: std::path::PathBuf::new(),
                include_chain: Vec::new(),
            });

        let location = format_include_chain(&expanded);

        if addressed.size > 0 && addressed.address > ROM_END {
            warnings.push(AssembleWarning {
                kind: AssembleWarningKind::OutsideRom {
                    address: addressed.address,
                },
                location: Some(SourceLocation {
                    file: expanded.file_path.to_string_lossy().to_string(),
                    line: expanded.original_line,
                    include_chain: location.clone(),
                }),
            });
        }

        if let ParsedLine::Directive {
            directive: crate::parser::Directive::Org(target),
        } = &addressed.parsed
        {
            let target_addr = *target as u16;
            if target_addr > binary.len() as u16 {
                let gap = target_addr as usize - binary.len();
                binary.extend(std::iter::repeat_n(0u8, gap));
            }
            continue;
        }

        let bytes = encode_line(
            &addressed.parsed,
            &assignment.symbols,
            addressed.address,
            addressed.source_line,
        )
        .map_err(|e| AssembleError {
            kind: AssembleErrorKind::Encode(e),
            location: Some(SourceLocation {
                file: expanded.file_path.to_string_lossy().to_string(),
                line: expanded.original_line,
                include_chain: location.clone(),
            }),
        })?;

        if !bytes.is_empty() {
            listing.push(ListingEntry {
                address: addressed.address,
                bytes: bytes.clone(),
                source: expanded.text.clone(),
                location: location.clone(),
            });
        }

        binary.extend(&bytes);
    }

    Ok((binary, warnings, listing))
}

fn format_include_chain_for_test(etb: &ExpandedTestBlock) -> String {
    if etb.include_chain.is_empty() {
        format!("{}:{}", etb.file_path.display(), etb.block.start_line)
    } else {
        let mut parts = vec![format!(
            "{}:{}",
            etb.file_path.display(),
            etb.block.start_line
        )];
        for entry in etb.include_chain.iter().rev() {
            parts.push(format!(
                "included from {}:{}",
                entry.from_file.display(),
                entry.line
            ));
        }
        parts.join(" (") + &")".repeat(etb.include_chain.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn create_temp_file(dir: &Path, name: &str, content: &str) -> PathBuf {
        let path = dir.join(name);
        fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn assemble_empty_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = create_temp_file(temp_dir.path(), "empty.n1", "");
        let result = assemble(&path).unwrap();
        assert!(result.binary.is_empty());
        assert!(result.test_blocks.is_empty());
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn assemble_single_nop() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = create_temp_file(temp_dir.path(), "nop.n1", "NOP\n");
        let result = assemble(&path).unwrap();
        assert_eq!(result.binary, &[0x00, 0x00]);
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn assemble_nop_halt() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = create_temp_file(temp_dir.path(), "simple.n1", "NOP\nHALT\n");
        let result = assemble(&path).unwrap();
        assert_eq!(result.binary, &[0x00, 0x00, 0x00, 0x10]);
    }

    #[test]
    fn assemble_with_label() {
        let temp_dir = tempfile::tempdir().unwrap();
        let content = "start:\n    NOP\n    JMP #start\n";
        let path = create_temp_file(temp_dir.path(), "loop.n1", content);
        let result = assemble(&path).unwrap();
        assert_eq!(result.binary.len(), 6);
        let primary = u16::from_be_bytes([result.binary[0], result.binary[1]]);
        assert_eq!(primary & 0xF000, 0x0000);
    }

    #[test]
    fn assemble_mov_immediate() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = create_temp_file(temp_dir.path(), "mov.n1", "MOV R0, #0x1234\n");
        let result = assemble(&path).unwrap();
        assert_eq!(result.binary.len(), 4);
        let extension = u16::from_be_bytes([result.binary[2], result.binary[3]]);
        assert_eq!(extension, 0x1234);
    }

    #[test]
    fn assemble_directives() {
        let temp_dir = tempfile::tempdir().unwrap();
        let content = ".word 0x1234\n.byte 0x42\n.ascii \"AB\"\n.zero 2\n";
        let path = create_temp_file(temp_dir.path(), "data.n1", content);
        let result = assemble(&path).unwrap();
        assert_eq!(result.binary, &[0x12, 0x34, 0x42, 0x41, 0x42, 0x00, 0x00]);
    }

    #[test]
    fn assemble_literate_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        let content = r"# Title

```n1asm
NOP
HALT
```
";
        let path = create_temp_file(temp_dir.path(), "lit.n1.md", content);
        let result = assemble(&path).unwrap();
        assert_eq!(result.binary, &[0x00, 0x00, 0x00, 0x10]);
    }

    #[test]
    fn assemble_extracts_test_blocks() {
        let temp_dir = tempfile::tempdir().unwrap();
        let content = r"# Title

```n1asm
MOV R0, #1
HALT
```

```n1test
R0 == 0x0001
```
";
        let path = create_temp_file(temp_dir.path(), "test.n1.md", content);
        let result = assemble(&path).unwrap();
        assert_eq!(result.binary.len(), 6);
        assert_eq!(result.test_blocks.len(), 1);
        assert_eq!(result.test_blocks[0].block.content, "R0 == 0x0001");
    }

    #[test]
    fn error_undefined_label() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = create_temp_file(temp_dir.path(), "bad.n1", "JMP #nonexistent\n");
        let result = assemble(&path);
        assert!(matches!(
            result,
            Err(AssembleError {
                kind: AssembleErrorKind::Encode(_),
                ..
            })
        ));
    }

    #[test]
    fn error_duplicate_label() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = create_temp_file(temp_dir.path(), "dup.n1", "start:\nNOP\nstart:\n");
        let result = assemble(&path);
        assert!(matches!(
            result,
            Err(AssembleError {
                kind: AssembleErrorKind::Symbol(_),
                ..
            })
        ));
    }

    #[test]
    fn warning_outside_rom() {
        let temp_dir = tempfile::tempdir().unwrap();
        let content = ".org 0x4000\nNOP\n";
        let path = create_temp_file(temp_dir.path(), "ram.n1", content);
        let result = assemble(&path).unwrap();
        assert_eq!(result.binary.len(), 0x4002);
        assert!(!result.warnings.is_empty());
        assert!(matches!(
            &result.warnings[0].kind,
            AssembleWarningKind::OutsideRom { address } if *address == 0x4000
        ));
    }

    #[test]
    fn assemble_with_include() {
        let temp_dir = tempfile::tempdir().unwrap();

        let included = create_temp_file(temp_dir.path(), "lib.n1", "ADD R0, R0, R1\n");

        let main_content = format!(
            "NOP\n.include \"{}\"\nHALT\n",
            included.file_name().unwrap().to_str().unwrap()
        );
        let main = create_temp_file(temp_dir.path(), "main.n1", &main_content);

        let result = assemble(&main).unwrap();
        assert_eq!(result.binary.len(), 6);
    }

    #[test]
    fn listing_generation() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = create_temp_file(temp_dir.path(), "list.n1", "NOP\nMOV R0, #1\nHALT\n");
        let result = assemble(&path).unwrap();

        assert_eq!(result.listing.len(), 3);
        assert_eq!(result.listing[0].address, 0);
        assert_eq!(result.listing[0].bytes, &[0x00, 0x00]);
        assert_eq!(result.listing[1].address, 2);
        assert_eq!(result.listing[1].bytes.len(), 4);
        assert_eq!(result.listing[2].address, 6);
    }

    #[test]
    fn assemble_forward_reference() {
        let temp_dir = tempfile::tempdir().unwrap();
        let content = "JMP #later\nNOP\nlater:\nHALT\n";
        let path = create_temp_file(temp_dir.path(), "fwd.n1", content);
        let result = assemble(&path).unwrap();

        assert_eq!(result.binary.len(), 8);
        let extension = u16::from_be_bytes([result.binary[2], result.binary[3]]);
        assert_eq!(extension, 0x0002);
    }

    #[test]
    fn assemble_backward_reference() {
        let temp_dir = tempfile::tempdir().unwrap();
        let content = "loop:\nNOP\nJMP #loop\n";
        let path = create_temp_file(temp_dir.path(), "back.n1", content);
        let result = assemble(&path).unwrap();

        assert_eq!(result.binary.len(), 6);
        let extension = u16::from_be_bytes([result.binary[4], result.binary[5]]);
        assert_eq!(extension, 0xFFFA);
    }

    #[test]
    fn assemble_branch_instructions() {
        let temp_dir = tempfile::tempdir().unwrap();
        let content = "start:\nNOP\nBEQ #start\nBNE #start\nHALT\n";
        let path = create_temp_file(temp_dir.path(), "branch.n1", content);
        let result = assemble(&path).unwrap();

        assert_eq!(result.binary.len(), 12);
    }

    #[test]
    fn assemble_call_ret() {
        let temp_dir = tempfile::tempdir().unwrap();
        let content = "CALL #sub\nHALT\nsub:\nPUSH R0\nPOP R0\nRET\n";
        let path = create_temp_file(temp_dir.path(), "call.n1", content);
        let result = assemble(&path).unwrap();

        assert_eq!(result.binary.len(), 12);
    }

    #[test]
    fn assemble_load_store() {
        let temp_dir = tempfile::tempdir().unwrap();
        let content = "MOV R0, #0x4000\nSTORE R0, [R0]\nLOAD R1, [R0]\nHALT\n";
        let path = create_temp_file(temp_dir.path(), "mem.n1", content);
        let result = assemble(&path).unwrap();

        assert_eq!(result.binary.len(), 10);
    }

    #[test]
    fn assemble_complete_program() {
        let temp_dir = tempfile::tempdir().unwrap();
        let content = r";
; Simple program: count from 1 to 10
    MOV R0, #1      ; counter
    MOV R1, #10     ; limit
loop:
    ADD R0, R0, #1  ; increment
    CMP R0, R1      ; compare
    BLT #loop       ; continue if less
    HALT            ; done
";
        let path = create_temp_file(temp_dir.path(), "count.n1", content);
        let result = assemble(&path).unwrap();

        assert!(!result.binary.is_empty());
        assert!(result.binary.len() <= 0x4000);
    }
}
