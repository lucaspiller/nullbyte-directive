//! Source ingestion for plain and literate assembly formats.
//!
//! This module handles extracting assembly source from two formats:
//! - **Literate** (`.n1.md`): Markdown files where fenced code blocks tagged
//!   `n1asm` contain assembly code, and everything else is prose.
//! - **Plain** (`.n1` or other): The entire file is assembly source.
//!
//! Source mapping is preserved so error messages can reference the original
//! file's line numbers.
//!
//! Inline test blocks (`n1test` fenced code blocks) are also extracted from
//! literate files and collected separately for the test runner.

use std::path::Path;

/// A line of extracted source with its original location.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceLine {
    /// The source text (without trailing newline).
    pub text: String,
    /// 1-indexed line number in the original file.
    pub original_line: usize,
}

/// An extracted `n1test` block with source location.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestBlock {
    /// The raw text content of the block (without fence lines).
    pub content: String,
    /// 1-indexed line number where the block starts (the opening fence).
    pub start_line: usize,
    /// 1-indexed line number where the block ends (the closing fence).
    pub end_line: usize,
}

/// Extracted source content from an input file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceContent {
    /// All extracted assembly source lines in document order.
    pub lines: Vec<SourceLine>,
    /// All extracted `n1test` blocks in document order.
    pub test_blocks: Vec<TestBlock>,
    /// The file path (for error reporting).
    pub file_path: String,
}

/// Extracts assembly source from a file.
///
/// For `.n1.md` files, extracts content from fenced code blocks tagged `n1asm`
/// and `n1test`. `n1asm` blocks contribute to the assembly source; `n1test`
/// blocks are collected separately for inline testing.
/// For all other files, treats the entire content as assembly source.
#[must_use]
pub fn extract_source(file_path: &Path, content: &str) -> SourceContent {
    let file_path_str = file_path.to_string_lossy().to_string();

    if is_literate_file(file_path) {
        let (lines, test_blocks) = extract_literate_source(content);
        SourceContent {
            lines,
            test_blocks,
            file_path: file_path_str,
        }
    } else {
        SourceContent {
            lines: extract_plain_source(content),
            test_blocks: Vec::new(),
            file_path: file_path_str,
        }
    }
}

/// Returns true if the file should be treated as literate (Markdown) format.
fn is_literate_file(path: &Path) -> bool {
    let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let lower = file_name.to_ascii_lowercase();
    lower.ends_with(".n1.md")
        || path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("md"))
}

/// Extracts source lines from plain (non-Markdown) format.
///
/// Each line preserves its original line number.
fn extract_plain_source(content: &str) -> Vec<SourceLine> {
    content
        .lines()
        .enumerate()
        .map(|(idx, line)| SourceLine {
            text: line.to_string(),
            original_line: idx + 1,
        })
        .collect()
}

/// The type of fenced code block being parsed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BlockType {
    /// Assembly source block (`n1asm`).
    N1asm,
    /// Inline test block (`n1test`).
    N1test,
}

/// Extracts source lines and test blocks from literate (Markdown) format.
///
/// Scans for fenced code blocks with the `n1asm` or `n1test` language tags and
/// extracts their contents in document order. `n1asm` lines carry their original
/// file line numbers for accurate error reporting. `n1test` blocks are
/// collected separately with their start/end line numbers.
fn extract_literate_source(content: &str) -> (Vec<SourceLine>, Vec<TestBlock>) {
    let mut lines = Vec::new();
    let mut test_blocks = Vec::new();
    let mut current_block: Option<BlockType> = None;
    let mut fence_len = 0;
    let mut test_content = String::new();
    let mut test_start_line = 0;

    for (idx, line) in content.lines().enumerate() {
        let line_num = idx + 1;

        if let Some(fence_length) = is_fence_start(line) {
            if let Some(block_type) = current_block {
                if fence_length >= fence_len {
                    if block_type == BlockType::N1test {
                        test_blocks.push(TestBlock {
                            content: test_content.clone(),
                            start_line: test_start_line,
                            end_line: line_num,
                        });
                        test_content.clear();
                    }
                    current_block = None;
                    fence_len = 0;
                } else {
                    lines.push(SourceLine {
                        text: line.to_string(),
                        original_line: line_num,
                    });
                }
            } else {
                let after_fence = &line[fence_length..];
                let trimmed = after_fence.trim_start();
                if trimmed.starts_with("n1asm") {
                    current_block = Some(BlockType::N1asm);
                    fence_len = fence_length;
                } else if trimmed.starts_with("n1test") {
                    current_block = Some(BlockType::N1test);
                    fence_len = fence_length;
                    test_start_line = line_num;
                }
            }
        } else if let Some(block_type) = current_block {
            match block_type {
                BlockType::N1asm => {
                    lines.push(SourceLine {
                        text: line.to_string(),
                        original_line: line_num,
                    });
                }
                BlockType::N1test => {
                    if !test_content.is_empty() {
                        test_content.push('\n');
                    }
                    test_content.push_str(line);
                }
            }
        }
    }

    (lines, test_blocks)
}

