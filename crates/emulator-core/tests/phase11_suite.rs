//! Phase 11 conformance-focused unit and integration coverage.
//!
//! These tests cover opcode semantics, FLAGS/addressing/timing behaviors, and
//! integration scenarios required by the TODO Phase 11 backlog.

#![allow(
    clippy::pedantic,
    clippy::nursery,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::similar_names,
    clippy::too_many_lines
)]

use emulator_core::execute::compute_effective_address;
use emulator_core::{
    cycle_cost, write_u16_be, AddressingMode, CoreConfig, CoreProfile, CoreState, CycleCostKind,
    DecodedInstruction, Decoder, DiagCoreFields, EventEnqueueError, FaultCode, GeneralRegister,
    MmioBus, MmioError, MmioWriteResult, OpcodeEncoding, RunState, StepOutcome,
    OPCODE_ENCODING_TABLE, VEC_EVENT, VEC_FAULT, VEC_TRAP,
};
use proptest as _;
use rstest as _;
#[cfg(feature = "serde")]
use serde as _;
use thiserror as _;

#[derive(Default)]
struct StubMmio {
    read_value: u16,
    fail_reads: bool,
    deny_writes: bool,
    fail_writes: bool,
}

impl MmioBus for StubMmio {
    fn read16(&mut self, _addr: u16) -> Result<u16, MmioError> {
        if self.fail_reads {
            Err(MmioError::ReadFailed)
        } else {
            Ok(self.read_value)
        }
    }

    fn write16(&mut self, _addr: u16, _value: u16) -> Result<MmioWriteResult, MmioError> {
        if self.fail_writes {
            Err(MmioError::WriteFailed)
        } else if self.deny_writes {
            Ok(MmioWriteResult::DeniedSuppressed)
        } else {
            Ok(MmioWriteResult::Applied)
        }
    }
}

const fn encode(op: u8, rd: u8, ra: u8, sub: u8, am: u8) -> u16 {
    (u16::from_be_bytes([0, 0]))
        | ((op as u16) << 12)
        | ((rd as u16) << 9)
        | ((ra as u16) << 6)
        | ((sub as u16) << 3)
        | (am as u16)
}

fn seed_state(state: &mut CoreState) {
    state.arch.set_pc(0x0000);
    state.arch.set_sp(0xD000);
    state.arch.set_gpr(GeneralRegister::R0, 0x4000);
    state.arch.set_gpr(GeneralRegister::R1, 0xE000);
    state.arch.set_gpr(GeneralRegister::R2, 0x0000);
    state.arch.set_gpr(GeneralRegister::R3, 0x0000);
    state.arch.set_gpr(GeneralRegister::R4, 0x0004);
    state.arch.set_gpr(GeneralRegister::R5, 0x0005);
    state.arch.set_gpr(GeneralRegister::R6, 0x0006);
    state.arch.set_gpr(GeneralRegister::R7, 0x0007);

    let _ = write_u16_be(state.memory.as_mut(), VEC_TRAP, 0x0020);
    let _ = write_u16_be(state.memory.as_mut(), VEC_EVENT, 0x0022);
    let _ = write_u16_be(state.memory.as_mut(), VEC_FAULT, 0x0024);
    let _ = write_u16_be(state.memory.as_mut(), 0x4000, 0xA55A);
}

fn load_primary(state: &mut CoreState, word: u16) {
    let [hi, lo] = word.to_be_bytes();
    state.memory[0] = hi;
    state.memory[1] = lo;
}

fn decode_word(word: u16) -> DecodedInstruction {
    Decoder::decode(word)
        .instruction()
        .expect("word must decode")
}

#[test]
fn unit_opcode_semantics_table_covers_all_encodings() {
    let config = CoreConfig::default();

    for (op, sub, encoding) in OPCODE_ENCODING_TABLE {
        let mut state = CoreState::default();
        seed_state(&mut state);
        let mut mmio = StubMmio {
            read_value: 0x1234,
            ..StubMmio::default()
        };

        let word = encode(*op, 0, 0, *sub, 0);
        load_primary(&mut state, word);

        let outcome = emulator_core::step_one(&mut state, &mut mmio, &config);

        match encoding {
            OpcodeEncoding::Halt => {
                assert!(matches!(outcome, StepOutcome::HaltedForTick));
            }
            OpcodeEncoding::Trap | OpcodeEncoding::Swi => {
                assert!(matches!(outcome, StepOutcome::TrapDispatch { .. }));
            }
            OpcodeEncoding::Eret => {
                assert!(matches!(
                    outcome,
                    StepOutcome::Fault {
                        cause: FaultCode::HandlerContextViolation
                    }
                ));
            }
            _ => {
                assert!(matches!(outcome, StepOutcome::Retired { .. }));
            }
        }
    }
}

