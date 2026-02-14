//! CLI entry point for the Nullbyte assembler binary.

use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use assembler as _;
use assembler::assembler::{assemble, AssembleError, AssembleResult};
use assembler::test_format::parse_test_block;
use assembler::test_runner::run_tests;
use emulator_core as _;
#[cfg(test)]
use tempfile as _;

const USAGE_TEXT: &str = "\
Usage: nullbyte-asm <command> [options]

Commands:
  build <input> [-o <output>] [--verbose]  Assemble source to binary
  test  <input>                            Assemble and run inline tests

Options:
  -o, --output <file>  Output file path (default: input stem + .bin)
  -v, --verbose        Print listing to stderr (build only)
  -h, --help           Show this help message

Examples:
  nullbyte-asm build program.n1.md
  nullbyte-asm build program.n1.md -o program.bin
  nullbyte-asm test program.n1.md
";

#[derive(Debug, PartialEq, Eq)]
enum Command {
    Build(BuildArgs),
    Test(TestArgs),
}

#[derive(Debug, PartialEq, Eq)]
struct BuildArgs {
    input: PathBuf,
    output: Option<PathBuf>,
    verbose: bool,
}

#[derive(Debug, PartialEq, Eq)]
struct TestArgs {
    input: PathBuf,
}

#[derive(Debug)]
enum ParseResult {
    Command(Command),
    Help,
}

fn parse_args(mut args: impl Iterator<Item = OsString>) -> Result<ParseResult, String> {
    let first = args.next().ok_or_else(|| "missing command".to_string())?;

    if first == "--help" || first == "-h" {
        return Ok(ParseResult::Help);
    }

    let command_str = first.to_string_lossy().to_string();

    match command_str.as_str() {
        "build" => parse_build_args(args)
            .map(Command::Build)
            .map(ParseResult::Command),
        "test" => parse_test_args(args)
            .map(Command::Test)
            .map(ParseResult::Command),
        other => Err(format!("unknown command: {other}")),
    }
}

#[allow(clippy::while_let_on_iterator)]
fn parse_build_args(mut args: impl Iterator<Item = OsString>) -> Result<BuildArgs, String> {
    let mut input: Option<PathBuf> = None;
    let mut output: Option<PathBuf> = None;
    let mut verbose = false;

    while let Some(arg) = args.next() {
        if arg == "--help" || arg == "-h" {
            return Err(USAGE_TEXT.to_string());
        }

        if arg == "--verbose" || arg == "-v" {
            verbose = true;
            continue;
        }

        if arg == "-o" || arg == "--output" {
            let value = args
                .next()
                .ok_or_else(|| "missing value for -o".to_string())?;
            output = Some(PathBuf::from(value));
            continue;
        }

        if arg.to_string_lossy().starts_with('-') {
            return Err(format!("unknown option: {}", arg.to_string_lossy()));
        }

        if input.is_some() {
            return Err("multiple input paths provided".to_string());
        }
        input = Some(PathBuf::from(arg));
    }

    let input = input.ok_or_else(|| "missing input path".to_string())?;
    Ok(BuildArgs {
        input,
        output,
        verbose,
    })
}

fn parse_test_args(args: impl Iterator<Item = OsString>) -> Result<TestArgs, String> {
    let mut input: Option<PathBuf> = None;

    for arg in args {
        if arg == "--help" || arg == "-h" {
            return Err(USAGE_TEXT.to_string());
        }

        if arg.to_string_lossy().starts_with('-') {
            return Err(format!("unknown option: {}", arg.to_string_lossy()));
        }

        if input.is_some() {
            return Err("multiple input paths provided".to_string());
        }
        input = Some(PathBuf::from(arg));
    }

    let input = input.ok_or_else(|| "missing input path".to_string())?;
    Ok(TestArgs { input })
}

fn default_output_path(input: &Path) -> PathBuf {
    let stem = input.file_stem().and_then(|s| s.to_str()).unwrap_or("out");

    let parent = input.parent().unwrap_or_else(|| std::path::Path::new(""));

    parent.join(format!("{stem}.bin"))
}

fn run_build(args: BuildArgs) -> Result<(), i32> {
    let result = match assemble(&args.input) {
        Ok(r) => r,
        Err(e) => {
            report_assemble_error(&e);
            return Err(1);
        }
    };

    for warning in &result.warnings {
        eprintln!("warning: {warning}");
    }

    let output_path = args
        .output
        .unwrap_or_else(|| default_output_path(&args.input));

    if let Err(e) = fs::write(&output_path, &result.binary) {
        eprintln!("error: failed to write output: {e}");
        return Err(1);
    }

    if args.verbose {
        print_listing(&result);
    }

    println!(
        "Assembled {} ({} bytes) -> {}",
        args.input.display(),
        result.binary.len(),
        output_path.display()
    );

    Ok(())
}

fn report_assemble_error(e: &AssembleError) {
    if let Some(loc) = &e.location {
        eprintln!("{}: error: {}", format_source_location(loc), e.kind);
    } else {
        eprintln!("error: {}", e.kind);
    }
}

