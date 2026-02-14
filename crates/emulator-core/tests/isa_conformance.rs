//! ISA conformance tests using literate markdown programs.

use emulator_core as _;
use proptest as _;
use rstest as _;
#[cfg(feature = "serde")]
use serde as _;
use thiserror as _;

use std::path::PathBuf;
use std::process::Command;

fn assembler_binary() -> PathBuf {
    let mut path = std::env::current_exe().unwrap();
    path.pop();
    path.pop();
    path.join("nullbyte-asm")
}

fn isa_test_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("isa")
}

fn run_isa_test(name: &str) -> (bool, String) {
    let test_path = isa_test_dir().join(name);

    if !test_path.exists() {
        return (
            false,
            format!("Test file not found: {}", test_path.display()),
        );
    }

    let output = Command::new(assembler_binary())
        .args(["test", test_path.to_str().unwrap()])
        .output()
        .expect("failed to run nullbyte-asm");

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let success = output.status.success();

    (success, stdout)
}

macro_rules! isa_test {
    ($name:ident, $filename:expr) => {
        #[test]
        fn $name() {
            let (success, output) = run_isa_test($filename);
            assert!(success, "ISA test failed:\n{output}");
        }
    };
}

isa_test!(isa_control, "control.n1.md");
isa_test!(isa_data_movement, "data_movement.n1.md");
isa_test!(isa_alu, "alu.n1.md");
isa_test!(isa_math, "math.n1.md");
isa_test!(isa_branch, "branch.n1.md");
isa_test!(isa_stack, "stack.n1.md");
isa_test!(isa_mmio, "mmio.n1.md");
isa_test!(isa_atomic, "atomic.n1.md");
isa_test!(isa_event, "event.n1.md");