#[test]
fn unit_div_mod_zero_edge_cases_return_zero_without_fault() {
    let mut state = CoreState::default();
    seed_state(&mut state);
    let config = CoreConfig::default();
    let mut mmio = StubMmio::default();

    state.arch.set_gpr(GeneralRegister::R1, 99);
    state.arch.set_gpr(GeneralRegister::R2, 0);
    load_primary(&mut state, encode(0x5, 0, 1, 0x2, 0));

    let outcome = emulator_core::step_one(&mut state, &mut mmio, &config);
    assert!(matches!(outcome, StepOutcome::Retired { .. }));
    assert_eq!(state.arch.gpr(GeneralRegister::R0), 0);

    state.arch.set_pc(0);
    state.arch.set_gpr(GeneralRegister::R1, 77);
    state.arch.set_gpr(GeneralRegister::R3, 0);
    load_primary(&mut state, encode(0x5, 0, 1, 0x3, 0));

    let outcome = emulator_core::step_one(&mut state, &mut mmio, &config);
    assert!(matches!(outcome, StepOutcome::Retired { .. }));
    assert_eq!(state.arch.gpr(GeneralRegister::R0), 0);
}

#[test]
fn unit_flags_transitions_for_instruction_classes() {
    let config = CoreConfig::default();

    let mut state = CoreState::default();
    seed_state(&mut state);
    state.arch.set_flags(0x3F);
    let mut mmio = StubMmio::default();
    load_primary(&mut state, encode(0x0, 0, 0, 0x0, 0));
    let _ = emulator_core::step_one(&mut state, &mut mmio, &config);
    assert_eq!(state.arch.flags(), 0x3F);

    let mut state = CoreState::default();
    seed_state(&mut state);
    state.arch.set_flags(0x3F);
    load_primary(&mut state, encode(0x1, 0, 0, 0x0, 5));
    let _ = emulator_core::step_one(&mut state, &mut mmio, &config);
    assert_eq!(state.arch.flags() & 0x0C, 0);

    let mut state = CoreState::default();
    seed_state(&mut state);
    state.arch.set_gpr(GeneralRegister::R0, 0x8000);
    load_primary(&mut state, encode(0x4, 0, 0, 0x0, 0));
    let _ = emulator_core::step_one(&mut state, &mut mmio, &config);
    assert!(state.arch.flag_is_set(0x01)); // Z
    assert!(state.arch.flag_is_set(0x04));

    let mut state = CoreState::default();
    seed_state(&mut state);
    state.arch.set_flags(0x15);
    load_primary(&mut state, encode(0x3, 0, 0, 0x0, 0));
    let _ = emulator_core::step_one(&mut state, &mut mmio, &config);
    assert_eq!(state.arch.flags(), 0x15);

    let mut state = CoreState::default();
    seed_state(&mut state);
    state.arch.set_gpr(GeneralRegister::R0, 0x0001);
    state.arch.set_gpr(GeneralRegister::R7, 0x0002);
    load_primary(&mut state, encode(0x4, 0, 0, 0x7, 0));
    let _ = emulator_core::step_one(&mut state, &mut mmio, &config);
    assert!(state.arch.flag_is_set(0x02));
}

#[test]
fn unit_addressing_mode_ea_calculation_and_fault_cases() {
    let mut state = CoreState::default();
    seed_state(&mut state);

    let direct = DecodedInstruction {
        encoding: OpcodeEncoding::Load,
        rd: Some(emulator_core::RegisterField::R0),
        ra: Some(emulator_core::RegisterField::R4),
        rb: Some(emulator_core::RegisterField::R0),
        addressing_mode: Some(AddressingMode::DirectRegister),
        immediate_value: Some(0),
    };
    assert_eq!(compute_effective_address(&direct, &state), Some(0x0004));

    let sx = DecodedInstruction {
        addressing_mode: Some(AddressingMode::SignExtendedDisplacement),
        immediate_value: Some(0x3F),
        ..direct
    };
    assert_eq!(compute_effective_address(&sx, &state), Some(0x0003));

    let zx = DecodedInstruction {
        addressing_mode: Some(AddressingMode::ZeroExtendedDisplacement),
        immediate_value: Some(0x3F),
        ..direct
    };
    assert_eq!(compute_effective_address(&zx, &state), Some(0x0043));

    let imm = DecodedInstruction {
        addressing_mode: Some(AddressingMode::Immediate),
        immediate_value: Some(0x2222),
        ..direct
    };
    assert_eq!(compute_effective_address(&imm, &state), Some(0x2222));

    assert!(Decoder::decode(encode(0x0, 0, 0, 0x0, 6)).fault().is_some());
    assert!(Decoder::decode(encode(0x0, 0, 0, 0x0, 7)).fault().is_some());
}

