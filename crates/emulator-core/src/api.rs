//! Public host-facing API contracts for embedding the emulator core.
//!
//! These are intentionally type-only scaffolds for FR-8/9/11/15 and NFR-4.

use crate::{
    ArchitecturalState, FaultCode, CAP_AUTHORITY_DEFAULT_MASK, CAP_RESTRICTED_DEFAULT_MASK,
};

/// Maximum number of pending external events accepted by the core queue.
pub const EVENT_QUEUE_CAPACITY: usize = 4;

/// Default cycle budget per tick.
pub const DEFAULT_TICK_BUDGET_CYCLES: u16 = 640;

/// Core execution profile controls capability defaults and policy hooks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum CoreProfile {
    /// Authority runtime default profile (`CAP[0..3] = 1`).
    #[default]
    Authority,
    /// Non-authority profile used for capability-gating tests and adapters.
    Restricted,
}

/// Top-level immutable configuration for a core instance.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct CoreConfig {
    /// Profile selection for capability defaults.
    pub profile: CoreProfile,
    /// Tick cycle budget checked at instruction boundaries.
    pub tick_budget_cycles: u16,
    /// Enables deterministic trace callback dispatch.
    pub tracing_enabled: bool,
}

impl Default for CoreConfig {
    fn default() -> Self {
        Self {
            profile: CoreProfile::Authority,
            tick_budget_cycles: DEFAULT_TICK_BUDGET_CYCLES,
            tracing_enabled: false,
        }
    }
}

impl CoreConfig {
    /// Returns the profile-specific default capability mask.
    #[must_use]
    pub const fn default_capability_mask(&self) -> u16 {
        match self.profile {
            CoreProfile::Authority => CAP_AUTHORITY_DEFAULT_MASK,
            CoreProfile::Restricted => CAP_RESTRICTED_DEFAULT_MASK,
        }
    }
}

/// Public run-state surface exposed to hosts and adapters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum RunState {
    /// Ready to execute the next instruction.
    Running,
    /// Halted for the remainder of the current tick.
    HaltedForTick,
    /// Currently inside a trap/event/fault handler context.
    HandlerContext,
    /// Fault is latched and no further progress is possible without reset/import.
    FaultLatched,
}

/// Complete host-visible core state snapshot used by stepping APIs.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct CoreState {
    /// Immutable runtime profile controlling baseline capability policy.
    pub profile: CoreProfile,
    /// Architectural register file and special register block.
    pub arch: ArchitecturalState,
    /// Flat 64 KiB memory image.
    pub memory: Box<[u8]>,
    /// Pending external events in deterministic FIFO order.
    pub event_queue: EventQueueSnapshot,
    /// Current execution state.
    pub run_state: RunState,
    /// Latched terminal or recoverable fault, when present.
    pub latched_fault: Option<FaultCode>,
}

impl Default for CoreState {
    fn default() -> Self {
        Self::with_config(&CoreConfig::default())
    }
}

impl CoreState {
    /// Creates a core state using profile-sensitive baseline defaults.
    #[must_use]
    pub fn with_config(config: &CoreConfig) -> Self {
        let mut arch = ArchitecturalState::default();
        arch.set_cap_core_owned(config.default_capability_mask());

        Self {
            profile: config.profile,
            arch,
            memory: vec![0; u16::MAX as usize + 1].into_boxed_slice(),
            event_queue: EventQueueSnapshot::default(),
            run_state: RunState::Running,
            latched_fault: None,
        }
    }

    /// Returns `true` when a capability bit is enabled in current state.
    #[must_use]
    pub const fn capability_enabled(&self, bit_index: u8) -> bool {
        self.arch.capability_enabled(bit_index)
    }

    /// Applies canonical reset semantics to the host-visible execution state.
    ///
    /// Reset restores architectural defaults, resumes at ROM entry
    /// (`PC=0x0000`), clears pending events, and clears any latched fault.
    pub fn reset_canonical(&mut self) {
        self.arch = ArchitecturalState::default();
        let cap_mask = match self.profile {
            CoreProfile::Authority => CAP_AUTHORITY_DEFAULT_MASK,
            CoreProfile::Restricted => CAP_RESTRICTED_DEFAULT_MASK,
        };
        self.arch.set_cap_core_owned(cap_mask);
        self.event_queue = EventQueueSnapshot::default();
        self.run_state = RunState::Running;
        self.latched_fault = None;
    }
}

/// Deterministic bounded external-event queue snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct EventQueueSnapshot {
    /// Events in dequeue order, valid up to `len`.
    pub events: [u8; EVENT_QUEUE_CAPACITY],
    /// Number of valid entries in `events`.
    pub len: u8,
}

impl EventQueueSnapshot {
    /// Returns true when no events are queued.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.len == 0
    }

    /// Returns true when queue has no remaining capacity.
    #[must_use]
    pub const fn is_full(self) -> bool {
        self.len as usize == EVENT_QUEUE_CAPACITY
    }
}

