//! HALT-driven test execution engine for inline test blocks.
//!
//! This module runs assembled binaries through `emulator-core` and evaluates
//! `n1test` assertions at each HALT boundary.
//!
//! ## Execution Model
//!
//! 1. Load assembled binary into an `emulator-core` instance at address 0x0000.
//! 2. For each `n1test` block in document order:
//!    a. Execute until HALT (or fault).
//!    b. Evaluate all assertions against current machine state.
//!    c. Report failures with expected vs. actual values.
//!    d. Resume execution (un-halt) for the next test block.
//! 3. Report summary: passed, failed, total.

#![allow(
    clippy::uninlined_format_args,
    clippy::redundant_closure,
    clippy::option_if_let_else,
    clippy::manual_strip,
    clippy::unnecessary_struct_initialization,
    clippy::unreadable_literal,
    clippy::useless_conversion,
    clippy::needless_collect,
    clippy::missing_const_for_fn,
    clippy::redundant_closure_for_method_calls,
    clippy::similar_names
)]

use std::fmt;

use emulator_core::{
    CoreConfig, CoreState, GeneralRegister, MmioBus, MmioError, MmioWriteResult, RunBoundary,
    RunState, StepOutcome,
};

use crate::test_format::{Assertion, ComparisonOp, ParsedTestBlock, Register};

/// Result of evaluating a single assertion against machine state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssertionResult {
    /// The original assertion that was evaluated.
    pub assertion: Assertion,
    /// Whether the assertion passed.
    pub passed: bool,
    /// The actual value observed (for failure reporting).
    pub actual: String,
}

/// Result of running a single test block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestBlockResult {
    /// 1-indexed source line where the test block starts.
    pub start_line: usize,
    /// 1-indexed source line where the test block ends.
    pub end_line: usize,
    /// Results for each assertion in the block.
    pub assertion_results: Vec<AssertionResult>,
    /// Whether the CPU faulted before reaching HALT.
    pub faulted: bool,
    /// Fault message if faulted.
    pub fault_message: Option<String>,
}

impl TestBlockResult {
    /// Returns true if all assertions passed and no fault occurred.
    #[must_use]
    pub fn passed(&self) -> bool {
        !self.faulted && self.assertion_results.iter().all(|r| r.passed)
    }
}

/// Result of running all test blocks for a program.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestRunResult {
    /// Results for each test block in order.
    pub block_results: Vec<TestBlockResult>,
    /// Number of test blocks that were not executed (more blocks than HALTs).
    pub unexecuted_blocks: usize,
}

impl TestRunResult {
    /// Returns true if all executed test blocks passed.
    #[must_use]
    pub fn all_passed(&self) -> bool {
        self.unexecuted_blocks == 0 && self.block_results.iter().all(|b| b.passed())
    }

    /// Returns counts for summary reporting.
    #[must_use]
    pub fn summary(&self) -> TestSummary {
        let passed = self.block_results.iter().filter(|b| b.passed()).count();
        let failed = self.block_results.len() - passed;
        TestSummary {
            passed,
            failed,
            unexecuted: self.unexecuted_blocks,
            total: self.block_results.len() + self.unexecuted_blocks,
        }
    }
}

/// Summary counts for test run reporting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TestSummary {
    /// Number of test blocks that passed.
    pub passed: usize,
    /// Number of test blocks that failed.
    pub failed: usize,
    /// Number of test blocks that were not executed.
    pub unexecuted: usize,
    /// Total number of test blocks.
    pub total: usize,
}