#[test]
fn unit_cycle_accounting_by_opcode_and_boundary_behavior() {
    let instructions = [
        (encode(0x0, 0, 0, 0x0, 0), CycleCostKind::Nop),
        (encode(0x2, 0, 0, 0x0, 0), CycleCostKind::Load),
        (encode(0x5, 0, 0, 0x2, 0), CycleCostKind::Div),
        (encode(0x8, 0, 1, 0x1, 0), CycleCostKind::MmioOut),
    ];

    for (word, kind) in instructions {
        let mut state = CoreState::default();
        seed_state(&mut state);
        let mut mmio = StubMmio::default();
        let config = CoreConfig::default();

        load_primary(&mut state, word);
        let before = state.arch.tick();
        let _ = emulator_core::step_one(&mut state, &mut mmio, &config);

        let expected = cycle_cost(kind).expect("cycle kind must exist");
        assert_eq!(state.arch.tick(), before.wrapping_add(expected));
    }

    let mut state = CoreState::default();
    seed_state(&mut state);
    state.arch.set_tick(639);
    let mut mmio = StubMmio::default();
    let config = CoreConfig::default();
    load_primary(&mut state, encode(0x2, 0, 0, 0x0, 0));

    let outcome = emulator_core::step_one(&mut state, &mut mmio, &config);
    assert!(matches!(outcome, StepOutcome::HaltedForTick));
}

#[test]
fn conformance_vectors_are_table_driven() {
    struct Vector {
        id: &'static str,
        word: u16,
        expected: StepOutcome,
    }

    let vectors = [
        Vector {
            id: "illegal_reserved_primary_opcode",
            word: 0xB000,
            expected: StepOutcome::Fault {
                cause: FaultCode::IllegalEncoding,
            },
        },
        Vector {
            id: "illegal_reserved_addressing_mode",
            word: encode(0x0, 0, 0, 0x0, 6),
            expected: StepOutcome::Fault {
                cause: FaultCode::IllegalEncoding,
            },
        },
        Vector {
            id: "eret_outside_handler_context",
            word: encode(0xA, 0, 0, 0x2, 0),
            expected: StepOutcome::Fault {
                cause: FaultCode::HandlerContextViolation,
            },
        },
    ];

    for vector in vectors {
        let mut state = CoreState::default();
        seed_state(&mut state);
        let mut mmio = StubMmio::default();
        let config = CoreConfig::default();

        load_primary(&mut state, vector.word);
        let outcome = emulator_core::step_one(&mut state, &mut mmio, &config);

        assert_eq!(outcome, vector.expected, "vector {} failed", vector.id);
    }
}

#[test]
fn integration_tick_budget_overrun_semantics() {
    let mut state = CoreState {
        run_state: RunState::HaltedForTick,
        ..CoreState::default()
    };
    state.arch.set_tick(640);

    let mut mmio = StubMmio::default();
    let config = CoreConfig::default();

    let outcome = emulator_core::step_one(&mut state, &mut mmio, &config);
    assert_eq!(
        outcome,
        StepOutcome::Fault {
            cause: FaultCode::BudgetOverrun,
        }
    );
}

#[test]
fn integration_event_overflow_and_deterministic_ordering() {
    let mut queue = emulator_core::EventQueueSnapshot::default();
    queue
        .enqueue(0x10)
        .expect("queue should accept first event");
    queue
        .enqueue(0x20)
        .expect("queue should accept second event");
    queue
        .enqueue(0x30)
        .expect("queue should accept third event");
    queue
        .enqueue(0x40)
        .expect("queue should accept fourth event");

    let overflow = queue.enqueue(0x50).expect_err("queue overflow expected");
    assert_eq!(overflow, EventEnqueueError::QueueFull);
    assert_eq!(overflow.fault_code(), FaultCode::EventQueueOverflow);

    assert_eq!(queue.dequeue(), Some(0x10));
    assert_eq!(queue.dequeue(), Some(0x20));
    assert_eq!(queue.dequeue(), Some(0x30));
    assert_eq!(queue.dequeue(), Some(0x40));
    assert_eq!(queue.dequeue(), None);
}

#[test]
fn integration_mmio_authorization_deny_error_propagation() {
    let config = CoreConfig::default();

    let mut denied_state = CoreState::default();
    seed_state(&mut denied_state);
    load_primary(&mut denied_state, encode(0x8, 0, 1, 0x1, 0));

    let mut denied_mmio = StubMmio {
        deny_writes: true,
        ..StubMmio::default()
    };
    let denied_outcome = emulator_core::step_one(&mut denied_state, &mut denied_mmio, &config);
    assert!(matches!(denied_outcome, StepOutcome::Retired { .. }));
    assert_eq!(denied_state.mmio_denied_write_count, 1);

    let mut errored_state = CoreState::default();
    seed_state(&mut errored_state);
    load_primary(&mut errored_state, encode(0x8, 0, 1, 0x1, 0));

    let mut errored_mmio = StubMmio {
        fail_writes: true,
        ..StubMmio::default()
    };
    let errored_outcome = emulator_core::step_one(&mut errored_state, &mut errored_mmio, &config);
    assert!(matches!(errored_outcome, StepOutcome::Retired { .. }));
    assert_eq!(errored_state.mmio_denied_write_count, 1);
}

