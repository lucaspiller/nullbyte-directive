//! FR-10 reset and boot semantics integration coverage.

use emulator_core::{
    CoreConfig, CoreProfile, CoreState, EventQueueSnapshot, FaultCode, GeneralRegister, RunState,
    CAP_AUTHORITY_DEFAULT_MASK, CAP_RESTRICTED_DEFAULT_MASK, EVENT_QUEUE_CAPACITY,
};
use proptest as _;
use rstest as _;
use serde as _;
use thiserror as _;

#[test]
fn canonical_reset_restores_boot_entry_and_arch_defaults() {
    let mut state = CoreState::default();
    state.arch.set_gpr(GeneralRegister::R3, 0xCAFE);
    state.arch.set_pc(0xBEEF);
    state.arch.set_sp(0x1234);
    state.arch.set_flags(u16::MAX);
    state.arch.set_tick(77);
    state.arch.set_cause(0x3456);
    state.arch.set_evp_core_owned(0xABCD);

    state.reset_canonical();

    assert_eq!(state.arch.gpr(GeneralRegister::R3), 0);
    assert_eq!(state.arch.pc(), 0x0000);
    assert_eq!(state.arch.sp(), 0);
    assert_eq!(state.arch.flags(), 0);
    assert_eq!(state.arch.tick(), 0);
    assert_eq!(state.arch.cause(), 0);
    assert_eq!(state.arch.evp(), 0);
    assert_eq!(state.arch.cap(), CAP_AUTHORITY_DEFAULT_MASK);
}

#[test]
fn canonical_reset_clears_event_queue_and_fault_latch() {
    let mut state = CoreState {
        event_queue: EventQueueSnapshot {
            events: [0x11, 0x22, 0x33, 0x44],
            len: u8::try_from(EVENT_QUEUE_CAPACITY).expect("queue capacity fits in u8"),
        },
        run_state: RunState::FaultLatched(FaultCode::IllegalEncoding),
        ..CoreState::default()
    };

    state.reset_canonical();

    assert!(state.event_queue.is_empty());
    assert_eq!(state.run_state, RunState::Running);
}

#[test]
fn canonical_reset_keeps_profile_specific_cap_defaults() {
    let config = CoreConfig {
        profile: CoreProfile::Restricted,
        ..CoreConfig::default()
    };
    let mut state = CoreState::with_config(&config);
    assert_eq!(state.arch.cap(), CAP_RESTRICTED_DEFAULT_MASK);

    state.arch.set_cap_core_owned(CAP_AUTHORITY_DEFAULT_MASK);
    assert_eq!(state.arch.cap(), CAP_AUTHORITY_DEFAULT_MASK);

    state.reset_canonical();

    assert_eq!(state.arch.cap(), CAP_RESTRICTED_DEFAULT_MASK);
}

#[test]
fn canonical_reset_preserves_loaded_memory_image() {
    let mut state = CoreState::default();
    state.memory[0x0000] = 0xDE;
    state.memory[0x2345] = 0xAD;
    state.memory[usize::from(u16::MAX)] = 0xBE;

    state.reset_canonical();

    assert_eq!(state.memory[0x0000], 0xDE);
    assert_eq!(state.memory[0x2345], 0xAD);
    assert_eq!(state.memory[usize::from(u16::MAX)], 0xBE);
}
