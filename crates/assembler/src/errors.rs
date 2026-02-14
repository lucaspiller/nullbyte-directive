//! Structured error reporting for assembler phases.
//!
//! This module provides a unified error type hierarchy with:
//! - Source location tracking (file, line, column)
//! - Include-chain traces for errors in included files
//! - Test error types for inline test failures
//! - Multi-error collection for reporting all errors at once
//!
//! # Error Format
//!
//! All errors format to stderr in the standard style:
//! ```text
//! file.n1:10:5: error: message
//! ```
//!
//! For errors in included files:
//! ```text
//! lib.n1:5:1: error: unknown mnemonic (included from main.n1:3)
//! ```

use std::fmt;
use std::path::PathBuf;

use crate::encoder::EncodeError;
use crate::include::IncludeError;
use crate::parser::ParseError;
use crate::symbols::SymbolError;
use crate::test_format::ParseAssertionError;
use crate::test_runner::{AssertionResult, TestBlockResult};

/// A source location for error reporting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceLoc {
    /// File path.
    pub file: PathBuf,
    /// 1-indexed line number.
    pub line: usize,
    /// 1-indexed column number (1 if unknown).
    pub column: usize,
    /// Include chain (outermost first).
    pub include_chain: Vec<IncludeTraceEntry>,
}

impl SourceLoc {
    /// Creates a new source location.
    #[must_use]
    pub const fn new(file: PathBuf, line: usize, column: usize) -> Self {
        Self {
            file,
            line,
            column,
            include_chain: Vec::new(),
        }
    }

    /// Creates a source location with an include chain.
    #[must_use]
    pub fn with_include_chain(mut self, chain: Vec<IncludeTraceEntry>) -> Self {
        self.include_chain = chain;
        self
    }

    /// Formats the location without the include chain.
    #[must_use]
    pub fn format_location(&self) -> String {
        format!("{}:{}:{}", self.file.display(), self.line, self.column)
    }

    /// Formats the full location with include chain.
    #[must_use]
    pub fn format_full(&self) -> String {
        if self.include_chain.is_empty() {
            self.format_location()
        } else {
            let mut parts = vec![self.format_location()];
            for entry in self.include_chain.iter().rev() {
                parts.push(format!(
                    "included from {}:{}",
                    entry.file.display(),
                    entry.line
                ));
            }
            parts.join(" (") + &")".repeat(self.include_chain.len())
        }
    }
}

impl fmt::Display for SourceLoc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.format_full())
    }
}

/// An entry in an include chain trace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncludeTraceEntry {
    /// The file that contained the `.include` directive.
    pub file: PathBuf,
    /// The line number of the `.include` directive.
    pub line: usize,
}

/// A unified assembler error with source context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssemblerError {
    /// The kind of error.
    pub kind: AssemblerErrorKind,
    /// Source location if available.
    pub location: Option<SourceLoc>,
}

impl AssemblerError {
    /// Creates a new assembler error.
    #[must_use]
    pub const fn new(kind: AssemblerErrorKind) -> Self {
        Self {
            kind,
            location: None,
        }
    }

    /// Adds a source location to the error.
    #[must_use]
    pub fn with_location(mut self, loc: SourceLoc) -> Self {
        self.location = Some(loc);
        self
    }

    /// Formats the error for stderr output.
    #[must_use]
    pub fn format_for_stderr(&self) -> String {
        self.location.as_ref().map_or_else(
            || format!("error: {}", self.kind),
            |loc| format!("{}: error: {}", loc.format_full(), self.kind),
        )
    }
}

impl fmt::Display for AssemblerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.location {
            Some(loc) => write!(f, "{}: {}", loc.format_full(), self.kind),
            None => write!(f, "{}", self.kind),
        }
    }
}

impl std::error::Error for AssemblerError {}

impl From<ParseError> for AssemblerError {
    fn from(e: ParseError) -> Self {
        Self {
            kind: AssemblerErrorKind::Parse(e.clone()),
            location: Some(SourceLoc {
                file: PathBuf::new(),
                line: e.location.line,
                column: e.location.column,
                include_chain: Vec::new(),
            }),
        }
    }
}

impl From<SymbolError> for AssemblerError {
    fn from(e: SymbolError) -> Self {
        Self {
            kind: AssemblerErrorKind::Symbol(e),
            location: None,
        }
    }
}