/// Checks if a line is a fenced code block delimiter.
///
/// Returns the number of backticks if this is a fence start (>= 3 backticks),
/// or None otherwise.
fn is_fence_start(line: &str) -> Option<usize> {
    let trimmed = line.trim_start();
    if trimmed.starts_with("```") {
        let count = trimmed.chars().take_while(|&c| c == '`').count();
        if count >= 3 {
            return Some(count);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_file_passthrough() {
        let content = "MOV R0, #1\n; comment\nADD R0, R0, R1\n";
        let path = Path::new("test.n1");
        let result = extract_source(path, content);

        assert_eq!(result.lines.len(), 3);
        assert_eq!(result.lines[0].text, "MOV R0, #1");
        assert_eq!(result.lines[0].original_line, 1);
        assert_eq!(result.lines[1].text, "; comment");
        assert_eq!(result.lines[1].original_line, 2);
        assert_eq!(result.lines[2].text, "ADD R0, R0, R1");
        assert_eq!(result.lines[2].original_line, 3);
    }

    #[test]
    fn literate_single_block() {
        let content = r"# Title

Some prose here.

```n1asm
MOV R0, #1
ADD R0, R0, R1
```

More prose.
";
        let path = Path::new("test.n1.md");
        let result = extract_source(path, content);

        assert_eq!(result.lines.len(), 2);
        assert_eq!(result.lines[0].text, "MOV R0, #1");
        assert_eq!(result.lines[0].original_line, 6);
        assert_eq!(result.lines[1].text, "ADD R0, R0, R1");
        assert_eq!(result.lines[1].original_line, 7);
    }

    #[test]
    fn literate_multiple_blocks() {
        let content = r"# Title

```n1asm
init:
    MOV R0, #1
```

Some prose.

```n1asm
loop:
    ADD R0, R0, R0
    JMP #loop
```
";
        let path = Path::new("test.n1.md");
        let result = extract_source(path, content);

        assert_eq!(result.lines.len(), 5);
        // First block
        assert_eq!(result.lines[0].text, "init:");
        assert_eq!(result.lines[0].original_line, 4);
        assert_eq!(result.lines[1].text, "    MOV R0, #1");
        assert_eq!(result.lines[1].original_line, 5);
        // Second block
        assert_eq!(result.lines[2].text, "loop:");
        assert_eq!(result.lines[2].original_line, 11);
        assert_eq!(result.lines[3].text, "    ADD R0, R0, R0");
        assert_eq!(result.lines[3].original_line, 12);
        assert_eq!(result.lines[4].text, "    JMP #loop");
        assert_eq!(result.lines[4].original_line, 13);
    }

    #[test]
    fn literate_ignores_other_language_blocks() {
        let content = r#"
```rust
let x = 1;
```

```n1asm
MOV R0, #1
```

```python
print("hello")
```
"#;
        let path = Path::new("test.n1.md");
        let result = extract_source(path, content);

        assert_eq!(result.lines.len(), 1);
        assert_eq!(result.lines[0].text, "MOV R0, #1");
    }

    #[test]
    fn literate_empty_block() {
        let content = r"
```n1asm
```
";
        let path = Path::new("test.n1.md");
        let result = extract_source(path, content);

        assert_eq!(result.lines.len(), 0);
    }

    #[test]
    fn literate_no_blocks() {
        let content = r"# Title

Just prose, no code blocks.
";
        let path = Path::new("test.n1.md");
        let result = extract_source(path, content);

        assert_eq!(result.lines.len(), 0);
    }

    #[test]
    fn literate_preserves_indentation() {
        let content = r"
```n1asm
label:
    MOV R0, #1
        DEEP_INDENT
```
";
        let path = Path::new("test.n1.md");
        let result = extract_source(path, content);

        assert_eq!(result.lines.len(), 3);
        assert_eq!(result.lines[0].text, "label:");
        assert_eq!(result.lines[1].text, "    MOV R0, #1");
        assert_eq!(result.lines[2].text, "        DEEP_INDENT");
    }

    #[test]
    fn literate_four_backtick_fence() {
        let content = r"
````n1asm
MOV R0, #1
````
";
        let path = Path::new("test.n1.md");
        let result = extract_source(path, content);

        assert_eq!(result.lines.len(), 1);
        assert_eq!(result.lines[0].text, "MOV R0, #1");
    }

    #[test]
    fn literate_nested_backticks_in_block() {
        // 4-backtick fence allows 3-backtick content inside
        let content = r"
````n1asm
MOV R0, #1
``` this is not a fence end
ADD R0, R0, R0
````
";
        let path = Path::new("test.n1.md");
        let result = extract_source(path, content);

        assert_eq!(result.lines.len(), 3);
        assert_eq!(result.lines[0].text, "MOV R0, #1");
        assert_eq!(result.lines[1].text, "``` this is not a fence end");
        assert_eq!(result.lines[2].text, "ADD R0, R0, R0");
    }

    #[test]
    fn is_literate_file_detection() {
        assert!(is_literate_file(Path::new("test.n1.md")));
        assert!(is_literate_file(Path::new("foo.md")));
        assert!(is_literate_file(Path::new("path/to/test.n1.md")));
        assert!(!is_literate_file(Path::new("test.n1")));
        assert!(!is_literate_file(Path::new("test.asm")));
        assert!(!is_literate_file(Path::new("test")));
    }

    #[test]
    fn fence_start_detection() {
        assert_eq!(is_fence_start("```"), Some(3));
        assert_eq!(is_fence_start("```n1asm"), Some(3));
        assert_eq!(is_fence_start("  ```n1asm"), Some(3));
        assert_eq!(is_fence_start("````"), Some(4));
        assert_eq!(is_fence_start("``"), None);
        assert_eq!(is_fence_start("text"), None);
        assert_eq!(is_fence_start("``text"), None);
    }

    #[test]
    fn file_path_preserved() {
        let content = "MOV R0, #1\n";
        let path = Path::new("/some/path/test.n1");
        let result = extract_source(path, content);

        assert_eq!(result.file_path, "/some/path/test.n1");
    }

    #[test]
    fn literate_single_n1test_block() {
        let content = r"# Title

```n1test
R0 == 0x4000
R1 == 0x0001
```

More prose.
";
        let path = Path::new("test.n1.md");
        let result = extract_source(path, content);

        assert_eq!(result.test_blocks.len(), 1);
        assert_eq!(result.test_blocks[0].content, "R0 == 0x4000\nR1 == 0x0001");
        assert_eq!(result.test_blocks[0].start_line, 3);
        assert_eq!(result.test_blocks[0].end_line, 6);
    }

    #[test]
    fn literate_multiple_n1test_blocks() {
        let content = r"# Title

```n1test
R0 == 0x0001
```

Some prose.

```n1test
R1 == 0x0002
```
";
        let path = Path::new("test.n1.md");
        let result = extract_source(path, content);

        assert_eq!(result.test_blocks.len(), 2);
        assert_eq!(result.test_blocks[0].content, "R0 == 0x0001");
        assert_eq!(result.test_blocks[0].start_line, 3);
        assert_eq!(result.test_blocks[1].content, "R1 == 0x0002");
        assert_eq!(result.test_blocks[1].start_line, 9);
    }

    #[test]
    fn literate_mixed_n1asm_and_n1test() {
        let content = r"# Title

```n1asm
MOV R0, #1
HALT
```

```n1test
R0 == 0x0001
```

```n1asm
ADD R0, R0, R0
HALT
```

```n1test
R0 == 0x0002
```
";
        let path = Path::new("test.n1.md");
        let result = extract_source(path, content);

        assert_eq!(result.lines.len(), 4);
        assert_eq!(result.lines[0].text, "MOV R0, #1");
        assert_eq!(result.lines[1].text, "HALT");
        assert_eq!(result.lines[2].text, "ADD R0, R0, R0");
        assert_eq!(result.lines[3].text, "HALT");

        assert_eq!(result.test_blocks.len(), 2);
        assert_eq!(result.test_blocks[0].content, "R0 == 0x0001");
        assert_eq!(result.test_blocks[1].content, "R0 == 0x0002");
    }

    #[test]
    fn literate_empty_n1test_block() {
        let content = r"
```n1test
```
";
        let path = Path::new("test.n1.md");
        let result = extract_source(path, content);

        assert_eq!(result.test_blocks.len(), 1);
        assert_eq!(result.test_blocks[0].content, "");
    }

    #[test]
    fn plain_file_no_test_blocks() {
        let content = "MOV R0, #1\nHALT\n";
        let path = Path::new("test.n1");
        let result = extract_source(path, content);

        assert_eq!(result.lines.len(), 2);
        assert!(result.test_blocks.is_empty());
    }
}
