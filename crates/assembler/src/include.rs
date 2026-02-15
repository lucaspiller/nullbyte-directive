//! Include expansion (Pass 0) for recursive source inclusion.
//!
//! This module handles expanding `.include` directives before the main
//! assembly pipeline. It supports:
//! - Recursive includes (files may include other files)
//! - Circular include detection
//! - Mixed format includes (`.n1` and `.n1.md`)
//! - Source location tracking with include chains

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::parser::{parse_line, Directive, ParsedLine};
use crate::source::{extract_source, SourceLine, TestBlock};

/// An expanded source line with full include chain context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpandedLine {
    /// The source text (without trailing newline).
    pub text: String,
    /// 1-indexed line number in the original file.
    pub original_line: usize,
    /// Path to the file containing this line.
    pub file_path: PathBuf,
    /// Include chain leading to this file (outermost first).
    pub include_chain: Vec<IncludeEntry>,
}

/// A test block collected from an included file with include chain context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpandedTestBlock {
    /// The test block content.
    pub block: TestBlock,
    /// Path to the file containing this block.
    pub file_path: PathBuf,
    /// Include chain leading to this file (outermost first).
    pub include_chain: Vec<IncludeEntry>,
}

/// An entry in an include chain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncludeEntry {
    /// Path of the file that contained the `.include` directive.
    pub from_file: PathBuf,
    /// Line number of the `.include` directive.
    pub line: usize,
}

/// Include expansion error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncludeError {
    /// Path where the error occurred.
    pub path: PathBuf,
    /// Include chain leading to the error.
    pub include_chain: Vec<IncludeEntry>,
    /// Kind of error.
    pub kind: IncludeErrorKind,
}

/// Classification of include errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IncludeErrorKind {
    /// File not found.
    FileNotFound,
    /// I/O error reading file.
    IoError(String),
    /// Circular include detected.
    CircularInclude(PathBuf),
    /// Parse error in the source.
    ParseError(String),
}

impl std::fmt::Display for IncludeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: ", self.path.display())?;
        match &self.kind {
            IncludeErrorKind::FileNotFound => write!(f, "file not found"),
            IncludeErrorKind::IoError(msg) => write!(f, "I/O error: {msg}"),
            IncludeErrorKind::CircularInclude(path) => {
                write!(f, "circular include detected: {}", path.display())
            }
            IncludeErrorKind::ParseError(msg) => write!(f, "parse error: {msg}"),
        }
    }
}

impl std::error::Error for IncludeError {}

/// Result of include expansion, containing both source lines and test blocks.
pub struct ExpansionResult {
    /// Expanded source lines in document order.
    pub lines: Vec<ExpandedLine>,
    /// Test blocks in document order (ordered by position in the expanded assembly stream).
    pub test_blocks: Vec<ExpandedTestBlock>,
}

/// Expands all `.include` directives in a source file.
///
/// This is Pass 0 of the assembler: it recursively processes `.include`
/// directives and produces a flat list of source lines with full location
/// tracking, plus all `n1test` blocks in document order.
///
/// # Arguments
///
/// * `root_path` - Path to the root source file
///
/// # Errors
///
/// Returns an `IncludeError` if:
/// - The file cannot be read
/// - A circular include is detected
/// - An included file does not exist
pub fn expand_includes(root_path: &Path) -> Result<ExpansionResult, IncludeError> {
    let mut visited = HashSet::new();
    let mut include_chain = Vec::new();
    let mut result = ExpansionResult {
        lines: Vec::new(),
        test_blocks: Vec::new(),
    };
    expand_includes_recursive(root_path, &mut visited, &mut include_chain, &mut result)?;
    Ok(result)
}