impl From<EncodeError> for AssemblerError {
    fn from(e: EncodeError) -> Self {
        Self {
            kind: AssemblerErrorKind::Encode(e),
            location: None,
        }
    }
}

impl From<IncludeError> for AssemblerError {
    fn from(e: IncludeError) -> Self {
        let chain: Vec<IncludeTraceEntry> = e
            .include_chain
            .iter()
            .map(|entry| IncludeTraceEntry {
                file: entry.from_file.clone(),
                line: entry.line,
            })
            .collect();

        Self {
            kind: AssemblerErrorKind::Include(e.clone()),
            location: Some(SourceLoc::new(e.path, 1, 1).with_include_chain(chain)),
        }
    }
}

/// Classification of assembler errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssemblerErrorKind {
    /// Parse error during source line parsing.
    Parse(ParseError),
    /// Symbol table error (duplicate label, address overflow).
    Symbol(SymbolError),
    /// Encoding error (undefined label, displacement out of range).
    Encode(EncodeError),
    /// Include expansion error (file not found, circular include).
    Include(IncludeError),
    /// I/O error reading source file.
    Io(String),
}

impl fmt::Display for AssemblerErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(e) => write!(f, "{e}"),
            Self::Symbol(e) => write!(f, "{e}"),
            Self::Encode(e) => write!(f, "{e}"),
            Self::Include(e) => write!(f, "{e}"),
            Self::Io(msg) => write!(f, "I/O error: {msg}"),
        }
    }
}

/// A collection of multiple errors.
#[derive(Debug, Clone, Default)]
pub struct ErrorCollection {
    errors: Vec<AssemblerError>,
}

impl ErrorCollection {
    /// Creates an empty error collection.
    #[must_use]
    pub const fn new() -> Self {
        Self { errors: Vec::new() }
    }

    /// Adds an error to the collection.
    pub fn push(&mut self, error: AssemblerError) {
        self.errors.push(error);
    }

    /// Returns true if the collection is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.errors.is_empty()
    }

    /// Returns the number of errors.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.errors.len()
    }

    /// Returns an iterator over the errors.
    pub fn iter(&self) -> impl Iterator<Item = &AssemblerError> {
        self.errors.iter()
    }

    /// Formats all errors for stderr output.
    #[must_use]
    pub fn format_for_stderr(&self) -> String {
        self.errors
            .iter()
            .map(AssemblerError::format_for_stderr)
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Returns the first error, if any.
    #[must_use]
    pub fn first(&self) -> Option<&AssemblerError> {
        self.errors.first()
    }

    /// Converts into a single error if there is exactly one.
    #[must_use]
    pub fn into_single(self) -> Option<AssemblerError> {
        if self.errors.len() == 1 {
            self.errors.into_iter().next()
        } else {
            None
        }
    }
}

impl fmt::Display for ErrorCollection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, error) in self.errors.iter().enumerate() {
            if i > 0 {
                writeln!(f)?;
            }
            write!(f, "{error}")?;
        }
        Ok(())
    }
}

impl std::error::Error for ErrorCollection {}

impl FromIterator<AssemblerError> for ErrorCollection {
    fn from_iter<T: IntoIterator<Item = AssemblerError>>(iter: T) -> Self {
        Self {
            errors: iter.into_iter().collect(),
        }
    }
}

/// Test runner error types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TestError {
    /// Assertion failure with expected vs actual.
    AssertionFailure {
        /// Source location of the test block.
        location: SourceLoc,
        /// The failing assertion.
        assertion: String,
        /// Expected value.
        expected: String,
        /// Actual value.
        actual: String,
    },
    /// CPU fault before HALT was reached.
    CpuFault {
        /// Source location of the test block.
        location: SourceLoc,
        /// Fault message.
        message: String,
    },
    /// More test blocks than HALTs in the program.
    TestHaltMismatch {
        /// Number of test blocks.
        test_blocks: usize,
        /// Number of HALTs reached.
        halts_reached: usize,
        /// Number of unexecuted blocks.
        unexecuted: usize,
    },
    /// Malformed assertion syntax.
    MalformedAssertion {
        /// Source location of the test block.
        location: SourceLoc,
        /// Parse error details.
        error: ParseAssertionError,
    },
}