/// Runs all test blocks against an assembled binary.
///
/// # Arguments
///
/// * `binary` - The assembled ROM image to load at address 0x0000.
/// * `test_blocks` - Parsed test blocks with assertions in document order.
///
/// # Returns
///
/// A `TestRunResult` with results for each test block.
#[must_use]
pub fn run_tests(binary: &[u8], test_blocks: &[ParsedTestBlock]) -> TestRunResult {
    let config = CoreConfig::default();
    let mut state = CoreState::with_config(&config);

    load_binary(&mut state, binary);

    let mut mmio = NullMmio;
    let mut block_results = Vec::new();

    for block in test_blocks {
        let result = run_test_block(&mut state, &config, &mut mmio, block);
        block_results.push(result);

        if matches!(state.run_state, RunState::FaultLatched(_)) {
            let remaining = test_blocks.len() - block_results.len();
            return TestRunResult {
                block_results,
                unexecuted_blocks: remaining,
            };
        }
    }

    TestRunResult {
        block_results,
        unexecuted_blocks: 0,
    }
}

/// Loads a binary image into ROM starting at address 0x0000.
fn load_binary(state: &mut CoreState, binary: &[u8]) {
    let len = binary.len().min(state.memory.len());
    state.memory[..len].copy_from_slice(&binary[..len]);
}

/// Runs a single test block to the next HALT and evaluates assertions.
fn run_test_block(
    state: &mut CoreState,
    config: &CoreConfig,
    mmio: &mut dyn MmioBus,
    block: &ParsedTestBlock,
) -> TestBlockResult {
    if matches!(state.run_state, RunState::FaultLatched(_)) {
        return TestBlockResult {
            start_line: block.start_line,
            end_line: block.end_line,
            assertion_results: Vec::new(),
            faulted: true,
            fault_message: Some(format!("CPU already faulted: {:?}", state.run_state)),
        };
    }

    if state.run_state == RunState::HaltedForTick {
        state.run_state = RunState::Running;
    }

    let outcome = emulator_core::run_one(state, mmio, config, RunBoundary::Halted);

    match outcome.final_step {
        StepOutcome::HaltedForTick => {
            let assertion_results = evaluate_assertions(state, &block.assertions);
            TestBlockResult {
                start_line: block.start_line,
                end_line: block.end_line,
                assertion_results,
                faulted: false,
                fault_message: None,
            }
        }
        StepOutcome::Fault { cause } => {
            let assertion_results = evaluate_assertions(state, &block.assertions);
            TestBlockResult {
                start_line: block.start_line,
                end_line: block.end_line,
                assertion_results,
                faulted: true,
                fault_message: Some(format!("CPU faulted before HALT: {:?}", cause)),
            }
        }
        StepOutcome::TrapDispatch { cause } => TestBlockResult {
            start_line: block.start_line,
            end_line: block.end_line,
            assertion_results: Vec::new(),
            faulted: true,
            fault_message: Some(format!("Unexpected TRAP dispatch (cause={:#06X})", cause)),
        },
        StepOutcome::EventDispatch { event_id } => TestBlockResult {
            start_line: block.start_line,
            end_line: block.end_line,
            assertion_results: Vec::new(),
            faulted: true,
            fault_message: Some(format!("Unexpected EVENT dispatch (id={:#04X})", event_id)),
        },
        StepOutcome::Retired { .. } => TestBlockResult {
            start_line: block.start_line,
            end_line: block.end_line,
            assertion_results: Vec::new(),
            faulted: true,
            fault_message: Some("Run loop exited without HALT or fault".to_string()),
        },
    }
}

/// Evaluates all assertions against the current machine state.
fn evaluate_assertions(state: &CoreState, assertions: &[Assertion]) -> Vec<AssertionResult> {
    assertions
        .iter()
        .map(|assertion| evaluate_assertion(state, assertion))
        .collect()
}

/// Evaluates a single assertion against the current machine state.
fn evaluate_assertion(state: &CoreState, assertion: &Assertion) -> AssertionResult {
    match assertion {
        Assertion::Register {
            register,
            operator,
            expected,
        } => {
            let actual = read_register(state, *register);
            let passed = match operator {
                ComparisonOp::Equal => actual == *expected,
                ComparisonOp::NotEqual => actual != *expected,
            };
            AssertionResult {
                assertion: assertion.clone(),
                passed,
                actual: format!("{:#06X}", actual),
            }
        }
        Assertion::Memory {
            address,
            operator,
            expected,
        } => {
            let actual = state.memory[usize::from(*address)];
            let passed = match operator {
                ComparisonOp::Equal => actual == *expected,
                ComparisonOp::NotEqual => actual != *expected,
            };
            AssertionResult {
                assertion: assertion.clone(),
                passed,
                actual: format!("{:#04X}", actual),
            }
        }
    }
}