fn expand_includes_recursive(
    path: &Path,
    visited: &mut HashSet<PathBuf>,
    include_chain: &mut Vec<IncludeEntry>,
    result: &mut ExpansionResult,
) -> Result<(), IncludeError> {
    let canonical = path.canonicalize().map_err(|_| IncludeError {
        path: path.to_path_buf(),
        include_chain: include_chain.clone(),
        kind: IncludeErrorKind::FileNotFound,
    })?;

    if visited.contains(&canonical) {
        return Err(IncludeError {
            path: path.to_path_buf(),
            include_chain: include_chain.clone(),
            kind: IncludeErrorKind::CircularInclude(canonical),
        });
    }
    visited.insert(canonical.clone());

    let content = fs::read_to_string(path).map_err(|e| IncludeError {
        path: path.to_path_buf(),
        include_chain: include_chain.clone(),
        kind: IncludeErrorKind::IoError(e.to_string()),
    })?;

    let source = extract_source(path, &content);

    let mut test_block_iter = source.test_blocks.into_iter().peekable();

    for SourceLine {
        text,
        original_line,
    } in source.lines
    {
        while let Some(test_block) = test_block_iter.peek() {
            if test_block.start_line < original_line {
                let test_block = test_block_iter.next().unwrap();
                result.test_blocks.push(ExpandedTestBlock {
                    block: test_block,
                    file_path: path.to_path_buf(),
                    include_chain: include_chain.clone(),
                });
            } else {
                break;
            }
        }

        let parse_result = parse_line(&text, original_line);

        match parse_result {
            Ok(ParsedLine::Directive {
                directive: Directive::Include(include_path),
            }) => {
                let resolved = resolve_include_path(&include_path, path);

                let entry = IncludeEntry {
                    from_file: path.to_path_buf(),
                    line: original_line,
                };
                include_chain.push(entry);

                expand_includes_recursive(&resolved, visited, include_chain, result)?;

                include_chain.pop();
            }
            Ok(_) => {
                result.lines.push(ExpandedLine {
                    text,
                    original_line,
                    file_path: path.to_path_buf(),
                    include_chain: include_chain.clone(),
                });
            }
            Err(e) => {
                return Err(IncludeError {
                    path: path.to_path_buf(),
                    include_chain: include_chain.clone(),
                    kind: IncludeErrorKind::ParseError(e.to_string()),
                });
            }
        }
    }

    for test_block in test_block_iter {
        result.test_blocks.push(ExpandedTestBlock {
            block: test_block,
            file_path: path.to_path_buf(),
            include_chain: include_chain.clone(),
        });
    }

    visited.remove(&canonical);
    Ok(())
}

/// Resolves an include path relative to the containing file's directory.
fn resolve_include_path(include_path: &str, containing_file: &Path) -> PathBuf {
    let include = PathBuf::from(include_path);

    if include.is_absolute() {
        include
    } else {
        match containing_file.parent() {
            Some(dir) => dir.join(include),
            None => include,
        }
    }
}