impl TestError {
    /// Creates an assertion failure error from test runner results.
    #[must_use]
    pub fn from_assertion_failure(location: SourceLoc, result: &AssertionResult) -> Self {
        Self::AssertionFailure {
            location,
            assertion: format!("{:?}", result.assertion),
            expected: format!("{:?}", result.assertion),
            actual: result.actual.clone(),
        }
    }

    /// Creates a CPU fault error from test runner results.
    #[must_use]
    pub fn from_cpu_fault(location: SourceLoc, result: &TestBlockResult) -> Self {
        Self::CpuFault {
            location,
            message: result.fault_message.clone().unwrap_or_default(),
        }
    }

    /// Formats the error for stderr output.
    #[must_use]
    pub fn format_for_stderr(&self) -> String {
        match self {
            Self::AssertionFailure {
                location,
                assertion,
                expected: _,
                actual,
            } => {
                format!(
                    "{}: error: assertion failed: {} (got {})",
                    location.format_full(),
                    assertion,
                    actual
                )
            }
            Self::CpuFault { location, message } => {
                format!(
                    "{}: error: CPU fault before HALT: {}",
                    location.format_full(),
                    message
                )
            }
            Self::TestHaltMismatch {
                test_blocks,
                halts_reached,
                unexecuted,
            } => {
                format!(
                    "error: test/HALT mismatch: {test_blocks} test blocks, {halts_reached} HALTs reached, {unexecuted} unexecuted"
                )
            }
            Self::MalformedAssertion { location, error } => {
                format!(
                    "{}: error: malformed assertion: {}",
                    location.format_full(),
                    error
                )
            }
        }
    }
}

impl fmt::Display for TestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AssertionFailure {
                location,
                assertion,
                expected: _,
                actual,
            } => {
                write!(
                    f,
                    "{}: assertion failed: {} (got {})",
                    location.format_full(),
                    assertion,
                    actual
                )
            }
            Self::CpuFault { location, message } => {
                write!(
                    f,
                    "{}: CPU fault before HALT: {}",
                    location.format_full(),
                    message
                )
            }
            Self::TestHaltMismatch {
                test_blocks,
                halts_reached,
                unexecuted,
            } => {
                write!(
                    f,
                    "test/HALT mismatch: {test_blocks} test blocks, {halts_reached} HALTs reached, {unexecuted} unexecuted"
                )
            }
            Self::MalformedAssertion { location, error } => {
                write!(
                    f,
                    "{}: malformed assertion: {}",
                    location.format_full(),
                    error
                )
            }
        }
    }
}

impl std::error::Error for TestError {}

/// A collection of test errors.
#[derive(Debug, Clone, Default)]
pub struct TestErrorCollection {
    errors: Vec<TestError>,
}

impl TestErrorCollection {
    /// Creates an empty collection.
    #[must_use]
    pub const fn new() -> Self {
        Self { errors: Vec::new() }
    }

    /// Adds an error to the collection.
    pub fn push(&mut self, error: TestError) {
        self.errors.push(error);
    }

    /// Returns true if the collection is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.errors.is_empty()
    }

    /// Returns the number of errors.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.errors.len()
    }

    /// Formats all errors for stderr output.
    #[must_use]
    pub fn format_for_stderr(&self) -> String {
        self.errors
            .iter()
            .map(TestError::format_for_stderr)
            .collect::<Vec<_>>()
            .join("\n")
    }
}

impl fmt::Display for TestErrorCollection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, error) in self.errors.iter().enumerate() {
            if i > 0 {
                writeln!(f)?;
            }
            write!(f, "{error}")?;
        }
        Ok(())
    }
}

impl std::error::Error for TestErrorCollection {}

/// Result type for multi-error operations.
pub type MultiResult<T> = Result<T, ErrorCollection>;

