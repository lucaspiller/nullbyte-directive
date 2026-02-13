//! Core emulator crate for Nullbyte Directive.

/// Memory model primitives and fixed region map.
pub mod memory;
pub use memory::{
    decode_memory_region, new_address_space, read_u16_be, validate_fetch_access,
    validate_mmio_alignment, validate_mmio_width, validate_word_alignment, validate_write_access,
    write_u16_be, MemoryRegion, RegionDescriptor, ADDRESS_SPACE_BYTES, DIAG_END, DIAG_START,
    FIXED_MEMORY_REGIONS, MMIO_END, MMIO_START, RAM_END, RAM_START, RESERVED_END, RESERVED_START,
    ROM_END, ROM_START, WORD_ACCESS_BYTES,
};

/// Diagnostics window (DIAG) model and provider trait.
pub mod diag;
pub use diag::{
    DiagCoreFields, DiagProvider, StaticDiagProvider, DIAG_DENIED_WRITE_COUNT_OFFSET,
    DIAG_FAULT_COUNT_BUDGET_OFFSET, DIAG_FAULT_COUNT_CAPABILITY_OFFSET,
    DIAG_FAULT_COUNT_DECODE_OFFSET, DIAG_FAULT_COUNT_DISPATCH_OFFSET,
    DIAG_FAULT_COUNT_EVENT_OFFSET, DIAG_FAULT_COUNT_MEMORY_OFFSET, DIAG_FAULT_COUNT_MMIO_OFFSET,
    DIAG_INSTRUCTION_COUNT_OFFSET, DIAG_LAST_FAULT_CODE_OFFSET, DIAG_LAST_FAULT_PC_OFFSET,
    DIAG_LAST_FAULT_TICK_OFFSET,
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

/// Instruction decode pipeline with field extraction and validation.
pub mod decoder;
pub use decoder::{AddressingMode, DecodedInstruction, DecodedOrFault, Decoder, RegisterField};

/// Fault taxonomy types for ISA-visible and runtime escalation faults.
pub mod fault;
pub use fault::{FaultClass, FaultCode};
/// Deterministic instruction cycle-cost table and lookup helpers.
pub mod timing;
pub use timing::{cycle_cost, CycleCostKind, CYCLE_COST_TABLE};

/// Instruction execution pipeline.
pub mod execute;
pub use execute::{
    commit_execution, execute_instruction, step_one, ExecuteOutcome, ExecuteState, FlagsUpdate,
};

#[cfg(test)]
use proptest as _;
#[cfg(test)]
use rstest as _;
