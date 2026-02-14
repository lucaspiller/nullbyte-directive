//! Phase 12 completion suite: integration, property, and fuzz-style coverage.

#![allow(
    clippy::pedantic,
    clippy::nursery,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::too_many_lines
)]

use std::panic::{self, AssertUnwindSafe};

use emulator_core::{
    replay_from_snapshot, validate_fetch_access, validate_mmio_alignment, validate_mmio_width,
    validate_word_alignment, write_u16_be, CoreConfig, CoreSnapshot, CoreState, Decoder, FaultCode,
    GeneralRegister, MmioBus, MmioError, MmioWriteResult, ReplayEventStream, RunBoundary, RunState,
    SnapshotVersion, StepOutcome, VEC_FAULT,
};
use proptest::prelude::*;
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

#[derive(Default)]
struct OrderingMmio {
    writes: Vec<(u16, u16)>,
}

impl MmioBus for OrderingMmio {
    fn read16(&mut self, _addr: u16) -> Result<u16, MmioError> {
        Ok(0)
    }

    fn write16(&mut self, addr: u16, value: u16) -> Result<MmioWriteResult, MmioError> {
        self.writes.push((addr, value));
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

#[test]
fn integration_ewait_and_budget_interaction_on_empty_queue() {
    let mut state = CoreState::default();
    state.arch.set_tick(639);
    load_word(&mut state, 0x0000, encode(0xA, 0, 0, 0x0, 0)); // EWAIT

    let config = CoreConfig::default();
    let mut mmio = NoopMmio;

    let first = emulator_core::step_one(&mut state, &mut mmio, &config);
    assert_eq!(first, StepOutcome::HaltedForTick);
    assert_eq!(state.arch.pc(), 0x0000);
    assert_eq!(state.arch.tick(), 640);

    let second = emulator_core::step_one(&mut state, &mut mmio, &config);
    assert_eq!(
        second,
        StepOutcome::Fault {
            cause: FaultCode::BudgetOverrun,
        }
    );
}

#[test]
fn integration_eret_fault_outside_handler_context() {
    let mut state = CoreState::default();
    load_word(&mut state, 0x0000, encode(0xA, 0, 0, 0x2, 0)); // ERET
    let _ = write_u16_be(state.memory.as_mut(), VEC_FAULT, 0x0008);

    let mut mmio = NoopMmio;
    let config = CoreConfig::default();

    let outcome = emulator_core::step_one(&mut state, &mut mmio, &config);
    assert_eq!(
        outcome,
        StepOutcome::Fault {
            cause: FaultCode::HandlerContextViolation,
        }
    );
}

#[test]
fn integration_double_fault_and_invalid_vec_fault_halt() {
    let config = CoreConfig::default();
    let mut mmio = NoopMmio;

    let mut double_fault = CoreState {
        run_state: RunState::HandlerContext,
        ..CoreState::default()
    };
    load_word(&mut double_fault, 0x0000, 0xB000); // illegal encoding while already handling

    let outcome = emulator_core::step_one(&mut double_fault, &mut mmio, &config);
    assert_eq!(
        outcome,
        StepOutcome::Fault {
            cause: FaultCode::DoubleFault,
        }
    );
    assert_eq!(
        double_fault.run_state,
        RunState::FaultLatched(FaultCode::DoubleFault)
    );

    let mut invalid_vector = CoreState::default();
    load_word(&mut invalid_vector, 0x0000, encode(0xA, 0, 0, 0x2, 0)); // ERET outside handler
    let _ = write_u16_be(invalid_vector.memory.as_mut(), VEC_FAULT, 0x0000);

    let outcome = emulator_core::step_one(&mut invalid_vector, &mut mmio, &config);
    assert_eq!(
        outcome,
        StepOutcome::Fault {
            cause: FaultCode::InvalidFaultVector,
        }
    );
    assert_eq!(
        invalid_vector.run_state,
        RunState::FaultLatched(FaultCode::InvalidFaultVector)
    );
}

#[test]
fn integration_non_zero_unused_register_field_fault_behavior() {
    let mut state = CoreState::default();
    load_word(&mut state, 0x0000, encode(0x0, 0x1, 0x0, 0x0, 0x0)); // NOP with RD != 0

    let mut mmio = NoopMmio;
    let config = CoreConfig::default();

    let outcome = emulator_core::step_one(&mut state, &mut mmio, &config);
    assert_eq!(
        outcome,
        StepOutcome::Fault {
            cause: FaultCode::IllegalEncoding,
        }
    );
}

#[test]
fn integration_div_mod_divide_by_zero_destination_behavior() {
    let mut state = CoreState::default();
    state.arch.set_gpr(GeneralRegister::R1, 123);
    state.arch.set_gpr(GeneralRegister::R2, 0);

    load_word(&mut state, 0x0000, encode(0x5, 0, 1, 0x2, 0)); // DIV
    load_word(&mut state, 0x0002, encode(0x5, 0, 1, 0x3, 0)); // MOD

    let config = CoreConfig::default();
    let mut mmio = NoopMmio;

    let div_outcome = emulator_core::step_one(&mut state, &mut mmio, &config);
    assert!(matches!(div_outcome, StepOutcome::Retired { .. }));
    assert_eq!(state.arch.gpr(GeneralRegister::R0), 0);

    state.arch.set_gpr(GeneralRegister::R1, 456);
    state.arch.set_gpr(GeneralRegister::R3, 0);

    let mod_outcome = emulator_core::step_one(&mut state, &mut mmio, &config);
    assert!(matches!(mod_outcome, StepOutcome::Retired { .. }));
    assert_eq!(state.arch.gpr(GeneralRegister::R0), 0);
}

#[test]
fn integration_mmio_strong_ordering_and_sync_visibility() {
    let mut state = CoreState::default();
    state.arch.set_gpr(GeneralRegister::R1, 0xE010);

    load_word(&mut state, 0x0000, encode(0x8, 0, 1, 0x1, 0)); // OUT
    load_word(&mut state, 0x0002, encode(0x0, 0, 0, 0x1, 0)); // SYNC

    let config = CoreConfig::default();
    let mut mmio = OrderingMmio::default();

    let first = emulator_core::step_one(&mut state, &mut mmio, &config);
    assert!(matches!(first, StepOutcome::Retired { .. }));
    assert_eq!(mmio.writes, vec![(0xE010, 0xE010)]);

    let second = emulator_core::step_one(&mut state, &mut mmio, &config);
    assert!(matches!(second, StepOutcome::Retired { .. }));
    assert_eq!(mmio.writes, vec![(0xE010, 0xE010)]);
}

#[test]
fn integration_halt_remainder_of_tick_and_next_tick_resume() {
    let mut state = CoreState::default();
    load_word(&mut state, 0x0000, encode(0x0, 0, 0, 0x2, 0)); // HALT
    load_word(&mut state, 0x0002, encode(0x0, 0, 0, 0x0, 0)); // NOP

    let config = CoreConfig::default();
    let mut mmio = NoopMmio;

    let first = emulator_core::step_one(&mut state, &mut mmio, &config);
    assert_eq!(first, StepOutcome::HaltedForTick);
    assert_eq!(state.arch.pc(), 0x0002);
    assert_eq!(state.run_state, RunState::HaltedForTick);

    let second = emulator_core::step_one(&mut state, &mut mmio, &config);
    assert!(matches!(second, StepOutcome::Retired { .. }));
    assert_eq!(state.arch.pc(), 0x0004);
    assert_eq!(state.run_state, RunState::Running);
}

proptest! {
    #[test]
    fn property_decode_robustness_over_arbitrary_words(word in any::<u16>()) {
        let decoded = Decoder::decode(word);
        if let Some(reason) = decoded.fault() {
            prop_assert_eq!(reason.code(), FaultCode::IllegalEncoding);
        }
    }

    #[test]
    fn property_memory_boundary_and_alignment_invariants(addr in any::<u16>(), width in any::<u8>()) {
        let fetch = validate_fetch_access(addr);
        if addr <= 0xDFFF {
            prop_assert!(fetch.is_ok());
        } else {
            prop_assert_eq!(fetch, Err(FaultCode::NonExecutableFetch));
        }

        let align = validate_word_alignment(addr);
        if addr % 2 == 0 {
            prop_assert!(align.is_ok());
        } else {
            prop_assert_eq!(align, Err(FaultCode::UnalignedDataAccess));
        }

        let mmio_align = validate_mmio_alignment(addr);
        if addr % 2 == 0 {
            prop_assert!(mmio_align.is_ok());
        } else {
            prop_assert_eq!(mmio_align, Err(FaultCode::MmioAlignmentViolation));
        }

        let mmio_width = validate_mmio_width(width);
        if width == 2 {
            prop_assert!(mmio_width.is_ok());
        } else {
            prop_assert_eq!(mmio_width, Err(FaultCode::MmioWidthViolation));
        }
    }

    #[test]
    fn property_snapshot_round_trip_invariants(
        pc in any::<u16>(),
        sp in any::<u16>(),
        tick in any::<u16>(),
        flags in any::<u16>(),
        cause in any::<u16>(),
        cap in any::<u16>(),
        evp in any::<u16>(),
        r0 in any::<u16>(),
        events in prop::collection::vec(any::<u8>(), 0..=4)
    ) {
        let mut state = CoreState::default();
        state.arch.set_pc(pc);
        state.arch.set_sp(sp);
        state.arch.set_tick(tick);
        state.arch.set_flags(flags);
        state.arch.set_cause(cause);
        state.arch.set_cap_core_owned(cap);
        state.arch.set_evp_core_owned(evp);
        state.arch.set_gpr(GeneralRegister::R0, r0);

        for event in events {
            let _ = state.event_queue.enqueue(event);
        }

        let snapshot = CoreSnapshot::from_core_state(SnapshotVersion::V1, &state);
        let restored = snapshot.try_into_core_state().expect("snapshot should round-trip");

        prop_assert_eq!(restored.arch.pc(), state.arch.pc());
        prop_assert_eq!(restored.arch.sp(), state.arch.sp());
        prop_assert_eq!(restored.arch.tick(), state.arch.tick());
        prop_assert_eq!(restored.arch.flags(), state.arch.flags());
        prop_assert_eq!(restored.arch.cause(), state.arch.cause());
        prop_assert_eq!(restored.arch.cap(), state.arch.cap());
        prop_assert_eq!(restored.arch.evp(), state.arch.evp());
        prop_assert_eq!(restored.arch.gpr(GeneralRegister::R0), state.arch.gpr(GeneralRegister::R0));
        prop_assert_eq!(restored.event_queue, state.event_queue);
        prop_assert_eq!(restored.run_state, state.run_state);
    }
}

#[test]
fn fuzz_harness_decode_execute_memory_interfaces_are_panic_free() {
    let config = CoreConfig::default();
    let mut seed: u64 = 0xA5A5_1337_55AA_F00D;

    for _ in 0..4096 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let word = (seed >> 16) as u16;
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let addr = (seed >> 16) as u16;

        let decode_result = panic::catch_unwind(AssertUnwindSafe(|| {
            let _ = Decoder::decode(word);
        }));
        assert!(
            decode_result.is_ok(),
            "decode panicked for word {word:#06X}"
        );

        let execute_result = panic::catch_unwind(AssertUnwindSafe(|| {
            let mut state = CoreState::default();
            load_word(&mut state, 0x0000, word);
            state.arch.set_pc(0x0000);
            let mut mmio = NoopMmio;
            let _ = emulator_core::step_one(&mut state, &mut mmio, &config);
        }));
        assert!(
            execute_result.is_ok(),
            "execute panicked for word {word:#06X}"
        );

        let memory_result = panic::catch_unwind(AssertUnwindSafe(|| {
            let _ = validate_fetch_access(addr);
            let _ = validate_word_alignment(addr);
            let _ = validate_mmio_alignment(addr);
            let _ = validate_mmio_width((word & 0x00FF) as u8);
        }));
        assert!(
            memory_result.is_ok(),
            "memory validation panicked for addr {addr:#06X}"
        );
    }
}

#[test]
fn deterministic_replay_is_stable_for_identical_inputs() {
    let mut initial = CoreState::default();
    load_word(&mut initial, 0x0000, encode(0x0, 0, 0, 0x0, 0)); // NOP
    load_word(&mut initial, 0x0002, encode(0x0, 0, 0, 0x2, 0)); // HALT

    let snapshot = CoreSnapshot::from_core_state(SnapshotVersion::V1, &initial);
    let mut stream = ReplayEventStream::new();
    stream.add_event(0x11);
    stream.add_event(0x22);

    let config = CoreConfig::default();
    let mut mmio_a = NoopMmio;
    let mut mmio_b = NoopMmio;

    let run_a = replay_from_snapshot(
        snapshot.clone(),
        &stream,
        &mut mmio_a,
        &config,
        RunBoundary::Halted,
    )
    .expect("first replay run should succeed");

    let run_b = replay_from_snapshot(snapshot, &stream, &mut mmio_b, &config, RunBoundary::Halted)
        .expect("second replay run should succeed");

    assert_eq!(run_a.steps, run_b.steps);
    assert_eq!(run_a.final_outcome, run_b.final_outcome);
    assert_eq!(run_a.final_state, run_b.final_state);
}