/// Result type for test operations.
pub type TestResult<T> = Result<T, TestErrorCollection>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_loc_format_simple() {
        let loc = SourceLoc::new(PathBuf::from("main.n1"), 10, 5);
        assert_eq!(loc.format_location(), "main.n1:10:5");
        assert_eq!(loc.format_full(), "main.n1:10:5");
    }

    #[test]
    fn source_loc_format_with_include_chain() {
        let loc = SourceLoc::new(PathBuf::from("lib.n1"), 5, 1).with_include_chain(vec![
            IncludeTraceEntry {
                file: PathBuf::from("main.n1"),
                line: 3,
            },
        ]);
        assert_eq!(loc.format_full(), "lib.n1:5:1 (included from main.n1:3)");
    }

    #[test]
    fn source_loc_format_with_nested_include() {
        let loc = SourceLoc::new(PathBuf::from("inner.n1"), 7, 1).with_include_chain(vec![
            IncludeTraceEntry {
                file: PathBuf::from("main.n1"),
                line: 2,
            },
            IncludeTraceEntry {
                file: PathBuf::from("middle.n1"),
                line: 4,
            },
        ]);
        assert_eq!(
            loc.format_full(),
            "inner.n1:7:1 (included from middle.n1:4 (included from main.n1:2))"
        );
    }

    #[test]
    fn assembler_error_format_no_location() {
        let error = AssemblerError::new(AssemblerErrorKind::Io("file not found".into()));
        assert_eq!(
            error.format_for_stderr(),
            "error: I/O error: file not found"
        );
    }

    #[test]
    fn assembler_error_format_with_location() {
        let loc = SourceLoc::new(PathBuf::from("test.n1"), 5, 1);
        let error =
            AssemblerError::new(AssemblerErrorKind::Io("read error".into())).with_location(loc);
        assert_eq!(
            error.format_for_stderr(),
            "test.n1:5:1: error: I/O error: read error"
        );
    }

    #[test]
    fn error_collection_empty() {
        let collection = ErrorCollection::new();
        assert!(collection.is_empty());
        assert_eq!(collection.len(), 0);
    }

    #[test]
    fn error_collection_push() {
        let mut collection = ErrorCollection::new();
        collection.push(AssemblerError::new(AssemblerErrorKind::Io(
            "error 1".into(),
        )));
        collection.push(AssemblerError::new(AssemblerErrorKind::Io(
            "error 2".into(),
        )));

        assert!(!collection.is_empty());
        assert_eq!(collection.len(), 2);
    }

    #[test]
    fn error_collection_format() {
        let mut collection = ErrorCollection::new();
        let loc1 = SourceLoc::new(PathBuf::from("a.n1"), 1, 1);
        let loc2 = SourceLoc::new(PathBuf::from("b.n1"), 2, 1);
        collection.push(
            AssemblerError::new(AssemblerErrorKind::Io("error 1".into())).with_location(loc1),
        );
        collection.push(
            AssemblerError::new(AssemblerErrorKind::Io("error 2".into())).with_location(loc2),
        );

        let output = collection.format_for_stderr();
        assert!(output.contains("a.n1:1:1: error: I/O error: error 1"));
        assert!(output.contains("b.n1:2:1: error: I/O error: error 2"));
    }

    #[test]
    fn test_error_assertion_failure() {
        let loc = SourceLoc::new(PathBuf::from("test.n1.md"), 10, 1);
        let error = TestError::AssertionFailure {
            location: loc,
            assertion: "R0 == 0x1234".into(),
            expected: "0x1234".into(),
            actual: "0x0000".into(),
        };

        let output = error.format_for_stderr();
        assert!(output.contains("test.n1.md:10:1"));
        assert!(output.contains("assertion failed"));
        assert!(output.contains("0x0000"));
    }

    #[test]
    fn test_error_cpu_fault() {
        let loc = SourceLoc::new(PathBuf::from("test.n1.md"), 15, 1);
        let error = TestError::CpuFault {
            location: loc,
            message: "illegal opcode".into(),
        };

        let output = error.format_for_stderr();
        assert!(output.contains("test.n1.md:15:1"));
        assert!(output.contains("CPU fault before HALT"));
        assert!(output.contains("illegal opcode"));
    }

    #[test]
    fn test_error_mismatch() {
        let error = TestError::TestHaltMismatch {
            test_blocks: 5,
            halts_reached: 3,
            unexecuted: 2,
        };

        let output = error.format_for_stderr();
        assert!(output.contains("5 test blocks"));
        assert!(output.contains("3 HALTs reached"));
        assert!(output.contains("2 unexecuted"));
    }

    #[test]
    fn test_error_collection() {
        let mut collection = TestErrorCollection::new();
        let loc = SourceLoc::new(PathBuf::from("test.n1.md"), 5, 1);
        collection.push(TestError::AssertionFailure {
            location: loc,
            assertion: "R0 == 0x0001".into(),
            expected: "0x0001".into(),
            actual: "0x0000".into(),
        });

        assert!(!collection.is_empty());
        assert_eq!(collection.len(), 1);
    }

    #[test]
    fn error_from_parse_error() {
        use crate::parser::{ParseError as InnerParseError, ParseErrorKind, SourceLocation};

        let parse_err = InnerParseError {
            location: SourceLocation {
                line: 10,
                column: 5,
            },
            kind: ParseErrorKind::UnknownMnemonic("FOO".into()),
        };

        let asm_err = AssemblerError::from(parse_err);
        assert!(matches!(asm_err.kind, AssemblerErrorKind::Parse(_)));
        assert!(asm_err.location.is_some());
        let loc = asm_err.location.unwrap();
        assert_eq!(loc.line, 10);
        assert_eq!(loc.column, 5);
    }

    #[test]
    fn error_from_include_error() {
        use crate::include::{IncludeEntry, IncludeError, IncludeErrorKind};

        let include_err = IncludeError {
            path: PathBuf::from("lib.n1"),
            include_chain: vec![IncludeEntry {
                from_file: PathBuf::from("main.n1"),
                line: 5,
            }],
            kind: IncludeErrorKind::FileNotFound,
        };

        let asm_err = AssemblerError::from(include_err);
        assert!(matches!(asm_err.kind, AssemblerErrorKind::Include(_)));
        assert!(asm_err.location.is_some());
        let loc = asm_err.location.unwrap();
        assert_eq!(loc.file, PathBuf::from("lib.n1"));
        assert_eq!(loc.include_chain.len(), 1);
        assert_eq!(loc.include_chain[0].file, PathBuf::from("main.n1"));
        assert_eq!(loc.include_chain[0].line, 5);
    }

    #[test]
    fn multi_result_ok() {
        let result: MultiResult<i32> = Ok(42);
        assert!(matches!(result, Ok(42)));
    }

    #[test]
    fn multi_result_err() {
        let mut collection = ErrorCollection::new();
        collection.push(AssemblerError::new(AssemblerErrorKind::Io("error".into())));
        let result: MultiResult<i32> = Err(collection);
        assert!(result.is_err());
        let Err(coll) = result else {
            panic!("expected Err")
        };
        assert_eq!(coll.len(), 1);
    }

    #[test]
    fn test_result_ok() {
        let result: TestResult<i32> = Ok(42);
        assert!(matches!(result, Ok(42)));
    }

    #[test]
    fn test_result_err() {
        let mut collection = TestErrorCollection::new();
        collection.push(TestError::TestHaltMismatch {
            test_blocks: 1,
            halts_reached: 0,
            unexecuted: 1,
        });
        let result: TestResult<i32> = Err(collection);
        assert!(result.is_err());
        let Err(coll) = result else {
            panic!("expected Err")
        };
        assert_eq!(coll.len(), 1);
    }

    #[test]
    fn error_collection_from_iterator() {
        let errors = vec![
            AssemblerError::new(AssemblerErrorKind::Io("error 1".into())),
            AssemblerError::new(AssemblerErrorKind::Io("error 2".into())),
        ];

        let collection: ErrorCollection = errors.into_iter().collect();
        assert_eq!(collection.len(), 2);
    }

    #[test]
    fn test_error_malformed_assertion() {
        use crate::test_format::ParseAssertionError;

        let loc = SourceLoc::new(PathBuf::from("test.n1.md"), 20, 1);
        let parse_err = ParseAssertionError {
            line_in_block: 2,
            text: "R8 == 0x0001".into(),
            message: "unknown register 'R8'".into(),
        };

        let error = TestError::MalformedAssertion {
            location: loc,
            error: parse_err,
        };

        let output = error.format_for_stderr();
        assert!(output.contains("test.n1.md:20:1"));
        assert!(output.contains("malformed assertion"));
        assert!(output.contains("unknown register"));
    }

    #[test]
    fn error_collection_into_single() {
        let mut collection = ErrorCollection::new();
        collection.push(AssemblerError::new(AssemblerErrorKind::Io("error".into())));

        let single = collection.into_single();
        assert!(single.is_some());

        let empty = ErrorCollection::new();
        assert!(empty.into_single().is_none());

        let mut multi = ErrorCollection::new();
        multi.push(AssemblerError::new(AssemblerErrorKind::Io("e1".into())));
        multi.push(AssemblerError::new(AssemblerErrorKind::Io("e2".into())));
        assert!(multi.into_single().is_none());
    }
}
