//! Core emulator crate for Nullbyte Directive.

/// Memory model primitives and fixed region map.
pub mod memory;
pub use memory::{
    decode_memory_region, new_address_space, validate_fetch_access, validate_mmio_alignment,
    validate_mmio_width, validate_word_alignment, validate_write_access, MemoryRegion,
    RegionDescriptor, ADDRESS_SPACE_BYTES, DIAG_END, DIAG_START, FIXED_MEMORY_REGIONS, MMIO_END,
    MMIO_START, RAM_END, RAM_START, RESERVED_END, RESERVED_START, ROM_END, ROM_START,
    WORD_ACCESS_BYTES,
};

/// Public host-facing API contract and integration types.
pub mod api;
pub use api::{
    CanonicalStateLayout, CoreConfig, CoreProfile, CoreSnapshot, CoreState, EventEnqueueError,
    EventQueueSnapshot, MmioBus, MmioError, MmioWriteResult, RunBoundary, RunOutcome,
    SnapshotLayoutError, SnapshotVersion, StepOutcome, TraceEvent, TraceSink,
    DEFAULT_TICK_BUDGET_CYCLES, EVENT_QUEUE_CAPACITY,
};

/// Architectural CPU state model primitives.
pub mod state;
pub use state::{
    ArchitecturalState, GeneralRegister, RunState, CAP_AUTHORITY_DEFAULT_MASK,
    CAP_RESTRICTED_DEFAULT_MASK, GENERAL_REGISTER_COUNT,
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
