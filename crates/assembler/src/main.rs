//! CLI entry point for the Nullbyte assembler binary.

use std::env;
use std::ffi::OsString;
use std::path::PathBuf;

use assembler as _;
use emulator_core as _;
#[cfg(test)]
use tempfile as _;

const HELP_TEXT: &str = "Usage: nullbyte-asm <input> [-o <output>] [--verbose] [--help]";

#[derive(Debug, PartialEq, Eq)]
struct CliArgs {
    input: PathBuf,
    output: Option<PathBuf>,
    verbose: bool,
}

#[derive(Debug)]
enum ParseResult {
    Args(CliArgs),
    Help,
}

fn parse_args(mut args: impl Iterator<Item = OsString>) -> Result<ParseResult, String> {
    let mut input: Option<PathBuf> = None;
    let mut output: Option<PathBuf> = None;
    let mut verbose = false;

    while let Some(arg) = args.next() {
        if arg == "--help" {
            return Ok(ParseResult::Help);
        }

        if arg == "--verbose" {
            verbose = true;
            continue;
        }

        if arg == "-o" {
            let value = args
                .next()
                .ok_or_else(|| String::from("missing value for -o"))?;
            output = Some(PathBuf::from(value));
            continue;
        }

        if arg.to_string_lossy().starts_with('-') {
            return Err(format!("unknown option: {}", arg.to_string_lossy()));
        }

        if input.is_some() {
            return Err(String::from("multiple input paths provided"));
        }
        input = Some(PathBuf::from(arg));
    }

    let input = input.ok_or_else(|| String::from("missing input path"))?;
    Ok(ParseResult::Args(CliArgs {
        input,
        output,
        verbose,
    }))
}

fn main() {
    match parse_args(env::args_os().skip(1)) {
        Ok(ParseResult::Help) => {
            println!("{HELP_TEXT}");
        }
        Ok(ParseResult::Args(args)) => {
            if args.verbose {
                eprintln!("verbose mode enabled");
            }
            let _ = args;
        }
        Err(error) => {
            eprintln!("error: {error}");
            eprintln!("{HELP_TEXT}");
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_args, CliArgs, ParseResult};
    use std::ffi::OsString;
    use std::path::PathBuf;

    #[test]
    fn parses_required_input() {
        let result = parse_args([OsString::from("program.n1")].into_iter())
            .expect("input-only args should parse");
        let ParseResult::Args(args) = result else {
            panic!("expected parsed args");
        };
        assert_eq!(
            args,
            CliArgs {
                input: PathBuf::from("program.n1"),
                output: None,
                verbose: false,
            }
        );
    }

    #[test]
    fn parses_output_and_verbose_flags() {
        let result = parse_args(
            [
                OsString::from("source.n1.md"),
                OsString::from("-o"),
                OsString::from("out.bin"),
                OsString::from("--verbose"),
            ]
            .into_iter(),
        )
        .expect("valid args should parse");
        let ParseResult::Args(args) = result else {
            panic!("expected parsed args");
        };
        assert_eq!(
            args,
            CliArgs {
                input: PathBuf::from("source.n1.md"),
                output: Some(PathBuf::from("out.bin")),
                verbose: true,
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
    fn rejects_unknown_flags() {
        let error = parse_args([OsString::from("--unknown")].into_iter())
            .expect_err("unknown flag should fail parse");
        assert!(error.contains("unknown option"));
    }
}
