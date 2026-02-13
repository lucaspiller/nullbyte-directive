//! Core emulator crate for Nullbyte Directive.

/// Public host-facing API contract and integration types.
pub mod api;
pub use api::{
    CoreConfig, CoreProfile, CoreSnapshot, CoreState, EventEnqueueError, EventQueueSnapshot,
    MmioBus, MmioError, MmioWriteResult, RunBoundary, RunOutcome, RunState, SnapshotVersion,
    StepOutcome, TraceEvent, TraceSink, DEFAULT_TICK_BUDGET_CYCLES, EVENT_QUEUE_CAPACITY,
};

/// Architectural CPU state model primitives.
pub mod state;
pub use state::{
    ArchitecturalState, GeneralRegister, CAP_AUTHORITY_DEFAULT_MASK, GENERAL_REGISTER_COUNT,
};

/// Deterministic opcode and encoding classification tables.
pub mod encoding;
pub use encoding::{
    classify_opcode, decode_primary_word_op_sub, is_reserved_primary_opcode, OpcodeClass,
    OpcodeEncoding, OPCODE_ENCODING_TABLE,
};
/// Fault taxonomy types for ISA-visible and runtime escalation faults.
pub mod fault;
pub use fault::{FaultClass, FaultCode};
/// Deterministic instruction cycle-cost table and lookup helpers.
pub mod timing;
pub use timing::{cycle_cost, CycleCostKind, CYCLE_COST_TABLE};

#[cfg(test)]
use proptest as _;
#[cfg(test)]
use rstest as _;