/// Reads a register value from machine state.
fn read_register(state: &CoreState, register: Register) -> u16 {
    match register {
        Register::R0 => state.arch.gpr(GeneralRegister::R0),
        Register::R1 => state.arch.gpr(GeneralRegister::R1),
        Register::R2 => state.arch.gpr(GeneralRegister::R2),
        Register::R3 => state.arch.gpr(GeneralRegister::R3),
        Register::R4 => state.arch.gpr(GeneralRegister::R4),
        Register::R5 => state.arch.gpr(GeneralRegister::R5),
        Register::R6 => state.arch.gpr(GeneralRegister::R6),
        Register::R7 => state.arch.gpr(GeneralRegister::R7),
        Register::PC => state.arch.pc(),
    }
}

/// A null MMIO bus that returns 0 on reads and denies all writes.
struct NullMmio;

impl MmioBus for NullMmio {
    fn read16(&mut self, _addr: u16) -> Result<u16, MmioError> {
        Ok(0)
    }

    fn write16(&mut self, _addr: u16, _value: u16) -> Result<MmioWriteResult, MmioError> {
        Ok(MmioWriteResult::DeniedSuppressed)
    }
}

impl fmt::Display for TestBlockResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.passed() {
            write!(
                f,
                "PASS (lines {}-{}): {} assertions",
                self.start_line,
                self.end_line,
                self.assertion_results.len()
            )
        } else if self.faulted {
            write!(
                f,
                "FAIL (lines {}-{}): {}",
                self.start_line,
                self.end_line,
                self.fault_message.as_deref().unwrap_or("unknown fault")
            )
        } else {
            let failures: Vec<_> = self
                .assertion_results
                .iter()
                .filter(|r| !r.passed)
                .collect();
            write!(
                f,
                "FAIL (lines {}-{}): {} assertion(s) failed",
                self.start_line,
                self.end_line,
                failures.len()
            )
        }
    }
}

impl fmt::Display for AssertionResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.passed {
            write!(f, "  PASS: {:?}", self.assertion)
        } else {
            write!(
                f,
                "  FAIL: {:?} (expected, got {})",
                self.assertion, self.actual
            )
        }
    }
}