fn format_source_location(loc: &assembler::assembler::SourceLocation) -> String {
    if loc.include_chain.is_empty() {
        format!("{}:{}", loc.file, loc.line)
    } else {
        format!("{}:{} ({})", loc.file, loc.line, loc.include_chain)
    }
}

fn print_listing(result: &AssembleResult) {
    for entry in &result.listing {
        let hex_bytes: String = entry
            .bytes
            .iter()
            .map(|b| format!("{b:02X}"))
            .collect::<Vec<_>>()
            .join(" ");

        eprintln!(
            "{:04X}: {:<12} {} ; {}",
            entry.address, hex_bytes, entry.source, entry.location
        );
    }
}

fn run_test(args: &TestArgs) -> Result<(), i32> {
    let result = match assemble(&args.input) {
        Ok(r) => r,
        Err(e) => {
            report_assemble_error(&e);
            return Err(1);
        }
    };

    if result.test_blocks.is_empty() {
        println!("No test blocks found in {}", args.input.display());
        return Ok(());
    }

    let parsed_blocks: Vec<_> = result
        .test_blocks
        .iter()
        .filter_map(|tbc| {
            parse_test_block(&tbc.block.content, tbc.block.start_line, tbc.block.end_line)
                .map_err(|e| {
                    eprintln!(
                        "error: failed to parse test block at {}: {}",
                        tbc.include_context, e
                    );
                })
                .ok()
        })
        .collect();

    if parsed_blocks.len() != result.test_blocks.len() {
        return Err(1);
    }

    let test_result = run_tests(&result.binary, &parsed_blocks);

    for block_result in &test_result.block_results {
        println!("{block_result}");

        if !block_result.passed() {
            for ar in &block_result.assertion_results {
                if !ar.passed {
                    println!("  {ar}");
                }
            }
        }
    }

    let summary = test_result.summary();
    println!();
    println!("Test Summary: {summary} (total: {})", summary.total);

    if test_result.all_passed() {
        Ok(())
    } else {
        Err(1)
    }
}

fn main() {
    let exit_code = match parse_args(env::args_os().skip(1)) {
        Ok(ParseResult::Help) => {
            println!("{USAGE_TEXT}");
            0
        }
        Ok(ParseResult::Command(Command::Build(args))) => match run_build(args) {
            Ok(()) => 0,
            Err(code) => code,
        },
        Ok(ParseResult::Command(Command::Test(args))) => match run_test(&args) {
            Ok(()) => 0,
            Err(code) => code,
        },
        Err(error) => {
            if error.starts_with("Usage:") {
                println!("{error}");
            } else {
                eprintln!("error: {error}");
                eprintln!("{USAGE_TEXT}");
            }
            1
        }
    };

    std::process::exit(exit_code);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;
    use std::path::PathBuf;

    #[test]
    fn parses_build_command() {
        let result = parse_build_args(
            [
                OsString::from("program.n1"),
                OsString::from("-o"),
                OsString::from("out.bin"),
                OsString::from("--verbose"),
            ]
            .into_iter(),
        )
        .expect("valid build args should parse");

        assert_eq!(
            result,
            BuildArgs {
                input: PathBuf::from("program.n1"),
                output: Some(PathBuf::from("out.bin")),
                verbose: true,
            }
        );
    }

    #[test]
    fn parses_test_command() {
        let result = parse_test_args([OsString::from("program.n1.md")].into_iter())
            .expect("valid test args should parse");

        assert_eq!(
            result,
            TestArgs {
                input: PathBuf::from("program.n1.md"),
            }
        );
    }

    #[test]
    fn parses_help_flag() {
        let result = parse_args([OsString::from("--help")].into_iter())
            .expect("help should parse without error");
        assert!(matches!(result, ParseResult::Help));
    }

    #[test]
    fn rejects_unknown_command() {
        let error = parse_args([OsString::from("unknown")].into_iter())
            .expect_err("unknown command should fail parse");
        assert!(error.contains("unknown command"));
    }

    #[test]
    fn default_output_path_simple() {
        let input = PathBuf::from("program.n1");
        let output = default_output_path(&input);
        assert_eq!(output, PathBuf::from("program.bin"));
    }

    #[test]
    fn default_output_path_with_dir() {
        let input = PathBuf::from("src/program.n1.md");
        let output = default_output_path(&input);
        assert_eq!(output, PathBuf::from("src/program.bin"));
    }

    #[test]
    fn default_output_path_no_extension() {
        let input = PathBuf::from("program");
        let output = default_output_path(&input);
        assert_eq!(output, PathBuf::from("program.bin"));
    }

    #[test]
    fn parse_build_short_flags() {
        let result = parse_build_args([OsString::from("src.n1"), OsString::from("-v")].into_iter())
            .expect("short flags should parse");

        assert!(result.verbose);
    }

    #[test]
    fn parse_build_missing_input() {
        let error = parse_build_args(std::iter::empty()).expect_err("missing input should fail");
        assert!(error.contains("missing input"));
    }

    #[test]
    fn parse_test_rejects_options() {
        let error = parse_test_args([OsString::from("--verbose")].into_iter())
            .expect_err("test should reject options");
        assert!(error.contains("unknown option"));
    }
}
