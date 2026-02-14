//! Source ingestion for plain and literate assembly formats.
//!
//! This module handles extracting assembly source from two formats:
//! - **Literate** (`.n1.md`): Markdown files where fenced code blocks tagged
//!   `n1asm` contain assembly code, and everything else is prose.
//! - **Plain** (`.n1` or other): The entire file is assembly source.
//!
//! Source mapping is preserved so error messages can reference the original
//! file's line numbers.

use std::path::Path;

/// A line of extracted source with its original location.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceLine {
    /// The source text (without trailing newline).
    pub text: String,
    /// 1-indexed line number in the original file.
    pub original_line: usize,
}

/// Extracted source content from an input file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceContent {
    /// All extracted source lines in document order.
    pub lines: Vec<SourceLine>,
    /// The file path (for error reporting).
    pub file_path: String,
}

/// Extracts assembly source from a file.
///
/// For `.n1.md` files, extracts content from fenced code blocks tagged `n1asm`.
/// For all other files, treats the entire content as assembly source.
#[must_use]
pub fn extract_source(file_path: &Path, content: &str) -> SourceContent {
    let file_path_str = file_path.to_string_lossy().to_string();

    if is_literate_file(file_path) {
        SourceContent {
            lines: extract_literate_source(content),
            file_path: file_path_str,
        }
    } else {
        SourceContent {
            lines: extract_plain_source(content),
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

/// Extracts source lines from literate (Markdown) format.
///
/// Scans for fenced code blocks with the `n1asm` language tag and extracts
/// their contents in document order. Lines carry their original file line
/// numbers for accurate error reporting.
fn extract_literate_source(content: &str) -> Vec<SourceLine> {
    let mut lines = Vec::new();
    let mut in_n1asm_block = false;
    let mut fence_len = 0;

    for (idx, line) in content.lines().enumerate() {
        let line_num = idx + 1;

        if let Some(fence_length) = is_fence_start(line) {
            if in_n1asm_block && fence_length >= fence_len {
                // End of current block (matching or longer fence)
                in_n1asm_block = false;
                fence_len = 0;
            } else if !in_n1asm_block {
                // Check if this fence starts an n1asm block
                let after_fence = &line[fence_length..];
                if after_fence.trim_start().starts_with("n1asm") {
                    in_n1asm_block = true;
                    fence_len = fence_length;
                }
            } else {
                // We're in a block but this fence is too short to end it
                // Treat as content (e.g., 3 backticks inside a 4-backtick fence)
                lines.push(SourceLine {
                    text: line.to_string(),
                    original_line: line_num,
                });
            }
        } else if in_n1asm_block {
            // Content line inside an n1asm block
            lines.push(SourceLine {
                text: line.to_string(),
                original_line: line_num,
            });
        }
    }

    lines
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
}