/// Error returned by host-driven event enqueue operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventEnqueueError {
    /// Queue is full; maps to deterministic overflow fault behavior.
    QueueFull,
}

impl EventEnqueueError {
    /// Maps enqueue failure to the canonical fault code surface.
    #[must_use]
    pub const fn fault_code(self) -> FaultCode {
        match self {
            Self::QueueFull => FaultCode::EventQueueOverflow,
        }
    }
}

/// MMIO adapter read/write transport failure categories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MmioError {
    /// Host adapter reported transport/device failure on read.
    ReadFailed,
    /// Host adapter reported transport/device failure on write.
    WriteFailed,
}

/// Result categories for MMIO write integration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MmioWriteResult {
    /// Write side effects are accepted and visible at commit.
    Applied,
    /// Write was denied and suppressed without ISA-visible trap/fault.
    DeniedSuppressed,
}

/// Deterministic MMIO bus contract consumed by step execution.
pub trait MmioBus {
    /// Reads a 16-bit value from the MMIO address space.
    ///
    /// # Errors
    ///
    /// Returns [`MmioError::ReadFailed`] when the adapter cannot complete the
    /// read.
    fn read16(&mut self, addr: u16) -> Result<u16, MmioError>;

    /// Writes a 16-bit value to the MMIO address space.
    ///
    /// # Errors
    ///
    /// Returns [`MmioError::WriteFailed`] when the adapter cannot complete the
    /// write.
    fn write16(&mut self, addr: u16, value: u16) -> Result<MmioWriteResult, MmioError>;
}

/// Output status from one instruction retirement attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StepOutcome {
    /// Instruction retired and consumed a fixed cycle cost.
    Retired {
        /// Fixed cycle cost consumed by the retired instruction.
        cycles: u16,
    },
    /// Core halted for the current tick after retiring `HALT`.
    HaltedForTick,
    /// Trap dispatch path was entered.
    TrapDispatch {
        /// ISA-visible trap cause payload.
        cause: u16,
    },
    /// Event dispatch path was entered.
    EventDispatch {
        /// 8-bit event identifier dequeued for dispatch.
        event_id: u8,
    },
    /// Fault was raised during decode/execute/dispatch.
    Fault {
        /// Canonical fault code raised by decode/execute/dispatch.
        cause: FaultCode,
    },
}

/// Run loop boundary modes for host-facing batched execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RunBoundary {
    /// Stop after reaching or crossing the current tick boundary.
    TickBoundary,
    /// Stop as soon as core enters halted-for-tick state.
    Halted,
    /// Stop when any fault is raised or latched.
    Fault,
}

/// Aggregated outcome from running multiple steps until a selected boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RunOutcome {
    /// Number of retired steps during this run call.
    pub steps: u32,
    /// Last step-level status observed before returning.
    pub final_step: StepOutcome,
}

/// Stable snapshot wire-version identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[repr(u16)]
pub enum SnapshotVersion {
    /// Initial schema revision for emulator-core v0.1.x.
    V1 = 1,
}

impl SnapshotVersion {
    /// Converts wire value to known snapshot version.
    #[must_use]
    pub const fn from_u16(version: u16) -> Option<Self> {
        match version {
            1 => Some(Self::V1),
            _ => None,
        }
    }
}

/// Serializable full-state snapshot used for import/export and replay fixtures.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct CoreSnapshot {
    /// Snapshot schema version.
    pub version: SnapshotVersion,
    /// Full host-visible core state.
    pub state: CoreState,
}

/// Deterministic trace events emitted at step boundaries when enabled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TraceEvent {
    /// Pre-execute event for instruction fetch/decode.
    InstructionStart {
        /// Program counter used for this fetch.
        pc: u16,
        /// Raw 16-bit primary instruction word.
        raw_word: u16,
    },
    /// Post-retire event reporting consumed cycles.
    InstructionRetired {
        /// Program counter of the retired instruction.
        pc: u16,
        /// Fixed cycle cost consumed by this retirement.
        cycles: u16,
    },
    /// Memory access event in architected commit order.
    MemoryAccess {
        /// Access target address.
        addr: u16,
        /// 16-bit value read or written.
        value: u16,
        /// True for writes, false for reads.
        is_write: bool,
        /// True when the access targets MMIO space.
        is_mmio: bool,
    },
    /// Fault emission event.
    FaultRaised {
        /// Canonical raised fault code.
        cause: FaultCode,
        /// Program counter active when fault was observed.
        pc: u16,
    },
}

/// Sink trait for deterministic trace hooks.
pub trait TraceSink {
    /// Records an event in execution order.
    fn on_event(&mut self, event: TraceEvent);
}