impl fmt::Display for TestSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} passed, {} failed", self.passed, self.failed)?;
        if self.unexecuted > 0 {
            write!(f, ", {} unexecuted", self.unexecuted)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_format::parse_test_block;

    fn encode_nop() -> Vec<u8> {
        vec![0x00, 0x00]
    }

    fn encode_halt() -> Vec<u8> {
        let op: u16 = 0x0;
        let sub: u16 = 0x2;
        let primary = (op << 12) | (sub << 3);
        vec![(primary >> 8) as u8, (primary & 0xFF) as u8]
    }

    fn encode_add(rd: u8, ra: u8, rb: u8) -> Vec<u8> {
        let op: u16 = 0x4;
        let am: u16 = 0x0;
        let primary = (op << 12)
            | (u16::from(rd & 0x7) << 9)
            | (u16::from(ra & 0x7) << 6)
            | (u16::from(rb & 0x7) << 3)
            | am;
        vec![(primary >> 8) as u8, (primary & 0xFF) as u8]
    }

    fn encode_store_indirect(rd: u8, ra: u8) -> Vec<u8> {
        let op: u16 = 0x3;
        let sub: u16 = 0x0;
        let am: u16 = 0x1;
        let primary =
            (op << 12) | (u16::from(rd & 0x7) << 9) | (u16::from(ra & 0x7) << 6) | (sub << 3) | am;
        vec![(primary >> 8) as u8, (primary & 0xFF) as u8]
    }

    fn create_state_with_gprs(values: &[(u8, u16)]) -> CoreState {
        let mut state = CoreState::with_config(&CoreConfig::default());
        for (reg, val) in values {
            state
                .arch
                .set_gpr(GeneralRegister::from_u3(*reg).unwrap(), *val);
        }
        state
    }

    #[test]
    fn nop_halt_test() {
        let mut state = create_state_with_gprs(&[(0, 0x1234)]);

        let mut binary = Vec::new();
        binary.extend(encode_nop());
        binary.extend(encode_halt());

        load_binary(&mut state, &binary);

        let test_block = parse_test_block("R0 == 0x1234", 1, 3).unwrap();

        let mut mmio = NullMmio;
        let result = run_test_block(&mut state, &CoreConfig::default(), &mut mmio, &test_block);

        assert!(result.passed());
    }

    #[test]
    fn test_fails_on_wrong_value() {
        let mut state = create_state_with_gprs(&[(0, 0x1234)]);

        let mut binary = Vec::new();
        binary.extend(encode_nop());
        binary.extend(encode_halt());

        load_binary(&mut state, &binary);

        let test_block = parse_test_block("R0 == 0x5678", 1, 3).unwrap();

        let mut mmio = NullMmio;
        let result = run_test_block(&mut state, &CoreConfig::default(), &mut mmio, &test_block);

        assert!(!result.passed());
        assert_eq!(result.assertion_results[0].actual, "0x1234");
    }

    #[test]
    fn multiple_assertions_in_block() {
        let mut state = create_state_with_gprs(&[(0, 0x1111), (1, 0x2222)]);

        let mut binary = Vec::new();
        binary.extend(encode_nop());
        binary.extend(encode_halt());

        load_binary(&mut state, &binary);

        let test_block = parse_test_block("R0 == 0x1111\nR1 == 0x2222", 1, 5).unwrap();

        let mut mmio = NullMmio;
        let result = run_test_block(&mut state, &CoreConfig::default(), &mut mmio, &test_block);

        assert!(result.passed());
        assert_eq!(result.assertion_results.len(), 2);
    }

    #[test]
    fn add_modifies_register() {
        let mut state = create_state_with_gprs(&[(0, 0x1000), (1, 0x0200)]);

        let mut binary = Vec::new();
        binary.extend(encode_add(0, 0, 1));
        binary.extend(encode_halt());

        load_binary(&mut state, &binary);

        let test_block = parse_test_block("R0 == 0x1200", 1, 3).unwrap();

        let mut mmio = NullMmio;
        let result = run_test_block(&mut state, &CoreConfig::default(), &mut mmio, &test_block);

        assert!(result.passed());
    }

    #[test]
    fn multiple_halts_multiple_blocks() {
        let mut state = create_state_with_gprs(&[(0, 0x0001), (1, 0x0001)]);

        let mut binary = Vec::new();
        binary.extend(encode_add(0, 0, 1));
        binary.extend(encode_halt());
        binary.extend(encode_add(0, 0, 1));
        binary.extend(encode_halt());

        load_binary(&mut state, &binary);

        let block1 = parse_test_block("R0 == 0x0002", 1, 3).unwrap();
        let block2 = parse_test_block("R0 == 0x0003", 5, 7).unwrap();

        let result = run_tests_with_state(&mut state, &[block1, block2]);

        assert!(result.all_passed());
        assert_eq!(result.block_results.len(), 2);
    }

    #[test]
    fn memory_assertion() {
        let mut state = create_state_with_gprs(&[(0, 0x12FF), (1, 0x4000)]);

        let mut binary = Vec::new();
        binary.extend(encode_store_indirect(0, 1));
        binary.extend(encode_halt());

        load_binary(&mut state, &binary);

        let test_block = parse_test_block("[0x4000] == 0xFF", 1, 5).unwrap();

        let mut mmio = NullMmio;
        let result = run_test_block(&mut state, &CoreConfig::default(), &mut mmio, &test_block);

        assert!(result.passed());
    }

    #[test]
    fn inequality_assertion() {
        let mut state = create_state_with_gprs(&[(0, 0x1234)]);

        let mut binary = Vec::new();
        binary.extend(encode_nop());
        binary.extend(encode_halt());

        load_binary(&mut state, &binary);

        let test_block = parse_test_block("R0 != 0x0000", 1, 3).unwrap();

        let mut mmio = NullMmio;
        let result = run_test_block(&mut state, &CoreConfig::default(), &mut mmio, &test_block);

        assert!(result.passed());
    }

    #[test]
    fn pc_assertion() {
        let mut state = CoreState::with_config(&CoreConfig::default());

        let mut binary = Vec::new();
        binary.extend(encode_nop());
        binary.extend(encode_halt());

        load_binary(&mut state, &binary);

        let test_block = parse_test_block("PC == 0x0002", 1, 3).unwrap();

        let mut mmio = NullMmio;
        let result = run_test_block(&mut state, &CoreConfig::default(), &mut mmio, &test_block);

        assert!(result.passed());
    }

    #[test]
    fn zero_test_blocks() {
        let mut state = CoreState::with_config(&CoreConfig::default());
        let binary = encode_halt();

        load_binary(&mut state, &binary);

        let result = run_tests_with_state(&mut state, &[]);

        assert!(result.all_passed());
        assert!(result.block_results.is_empty());
        assert_eq!(result.unexecuted_blocks, 0);
    }

    #[test]
    fn more_blocks_than_halts() {
        let mut state = create_state_with_gprs(&[(0, 0x0001)]);

        let mut binary = Vec::new();
        binary.extend(encode_nop());
        binary.extend(encode_halt());

        load_binary(&mut state, &binary);

        let block1 = parse_test_block("R0 == 0x0001", 1, 3).unwrap();
        let block2 = parse_test_block("R0 == 0x0002", 5, 7).unwrap();
        let block3 = parse_test_block("R0 == 0x0003", 9, 11).unwrap();

        let result = run_tests_with_state(&mut state, &[block1, block2, block3]);

        assert!(!result.all_passed());
        assert_eq!(result.block_results.len(), 2);
        assert!(result.block_results[0].passed());
        assert!(result.block_results[1].faulted);
        assert_eq!(result.unexecuted_blocks, 1);
    }

    #[test]
    fn summary_counts() {
        let mut state = create_state_with_gprs(&[(0, 0x0001), (1, 0x0001), (2, 0x0001)]);

        let mut binary = Vec::new();
        binary.extend(encode_add(0, 0, 1));
        binary.extend(encode_halt());
        binary.extend(encode_add(0, 0, 2));
        binary.extend(encode_halt());
        binary.extend(encode_add(0, 0, 1));
        binary.extend(encode_halt());

        load_binary(&mut state, &binary);

        let block1 = parse_test_block("R0 == 0x0002", 1, 3).unwrap();
        let block2 = parse_test_block("R0 == 0x9999", 5, 7).unwrap();
        let block3 = parse_test_block("R0 == 0x0004", 9, 11).unwrap();

        let result = run_tests_with_state(&mut state, &[block1, block2, block3]);

        let summary = result.summary();
        assert_eq!(summary.passed, 2);
        assert_eq!(summary.failed, 1);
        assert_eq!(summary.unexecuted, 0);
        assert_eq!(summary.total, 3);
    }

    fn run_tests_with_state(
        state: &mut CoreState,
        test_blocks: &[ParsedTestBlock],
    ) -> TestRunResult {
        let config = CoreConfig::default();
        let mut mmio = NullMmio;
        let mut block_results = Vec::new();

        for block in test_blocks {
            let result = run_test_block(state, &config, &mut mmio, block);
            block_results.push(result);

            if matches!(state.run_state, RunState::FaultLatched(_)) {
                let remaining = test_blocks.len() - block_results.len();
                return TestRunResult {
                    block_results,
                    unexecuted_blocks: remaining,
                };
            }
        }

        TestRunResult {
            block_results,
            unexecuted_blocks: 0,
        }
    }
}