/// Formats an include chain for error messages.
///
/// Returns a string like `math.n1:12 (included from main.n1.md:3)`.
#[must_use]
pub fn format_include_chain(line: &ExpandedLine) -> String {
    if line.include_chain.is_empty() {
        format!("{}:{}", line.file_path.display(), line.original_line)
    } else {
        let mut parts = vec![format!(
            "{}:{}",
            line.file_path.display(),
            line.original_line
        )];
        for entry in line.include_chain.iter().rev() {
            parts.push(format!(
                "included from {}:{}",
                entry.from_file.display(),
                entry.line
            ));
        }
        parts.join(" (") + &")".repeat(line.include_chain.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[allow(unused_imports)]
    use tempfile as _;

    fn create_temp_file(dir: &Path, name: &str, content: &str) -> PathBuf {
        let path = dir.join(name);
        fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn resolve_relative_path() {
        let containing = Path::new("/project/src/main.n1");
        let result = resolve_include_path("utils.n1", containing);
        assert_eq!(result, PathBuf::from("/project/src/utils.n1"));
    }

    #[test]
    fn resolve_relative_path_with_subdirectory() {
        let containing = Path::new("/project/src/main.n1");
        let result = resolve_include_path("lib/math.n1", containing);
        assert_eq!(result, PathBuf::from("/project/src/lib/math.n1"));
    }

    #[test]
    fn resolve_absolute_path() {
        let containing = Path::new("/project/src/main.n1");
        let result = resolve_include_path("/lib/math.n1", containing);
        assert_eq!(result, PathBuf::from("/lib/math.n1"));
    }

    #[test]
    fn resolve_with_no_parent() {
        let containing = Path::new("main.n1");
        let result = resolve_include_path("utils.n1", containing);
        assert_eq!(result, PathBuf::from("utils.n1"));
    }

    #[test]
    fn expand_single_file_no_includes() {
        let temp_dir = tempfile::tempdir().unwrap();
        let content = "MOV R0, #1\nADD R0, R0, R1\n";
        let path = create_temp_file(temp_dir.path(), "test.n1", content);

        let result = expand_includes(&path).unwrap();
        assert_eq!(result.lines.len(), 2);
        assert_eq!(result.lines[0].text, "MOV R0, #1");
        assert_eq!(result.lines[0].original_line, 1);
        assert_eq!(result.lines[1].text, "ADD R0, R0, R1");
        assert_eq!(result.lines[1].original_line, 2);
        assert!(result.lines[0].include_chain.is_empty());
        assert!(result.test_blocks.is_empty());
    }

    #[test]
    fn expand_single_level_include() {
        let temp_dir = tempfile::tempdir().unwrap();

        let included_content = "ADD R0, R0, R1\n";
        let included_path = create_temp_file(temp_dir.path(), "utils.n1", included_content);

        let main_content = format!(
            "MOV R0, #1\n.include \"{}\"\nHALT\n",
            included_path.file_name().unwrap().to_str().unwrap()
        );
        let main_path = create_temp_file(temp_dir.path(), "main.n1", &main_content);

        let result = expand_includes(&main_path).unwrap();
        assert_eq!(result.lines.len(), 3);
        assert_eq!(result.lines[0].text, "MOV R0, #1");
        assert_eq!(result.lines[1].text, "ADD R0, R0, R1");
        assert_eq!(result.lines[2].text, "HALT");

        assert!(result.lines[0].include_chain.is_empty());
        assert_eq!(result.lines[1].include_chain.len(), 1);
        assert_eq!(result.lines[1].include_chain[0].line, 2);
    }

    #[test]
    fn expand_recursive_include() {
        let temp_dir = tempfile::tempdir().unwrap();
        let sub_dir = temp_dir.path().join("lib");
        fs::create_dir(&sub_dir).unwrap();

        let innermost_content = "XOR R0, R0\n";
        let innermost_path = create_temp_file(&sub_dir, "inner.n1", innermost_content);

        let middle_content = format!(
            "ADD R0, R0, R1\n.include \"{}\"\nSUB R0, R0, R1\n",
            innermost_path.file_name().unwrap().to_str().unwrap()
        );
        let _middle_path = create_temp_file(&sub_dir, "middle.n1", &middle_content);

        let main_content = "MOV R0, #1\n.include \"lib/middle.n1\"\nHALT\n";
        let main_path = create_temp_file(temp_dir.path(), "main.n1", main_content);

        let result = expand_includes(&main_path).unwrap();
        assert_eq!(result.lines.len(), 5);
        assert_eq!(result.lines[0].text, "MOV R0, #1");
        assert_eq!(result.lines[1].text, "ADD R0, R0, R1");
        assert_eq!(result.lines[2].text, "XOR R0, R0");
        assert_eq!(result.lines[3].text, "SUB R0, R0, R1");
        assert_eq!(result.lines[4].text, "HALT");

        assert!(result.lines[0].include_chain.is_empty());
        assert_eq!(result.lines[1].include_chain.len(), 1);
        assert_eq!(result.lines[2].include_chain.len(), 2);
    }

    #[test]
    fn detect_circular_include() {
        let temp_dir = tempfile::tempdir().unwrap();

        let a_content = ".include \"b.n1\"\nMOV R0, #1\n";
        let a_path = create_temp_file(temp_dir.path(), "a.n1", a_content);

        let b_content = ".include \"a.n1\"\nADD R0, R0, R1\n";
        let _b_path = create_temp_file(temp_dir.path(), "b.n1", b_content);

        let result = expand_includes(&a_path);
        assert!(matches!(
            result,
            Err(IncludeError {
                kind: IncludeErrorKind::CircularInclude(_),
                ..
            })
        ));
    }

    #[test]
    fn file_not_found_error() {
        let temp_dir = tempfile::tempdir().unwrap();

        let content = ".include \"nonexistent.n1\"\nMOV R0, #1\n";
        let path = create_temp_file(temp_dir.path(), "test.n1", content);

        let result = expand_includes(&path);
        assert!(matches!(
            result,
            Err(IncludeError {
                kind: IncludeErrorKind::FileNotFound,
                ..
            })
        ));
    }

    #[test]
    fn expand_literate_file() {
        let temp_dir = tempfile::tempdir().unwrap();

        let included_content = "```n1asm\nADD R0, R0, R1\n```\n";
        let included_path = create_temp_file(temp_dir.path(), "utils.n1.md", included_content);

        let main_content = format!(
            "```n1asm\nMOV R0, #1\n.include \"{}\"\nHALT\n```\n",
            included_path.file_name().unwrap().to_str().unwrap()
        );
        let main_path = create_temp_file(temp_dir.path(), "main.n1.md", &main_content);

        let result = expand_includes(&main_path).unwrap();
        assert_eq!(result.lines.len(), 3);
        assert_eq!(result.lines[0].text, "MOV R0, #1");
        assert_eq!(result.lines[1].text, "ADD R0, R0, R1");
        assert_eq!(result.lines[2].text, "HALT");
    }

    #[test]
    fn format_empty_include_chain() {
        let line = ExpandedLine {
            text: "MOV R0, #1".into(),
            original_line: 5,
            file_path: PathBuf::from("main.n1"),
            include_chain: vec![],
        };
        assert_eq!(format_include_chain(&line), "main.n1:5");
    }

    #[test]
    fn format_single_include_chain() {
        let line = ExpandedLine {
            text: "ADD R0, R0, R1".into(),
            original_line: 3,
            file_path: PathBuf::from("utils.n1"),
            include_chain: vec![IncludeEntry {
                from_file: PathBuf::from("main.n1"),
                line: 2,
            }],
        };
        assert_eq!(
            format_include_chain(&line),
            "utils.n1:3 (included from main.n1:2)"
        );
    }

    #[test]
    fn format_nested_include_chain() {
        let line = ExpandedLine {
            text: "XOR R0, R0".into(),
            original_line: 7,
            file_path: PathBuf::from("inner.n1"),
            include_chain: vec![
                IncludeEntry {
                    from_file: PathBuf::from("main.n1"),
                    line: 2,
                },
                IncludeEntry {
                    from_file: PathBuf::from("middle.n1"),
                    line: 4,
                },
            ],
        };
        assert_eq!(
            format_include_chain(&line),
            "inner.n1:7 (included from middle.n1:4 (included from main.n1:2))"
        );
    }

    #[test]
    fn collect_test_blocks_from_single_file() {
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

        let result = expand_includes(&path).unwrap();
        assert_eq!(result.lines.len(), 2);
        assert_eq!(result.test_blocks.len(), 1);
        assert_eq!(result.test_blocks[0].block.content, "R0 == 0x0001");
        assert!(result.test_blocks[0].include_chain.is_empty());
    }

    #[test]
    fn collect_test_blocks_from_included_file() {
        let temp_dir = tempfile::tempdir().unwrap();

        let included_content = "```n1asm\nADD R0, R0, R1\n```\n\n```n1test\nR1 == 0x0002\n```\n";
        let included_path = create_temp_file(temp_dir.path(), "utils.n1.md", included_content);

        let main_content = format!(
            "```n1asm\nMOV R0, #1\n.include \"{}\"\nHALT\n```\n\n```n1test\nR0 == 0x0001\n```\n",
            included_path.file_name().unwrap().to_str().unwrap()
        );
        let main_path = create_temp_file(temp_dir.path(), "main.n1.md", &main_content);

        let result = expand_includes(&main_path).unwrap();
        assert_eq!(result.lines.len(), 3);

        assert_eq!(result.test_blocks.len(), 2);
        assert_eq!(result.test_blocks[0].block.content, "R1 == 0x0002");
        assert_eq!(result.test_blocks[0].include_chain.len(), 1);
        assert_eq!(result.test_blocks[1].block.content, "R0 == 0x0001");
        assert!(result.test_blocks[1].include_chain.is_empty());
    }

    #[test]
    fn test_blocks_in_document_order() {
        let temp_dir = tempfile::tempdir().unwrap();

        let included_content =
            "```n1asm\n; included code\n```\n\n```n1test\n; test B - from included file\n```\n";
        let included_path = create_temp_file(temp_dir.path(), "inc.n1.md", included_content);

        let main_content = format!(
            "```n1test\n; test A - before include\n```\n\n```n1asm\n.include \"{}\"\n```\n\n```n1test\n; test C - after include\n```\n",
            included_path.file_name().unwrap().to_str().unwrap()
        );
        let main_path = create_temp_file(temp_dir.path(), "main.n1.md", &main_content);

        let result = expand_includes(&main_path).unwrap();
        assert_eq!(result.test_blocks.len(), 3);
        assert!(result.test_blocks[0].block.content.contains("test A"));
        assert!(result.test_blocks[1].block.content.contains("test B"));
        assert!(result.test_blocks[2].block.content.contains("test C"));
    }

    #[test]
    fn test_blocks_from_nested_includes() {
        let temp_dir = tempfile::tempdir().unwrap();
        let sub_dir = temp_dir.path().join("lib");
        fs::create_dir(&sub_dir).unwrap();

        let inner_content = "```n1test\n; inner test\n```\n";
        let inner_path = create_temp_file(&sub_dir, "inner.n1.md", inner_content);

        let middle_content = format!(
            "```n1asm\n.include \"{}\"\n```\n\n```n1test\n; middle test\n```\n",
            inner_path.file_name().unwrap().to_str().unwrap()
        );
        let _middle_path = create_temp_file(&sub_dir, "middle.n1.md", &middle_content);

        let include_path = String::from("lib/middle.n1.md");
        let main_content = format!(
            "```n1test\n; main test 1\n```\n\n```n1asm\n.include \"{include_path}\"\n```\n\n```n1test\n; main test 2\n```\n"
        );
        let main_path = create_temp_file(temp_dir.path(), "main.n1.md", &main_content);

        let result = expand_includes(&main_path).unwrap();
        assert_eq!(result.test_blocks.len(), 4);
        assert!(result.test_blocks[0].block.content.contains("main test 1"));
        assert!(result.test_blocks[1].block.content.contains("inner test"));
        assert!(result.test_blocks[2].block.content.contains("middle test"));
        assert!(result.test_blocks[3].block.content.contains("main test 2"));

        assert_eq!(result.test_blocks[1].include_chain.len(), 2);
    }

    #[test]
    fn cross_format_include_n1_into_n1md() {
        let temp_dir = tempfile::tempdir().unwrap();

        let plain_content = "ADD R0, R0, R1\nSUB R0, R0, R0\n";
        let plain_path = create_temp_file(temp_dir.path(), "utils.n1", plain_content);

        let literate_content = format!(
            "```n1asm\nMOV R0, #1\n.include \"{}\"\nHALT\n```\n",
            plain_path.file_name().unwrap().to_str().unwrap()
        );
        let literate_path = create_temp_file(temp_dir.path(), "main.n1.md", &literate_content);

        let result = expand_includes(&literate_path).unwrap();
        assert_eq!(result.lines.len(), 4);
        assert_eq!(result.lines[0].text, "MOV R0, #1");
        assert_eq!(result.lines[1].text, "ADD R0, R0, R1");
        assert_eq!(result.lines[2].text, "SUB R0, R0, R0");
        assert_eq!(result.lines[3].text, "HALT");

        assert_eq!(result.lines[1].include_chain.len(), 1);
        assert!(result.lines[1].include_chain[0]
            .from_file
            .ends_with("main.n1.md"));
    }

    #[test]
    fn cross_format_include_n1md_into_n1() {
        let temp_dir = tempfile::tempdir().unwrap();

        let literate_content = "```n1asm\nADD R0, R0, R1\n```\n";
        let literate_path = create_temp_file(temp_dir.path(), "utils.n1.md", literate_content);

        let plain_content = format!(
            "MOV R0, #1\n.include \"{}\"\nHALT\n",
            literate_path.file_name().unwrap().to_str().unwrap()
        );
        let plain_path = create_temp_file(temp_dir.path(), "main.n1", &plain_content);

        let result = expand_includes(&plain_path).unwrap();
        assert_eq!(result.lines.len(), 3);
        assert_eq!(result.lines[0].text, "MOV R0, #1");
        assert_eq!(result.lines[1].text, "ADD R0, R0, R1");
        assert_eq!(result.lines[2].text, "HALT");
    }

    #[test]
    fn tele7_directives_in_included_file() {
        let temp_dir = tempfile::tempdir().unwrap();

        let included_content = ".twchar \"AB\"\n.tstring \"HELLO\"\n";
        let included_path = create_temp_file(temp_dir.path(), "data.n1", included_content);

        let main_content = format!(
            ".org 0x4100\n.include \"{}\"\n",
            included_path.file_name().unwrap().to_str().unwrap()
        );
        let main_path = create_temp_file(temp_dir.path(), "main.n1", &main_content);

        let result = expand_includes(&main_path).unwrap();
        assert_eq!(result.lines.len(), 3);
        assert_eq!(result.lines[0].text, ".org 0x4100");
        assert_eq!(result.lines[1].text, ".twchar \"AB\"");
        assert_eq!(result.lines[2].text, ".tstring \"HELLO\"");
    }
}
