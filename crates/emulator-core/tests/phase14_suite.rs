//! Phase 14 hardening and release-readiness verification suite.

#![allow(
    clippy::pedantic,
    clippy::nursery,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::too_many_lines
)]

use std::panic::{self, AssertUnwindSafe};

use emulator_core::{
    replay_from_snapshot, CoreConfig, CoreSnapshot, CoreState, MmioBus, MmioError, MmioWriteResult,
    ReplayEventStream, ReplayResult, RunBoundary, SnapshotVersion, StepOutcome,
};
use proptest as _;
use rstest as _;
#[cfg(feature = "serde")]
use serde as _;
use thiserror as _;

#[derive(Default)]
struct NoopMmio;

impl MmioBus for NoopMmio {
    fn read16(&mut self, _addr: u16) -> Result<u16, MmioError> {
        Ok(0)
    }

    fn write16(&mut self, _addr: u16, _value: u16) -> Result<MmioWriteResult, MmioError> {
        Ok(MmioWriteResult::Applied)
    }
}

const fn encode(op: u8, rd: u8, ra: u8, sub: u8, am: u8) -> u16 {
    ((op as u16) << 12) | ((rd as u16) << 9) | ((ra as u16) << 6) | ((sub as u16) << 3) | am as u16
}

fn load_word(state: &mut CoreState, addr: u16, word: u16) {
    let [hi, lo] = word.to_be_bytes();
    state.memory[usize::from(addr)] = hi;
    state.memory[usize::from(addr.wrapping_add(1))] = lo;
}

fn replay_bytes(result: &ReplayResult) -> Vec<u8> {
    let mut bytes = Vec::new();

    bytes.extend_from_slice(&result.steps.to_le_bytes());
    match result.final_outcome {
        StepOutcome::Retired { cycles } => {
            bytes.push(0x10);
            bytes.extend_from_slice(&cycles.to_le_bytes());
        }
        StepOutcome::HaltedForTick => bytes.push(0x11),
        StepOutcome::TrapDispatch { cause } => {
            bytes.push(0x12);
            bytes.extend_from_slice(&cause.to_le_bytes());
        }
        StepOutcome::EventDispatch { event_id } => {
            bytes.push(0x13);
            bytes.push(event_id);
        }
        StepOutcome::Fault { cause } => {
            bytes.push(0x14);
            bytes.push(cause.as_u8());
        }
    }

    bytes.extend_from_slice(&result.final_state.arch.pc().to_le_bytes());
    bytes.extend_from_slice(&result.final_state.arch.sp().to_le_bytes());
    bytes.extend_from_slice(&result.final_state.arch.tick().to_le_bytes());
    bytes.extend_from_slice(&result.final_state.arch.flags().to_le_bytes());
    bytes.extend_from_slice(&result.final_state.arch.cause().to_le_bytes());
    bytes.extend_from_slice(&result.final_state.arch.cap().to_le_bytes());
    bytes.extend_from_slice(&result.final_state.arch.evp().to_le_bytes());
    bytes.extend_from_slice(&result.final_state.memory);

    bytes
}

#[test]
fn fuzz_stress_budget_no_guest_input_panics_host() {
    let config = CoreConfig::default();
    let mut seed = 0x9E37_79B9_7F4A_7C15_u64;

    for _ in 0..2048 {
        let run = panic::catch_unwind(AssertUnwindSafe(|| {
            let mut state = CoreState::default();

            for addr in (0..256_u16).step_by(2) {
                seed = seed
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1_442_695_040_888_963_407);
                let word = (seed >> 16) as u16;
                load_word(&mut state, addr, word);
            }

            let mut mmio = NoopMmio;
            for _ in 0..512 {
                let _ = emulator_core::step_one(&mut state, &mut mmio, &config);
            }
        }));

        assert!(
            run.is_ok(),
            "step pipeline panicked under fuzz stress campaign"
        );
    }
}

#[test]
fn repeated_runs_produce_byte_identical_results() {
    let mut initial = CoreState::default();
    load_word(&mut initial, 0x0000, encode(0x0, 0, 0, 0x0, 0)); // NOP
    load_word(&mut initial, 0x0002, encode(0x0, 0, 0, 0x2, 0)); // HALT

    let snapshot = CoreSnapshot::from_core_state(SnapshotVersion::V1, &initial);
    let mut events = ReplayEventStream::new();
    events.add_event(0x11);
    events.add_event(0x22);

    let config = CoreConfig::default();
    let mut reference: Option<Vec<u8>> = None;

    for _ in 0..16 {
        let mut mmio = NoopMmio;
        let replay = replay_from_snapshot(
            snapshot.clone(),
            &events,
            &mut mmio,
            &config,
            RunBoundary::Halted,
        )
        .expect("replay should succeed");

        let bytes = replay_bytes(&replay);
        if let Some(reference_bytes) = &reference {
            assert_eq!(bytes, *reference_bytes);
        } else {
            reference = Some(bytes);
        }
    }
}
