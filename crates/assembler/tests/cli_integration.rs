//! Integration tests for the nullbyte-asm CLI.

use assembler as _;
use emulator_core as _;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn binary_path() -> PathBuf {
    let mut path = std::env::current_exe().unwrap();
    path.pop();
    path.pop();
    path.join("nullbyte-asm")
}

fn create_temp_file(dir: &std::path::Path, name: &str, content: &str) -> PathBuf {
    let path = dir.join(name);
    fs::write(&path, content).unwrap();
    path
}

#[test]
fn build_simple_program() {
    let temp_dir = tempfile::tempdir().unwrap();
    let source = create_temp_file(temp_dir.path(), "simple.n1", "NOP\nHALT\n");

    let output = temp_dir.path().join("simple.bin");

    let status = Command::new(binary_path())
        .args([
            "build",
            source.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
        ])
        .status()
        .expect("failed to run nullbyte-asm");

    assert!(status.success());
    assert!(output.exists());

    let binary = fs::read(&output).unwrap();
    assert_eq!(binary.len(), 4);
    assert_eq!(&binary[0..2], &[0x00, 0x00]);
    assert_eq!(&binary[2..4], &[0x00, 0x10]);
}

#[test]
fn build_with_default_output() {
    let temp_dir = tempfile::tempdir().unwrap();
    let source = create_temp_file(temp_dir.path(), "test.n1", "NOP\n");

    let expected_output = temp_dir.path().join("test.bin");

    let status = Command::new(binary_path())
        .args(["build", source.to_str().unwrap()])
        .current_dir(temp_dir.path())
        .status()
        .expect("failed to run nullbyte-asm");

    assert!(status.success());
    assert!(expected_output.exists());
}

const LITERATE_CONTENT: &str = r"# Test

```n1asm
NOP
HALT
```
";

#[test]
fn build_literate_file() {
    let temp_dir = tempfile::tempdir().unwrap();
    let source = create_temp_file(temp_dir.path(), "lit.n1.md", LITERATE_CONTENT);

    let output = temp_dir.path().join("lit.bin");

    let status = Command::new(binary_path())
        .args([
            "build",
            source.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
        ])
        .status()
        .expect("failed to run nullbyte-asm");

    assert!(status.success());

    let binary = fs::read(&output).unwrap();
    assert_eq!(binary, &[0x00, 0x00, 0x00, 0x10]);
}

#[test]
fn build_reports_errors() {
    let temp_dir = tempfile::tempdir().unwrap();
    let source = create_temp_file(temp_dir.path(), "bad.n1", "INVALID_OPCODE\n");

    let output = Command::new(binary_path())
        .args(["build", source.to_str().unwrap()])
        .output()
        .expect("failed to run nullbyte-asm");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("error"));
}

#[test]
fn build_verbose_prints_listing() {
    let temp_dir = tempfile::tempdir().unwrap();
    let source = create_temp_file(temp_dir.path(), "verbose.n1", "NOP\nHALT\n");

    let output = temp_dir.path().join("verbose.bin");

    let result = Command::new(binary_path())
        .args([
            "build",
            source.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
            "--verbose",
        ])
        .output()
        .expect("failed to run nullbyte-asm");

    assert!(result.status.success());
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(stderr.contains("0000:"));
    assert!(stderr.contains("NOP"));
}

const PASSING_TEST_CONTENT: &str = r"# Test

```n1asm
NOP
HALT
```

```n1test
; PC should be at 0x0004 after NOP (2 bytes) and HALT (2 bytes)
PC == 0x0004
```
";

#[test]
fn test_with_passing_assertions() {
    let temp_dir = tempfile::tempdir().unwrap();
    let source = create_temp_file(temp_dir.path(), "pass.n1.md", PASSING_TEST_CONTENT);

    let result = Command::new(binary_path())
        .args(["test", source.to_str().unwrap()])
        .output()
        .expect("failed to run nullbyte-asm");

    let stdout = String::from_utf8_lossy(&result.stdout);
    let stderr = String::from_utf8_lossy(&result.stderr);

    assert!(
        result.status.success(),
        "test should pass\nstdout: {stdout}\nstderr: {stderr}"
    );
    assert!(stdout.contains("PASS"));
    assert!(stdout.contains("Test Summary"));
}

#[test]
fn test_with_no_test_blocks() {
    let temp_dir = tempfile::tempdir().unwrap();
    let source = create_temp_file(temp_dir.path(), "notests.n1", "NOP\nHALT\n");

    let result = Command::new(binary_path())
        .args(["test", source.to_str().unwrap()])
        .output()
        .expect("failed to run nullbyte-asm");

    let stdout = String::from_utf8_lossy(&result.stdout);

    assert!(result.status.success());
    assert!(stdout.contains("No test blocks"));
}

const FAILING_TEST_CONTENT: &str = r"# Test

```n1asm
NOP
HALT
```

```n1test
PC == 0xFFFF
```
";

#[test]
fn test_reports_failing_assertions() {
    let temp_dir = tempfile::tempdir().unwrap();
    let source = create_temp_file(temp_dir.path(), "fail.n1.md", FAILING_TEST_CONTENT);

    let result = Command::new(binary_path())
        .args(["test", source.to_str().unwrap()])
        .output()
        .expect("failed to run nullbyte-asm");

    assert!(!result.status.success());
    let stdout = String::from_utf8_lossy(&result.stdout);
    assert!(stdout.contains("FAIL"));
}

#[test]
fn help_shows_usage() {
    let result = Command::new(binary_path())
        .args(["--help"])
        .output()
        .expect("failed to run nullbyte-asm");

    let stdout = String::from_utf8_lossy(&result.stdout);
    assert!(stdout.contains("Commands:"));
    assert!(stdout.contains("build"));
    assert!(stdout.contains("test"));
}

#[test]
fn unknown_command_fails() {
    let result = Command::new(binary_path())
        .args(["unknown"])
        .output()
        .expect("failed to run nullbyte-asm");

    assert!(!result.status.success());
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(stderr.contains("unknown command"));
}

#[test]
fn blinker_program_tests_pass() {
    let blinker_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("programs/blinker.n1.md");

    if !blinker_path.exists() {
        eprintln!("Skipping blinker test - file not found at {blinker_path:?}");
        return;
    }

    let result = Command::new(binary_path())
        .args(["test", blinker_path.to_str().unwrap()])
        .output()
        .expect("failed to run nullbyte-asm");

    let stdout = String::from_utf8_lossy(&result.stdout);
    assert!(result.status.success(), "blinker tests failed:\n{stdout}");
    assert!(stdout.contains("Test Summary: 3 passed"));
}