#[cfg(test)]
mod tests {
    use super::{
        CoreConfig, CoreProfile, CoreState, EventEnqueueError, EventQueueSnapshot, RunState,
        SnapshotVersion, DEFAULT_TICK_BUDGET_CYCLES, EVENT_QUEUE_CAPACITY,
    };
    use crate::{
        ArchitecturalState, FaultCode, GeneralRegister, CAP_AUTHORITY_DEFAULT_MASK,
        CAP_RESTRICTED_DEFAULT_MASK,
    };

    #[test]
    fn default_core_config_matches_prd_contract() {
        let config = CoreConfig::default();

        assert_eq!(config.profile, CoreProfile::Authority);
        assert_eq!(config.tick_budget_cycles, DEFAULT_TICK_BUDGET_CYCLES);
        assert!(!config.tracing_enabled);
    }

    #[test]
    fn event_queue_snapshot_capacity_helpers_are_consistent() {
        let empty = EventQueueSnapshot::default();
        assert!(empty.is_empty());
        assert!(!empty.is_full());

        let queue_len_capacity = u8::try_from(super::EVENT_QUEUE_CAPACITY)
            .expect("event queue capacity must fit in queue length field");
        let full = EventQueueSnapshot {
            len: queue_len_capacity,
            ..EventQueueSnapshot::default()
        };
        assert!(!full.is_empty());
        assert!(full.is_full());
    }

    #[test]
    fn enqueue_error_maps_to_event_queue_overflow_fault() {
        assert_eq!(
            EventEnqueueError::QueueFull.fault_code(),
            FaultCode::EventQueueOverflow
        );
    }

    #[test]
    fn snapshot_version_roundtrip_is_stable() {
        assert_eq!(SnapshotVersion::from_u16(1), Some(SnapshotVersion::V1));
        assert_eq!(SnapshotVersion::from_u16(2), None);
    }

    #[test]
    fn core_state_default_allocates_full_address_space() {
        let state = CoreState::default();
        assert_eq!(state.profile, CoreProfile::Authority);
        assert_eq!(state.memory.len(), u16::MAX as usize + 1);
        assert_eq!(state.arch.cap(), CAP_AUTHORITY_DEFAULT_MASK);
    }

    #[test]
    fn canonical_reset_restores_defaults_and_boot_entry() {
        let mut state = CoreState::default();
        state.arch.set_gpr(GeneralRegister::R0, 0x1234);
        state.arch.set_pc(0x4567);
        state.arch.set_sp(0x89AB);
        state.arch.set_flags(u16::MAX);
        state.arch.set_tick(0x00FF);
        state.arch.set_cause(0x1122);
        state.arch.set_evp_core_owned(0x3344);
        state.event_queue = EventQueueSnapshot {
            events: [0xAA; EVENT_QUEUE_CAPACITY],
            len: u8::try_from(EVENT_QUEUE_CAPACITY).expect("queue capacity must fit in u8"),
        };
        state.run_state = RunState::FaultLatched;
        state.latched_fault = Some(FaultCode::IllegalEncoding);

        state.reset_canonical();

        assert_eq!(state.arch, ArchitecturalState::default());
        assert_eq!(state.arch.pc(), 0x0000);
        assert_eq!(state.run_state, RunState::Running);
        assert!(state.event_queue.is_empty());
        assert!(state.latched_fault.is_none());
    }

    #[test]
    fn core_state_with_restricted_profile_uses_restricted_cap_defaults() {
        let config = CoreConfig {
            profile: CoreProfile::Restricted,
            ..CoreConfig::default()
        };
        let mut state = CoreState::with_config(&config);
        assert_eq!(state.profile, CoreProfile::Restricted);
        assert_eq!(state.arch.cap(), CAP_RESTRICTED_DEFAULT_MASK);
        assert!(!state.capability_enabled(0));
        assert!(!state.capability_enabled(15));

        state.arch.set_cap_core_owned(CAP_AUTHORITY_DEFAULT_MASK);
        assert!(state.capability_enabled(0));
        state.reset_canonical();

        assert_eq!(state.arch.cap(), CAP_RESTRICTED_DEFAULT_MASK);
        assert!(!state.capability_enabled(0));
    }

    #[test]
    fn canonical_reset_preserves_memory_image() {
        let mut state = CoreState::default();
        state.memory[0x0000] = 0xDE;
        state.memory[0x1234] = 0xAD;
        state.memory[usize::from(u16::MAX)] = 0xBE;

        state.reset_canonical();

        assert_eq!(state.memory[0x0000], 0xDE);
        assert_eq!(state.memory[0x1234], 0xAD);
        assert_eq!(state.memory[usize::from(u16::MAX)], 0xBE);
    }

    #[test]
    fn queue_capacity_fits_u8_len_field() {
        let capacity = u8::try_from(super::EVENT_QUEUE_CAPACITY)
            .expect("event queue capacity must fit in queue length field");
        assert_eq!(capacity, 4);
    }
}