#[test]
fn integration_reset_defaults_and_first_fetch_from_boot_pc() {
    let mut state = CoreState::default();
    seed_state(&mut state);
    state.arch.set_pc(0x0400);
    load_primary(&mut state, encode(0x0, 0, 0, 0x2, 0));

    state.reset_canonical();

    let mut mmio = StubMmio::default();
    let config = CoreConfig::default();
    let outcome = emulator_core::step_one(&mut state, &mut mmio, &config);

    assert!(matches!(outcome, StepOutcome::HaltedForTick));
    assert_eq!(state.arch.pc(), 0x0002);
}

#[test]
fn integration_precise_fault_no_partial_commit_invariants() {
    let mut state = CoreState::default();
    seed_state(&mut state);
    state.arch.set_pc(0x0000);
    state.arch.set_tick(10);
    state.arch.set_gpr(GeneralRegister::R0, 0xBEEF);
    load_primary(&mut state, 0xB000);

    let mut mmio = StubMmio::default();
    let config = CoreConfig::default();
    let outcome = emulator_core::step_one(&mut state, &mut mmio, &config);

    assert_eq!(
        outcome,
        StepOutcome::Fault {
            cause: FaultCode::IllegalEncoding,
        }
    );
    assert_eq!(state.arch.pc(), 0x0000);
    assert_eq!(state.arch.tick(), 10);
    assert_eq!(state.arch.gpr(GeneralRegister::R0), 0xBEEF);
}

#[test]
fn integration_diag_latching_counter_behavior() {
    let mut state = CoreState::default();
    seed_state(&mut state);
    load_primary(&mut state, 0xB000);

    let mut mmio = StubMmio::default();
    let config = CoreConfig::default();

    let outcome = emulator_core::step_one(&mut state, &mut mmio, &config);
    let mut diag = DiagCoreFields::new();

    if let StepOutcome::Fault { cause } = outcome {
        diag.record_fault(cause, state.arch.pc(), u32::from(state.arch.tick()));
    }

    assert_eq!(diag.last_fault_code, Some(FaultCode::IllegalEncoding));
    assert_eq!(diag.last_fault_pc, 0x0000);
    assert_eq!(diag.fault_count_decode, 1);

    for _ in 0..0x20_000 {
        diag.record_fault(FaultCode::IllegalEncoding, 0x0000, 0);
    }
    assert_eq!(diag.fault_count_decode, u16::MAX);
}

#[test]
fn integration_capability_gated_opcode_faults_for_restricted_profile() {
    let config = CoreConfig {
        profile: CoreProfile::Restricted,
        ..CoreConfig::default()
    };

    let cases = [
        ("atomic_mmio", encode(0x9, 0, 1, 0x0, 0)),
        ("fixed_point_helper", encode(0x5, 0, 1, 0x1, 0)),
        ("event_queue_op", encode(0xA, 0, 0, 0x0, 0)),
    ];

    for (label, word) in cases {
        let mut state = CoreState::with_config(&config);
        seed_state(&mut state);
        load_primary(&mut state, word);

        let mut mmio = StubMmio::default();
        let outcome = emulator_core::step_one(&mut state, &mut mmio, &config);
        assert_eq!(
            outcome,
            StepOutcome::Fault {
                cause: FaultCode::CapabilityViolation,
            },
            "restricted profile must fault for {label}",
        );
    }
}

#[test]
fn integration_capability_gated_opcodes_retire_in_authority_profile() {
    let config = CoreConfig::default();
    let words = [
        encode(0x9, 0, 1, 0x0, 0),
        encode(0x5, 0, 1, 0x1, 0),
        encode(0xA, 0, 0, 0x0, 0),
    ];

    for word in words {
        let mut state = CoreState::with_config(&config);
        seed_state(&mut state);
        load_primary(&mut state, word);

        let mut mmio = StubMmio::default();
        let outcome = emulator_core::step_one(&mut state, &mut mmio, &config);
        assert!(
            matches!(outcome, StepOutcome::Retired { .. }),
            "authority profile should retire {word:#06X}",
        );
    }
}

#[test]
fn conformance_decoder_roundtrip_for_known_opcode_words() {
    for (op, sub, expected_encoding) in OPCODE_ENCODING_TABLE {
        let word = encode(*op, 0, 0, *sub, 0);
        let decoded = decode_word(word);
        assert_eq!(decoded.encoding, *expected_encoding);
    }
}
