//! Diagnostics window (DIAG) model and provider trait.

use crate::{FaultClass, FaultCode};

/// Offset for the last fault code in the DIAG region.
pub const DIAG_LAST_FAULT_CODE_OFFSET: u16 = 0x00;
/// Offset for the last faulting PC in the DIAG region.
pub const DIAG_LAST_FAULT_PC_OFFSET: u16 = 0x02;
/// Offset for the last fault tick index in the DIAG region.
pub const DIAG_LAST_FAULT_TICK_OFFSET: u16 = 0x04;
/// Offset for the decode fault counter in the DIAG region.
pub const DIAG_FAULT_COUNT_DECODE_OFFSET: u16 = 0x06;
/// Offset for the memory fault counter in the DIAG region.
pub const DIAG_FAULT_COUNT_MEMORY_OFFSET: u16 = 0x08;
/// Offset for the MMIO fault counter in the DIAG region.
pub const DIAG_FAULT_COUNT_MMIO_OFFSET: u16 = 0x0A;
/// Offset for the event fault counter in the DIAG region.
pub const DIAG_FAULT_COUNT_EVENT_OFFSET: u16 = 0x0C;
/// Offset for the dispatch fault counter in the DIAG region.
pub const DIAG_FAULT_COUNT_DISPATCH_OFFSET: u16 = 0x0E;
/// Offset for the budget fault counter in the DIAG region.
pub const DIAG_FAULT_COUNT_BUDGET_OFFSET: u16 = 0x10;
/// Offset for the capability fault counter in the DIAG region.
pub const DIAG_FAULT_COUNT_CAPABILITY_OFFSET: u16 = 0x12;
/// Offset for the executed instruction counter in the DIAG region.
pub const DIAG_INSTRUCTION_COUNT_OFFSET: u16 = 0x14;
/// Offset for the denied write counter in the DIAG region.
pub const DIAG_DENIED_WRITE_COUNT_OFFSET: u16 = 0x16;

/// Number of core-owned fields in the DIAG window.
pub const DIAG_CORE_OWNED_FIELD_COUNT: usize = 11;

/// Core-owned diagnostic fields visible in the DIAG memory region.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DiagCoreFields {
    /// The last fault code that occurred, if any.
    pub last_fault_code: Option<FaultCode>,
    /// The program counter at the time of the last fault.
    pub last_fault_pc: u16,
    /// The tick counter at the time of the last fault.
    pub last_fault_tick: u32,
    /// Saturating counter for decode-class faults.
    pub fault_count_decode: u16,
    /// Saturating counter for memory-class faults.
    pub fault_count_memory: u16,
    /// Saturating counter for MMIO-class faults.
    pub fault_count_mmio: u16,
    /// Saturating counter for event-class faults.
    pub fault_count_event: u16,
    /// Saturating counter for dispatch-class faults.
    pub fault_count_dispatch: u16,
    /// Saturating counter for budget-class faults.
    pub fault_count_budget: u16,
    /// Saturating counter for capability-class faults.
    pub fault_count_capability: u16,
    /// Saturating counter for executed instructions.
    pub instruction_count: u16,
    /// Saturating counter for denied MMIO writes.
    pub denied_write_count: u16,
}

impl DiagCoreFields {
    /// Creates a new set of diagnostic fields with default values.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Records a fault occurrence, updating the last fault info and incrementing
    /// the appropriate fault class counter.
    #[allow(clippy::missing_const_for_fn)]
    pub fn record_fault(&mut self, code: FaultCode, pc: u16, tick: u32) {
        self.last_fault_code = Some(code);
        self.last_fault_pc = pc;
        self.last_fault_tick = tick;
        let class = code.class();
        match class {
            FaultClass::Decode => {
                self.fault_count_decode = self.fault_count_decode.saturating_add(1);
            }
            FaultClass::Memory => {
                self.fault_count_memory = self.fault_count_memory.saturating_add(1);
            }
            FaultClass::Mmio => {
                self.fault_count_mmio = self.fault_count_mmio.saturating_add(1);
            }
            FaultClass::Event => {
                self.fault_count_event = self.fault_count_event.saturating_add(1);
            }
            FaultClass::Dispatch => {
                self.fault_count_dispatch = self.fault_count_dispatch.saturating_add(1);
            }
            FaultClass::Budget => {
                self.fault_count_budget = self.fault_count_budget.saturating_add(1);
            }
            FaultClass::Capability => {
                self.fault_count_capability = self.fault_count_capability.saturating_add(1);
            }
        }
    }

    /// Increments the instruction counter with saturating behavior.
    #[allow(clippy::missing_const_for_fn)]
    pub fn increment_instruction_count(&mut self) {
        self.instruction_count = self.instruction_count.saturating_add(1);
    }

    /// Records a denied MMIO write occurrence.
    #[allow(clippy::missing_const_for_fn)]
    pub fn record_denied_write(&mut self) {
        self.denied_write_count = self.denied_write_count.saturating_add(1);
    }

    /// Resets all diagnostic fields to their default values.
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

/// Trait for providing DIAG region data.
///
/// Implementors can provide custom diagnostic data while preserving
/// core-owned fields.
pub trait DiagProvider: Send + Sync {
    /// Returns the core-owned diagnostic fields.
    fn get_core_fields(&self) -> DiagCoreFields;
    /// Reads a byte from the user-defined portion of the DIAG region.
    fn read_user_byte(&self, offset: u16) -> Option<u8>;
    /// Writes a byte to the user-defined portion of the DIAG region.
    fn write_user_byte(&mut self, offset: u16, value: u8);
}

/// Default DIAG provider with static storage for user-defined diagnostics.
#[derive(Debug, Clone)]
pub struct StaticDiagProvider {
    core_fields: DiagCoreFields,
    user_space: [u8; 232],
}

impl StaticDiagProvider {
    /// Creates a new static DIAG provider.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns a mutable reference to the core fields for core-owned updates.
    #[allow(clippy::missing_const_for_fn)]
    pub fn get_core_fields_mut(&mut self) -> &mut DiagCoreFields {
        &mut self.core_fields
    }
}

impl Default for StaticDiagProvider {
    fn default() -> Self {
        Self {
            core_fields: DiagCoreFields::default(),
            user_space: [0; 232],
        }
    }
}

impl DiagProvider for StaticDiagProvider {
    fn get_core_fields(&self) -> DiagCoreFields {
        self.core_fields
    }

    fn read_user_byte(&self, offset: u16) -> Option<u8> {
        let idx = offset.checked_sub(0x18)? as usize;
        if idx < self.user_space.len() {
            Some(self.user_space[idx])
        } else {
            None
        }
    }

    fn write_user_byte(&mut self, offset: u16, value: u8) {
        if let Some(idx) = offset.checked_sub(0x18) {
            let idx = idx as usize;
            if idx < self.user_space.len() {
                self.user_space[idx] = value;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diag_core_fields_default() {
        let fields = DiagCoreFields::default();
        assert_eq!(fields.last_fault_code, None);
        assert_eq!(fields.last_fault_pc, 0);
        assert_eq!(fields.last_fault_tick, 0);
        assert_eq!(fields.fault_count_decode, 0);
    }

    #[test]
    fn record_fault_increments_decode_count() {
        let mut fields = DiagCoreFields::default();
        fields.record_fault(FaultCode::IllegalEncoding, 0x0100, 100);
        assert_eq!(fields.fault_count_decode, 1);
        assert_eq!(fields.last_fault_code, Some(FaultCode::IllegalEncoding));
    }

    #[test]
    fn record_fault_saturates_at_max() {
        let mut fields = DiagCoreFields::default();
        for _ in 0..0x20000 {
            fields.record_fault(FaultCode::IllegalEncoding, 0, 0);
        }
        assert_eq!(fields.fault_count_decode, u16::MAX);
    }

    #[test]
    fn instruction_count_saturates() {
        let mut fields = DiagCoreFields::default();
        for _ in 0..0x20000 {
            fields.increment_instruction_count();
        }
        assert_eq!(fields.instruction_count, u16::MAX);
    }

    #[test]
    fn denied_write_count_increments() {
        let mut fields = DiagCoreFields::default();
        fields.record_denied_write();
        assert_eq!(fields.denied_write_count, 1);
    }

    #[test]
    fn static_diag_provider_user_space_bounds() {
        let provider = StaticDiagProvider::new();
        assert_eq!(provider.read_user_byte(0x17), None);
        assert_eq!(provider.read_user_byte(0x18), Some(0));
        assert_eq!(provider.read_user_byte(0xFF), Some(0));
        assert_eq!(provider.read_user_byte(0x100), None);
    }

    #[test]
    fn static_diag_provider_reads_core_fields() {
        let provider = StaticDiagProvider::new();
        let fields = provider.get_core_fields();
        assert_eq!(fields.last_fault_code, None);
    }

    #[test]
    fn record_fault_increments_all_classes() {
        let mut fields = DiagCoreFields::default();
        fields.record_fault(FaultCode::IllegalEncoding, 0, 0);
        assert_eq!(fields.fault_count_decode, 1);

        fields.record_fault(FaultCode::IllegalMemoryAccess, 0, 0);
        assert_eq!(fields.fault_count_memory, 1);

        fields.record_fault(FaultCode::MmioWidthViolation, 0, 0);
        assert_eq!(fields.fault_count_mmio, 1);

        fields.record_fault(FaultCode::EventQueueOverflow, 0, 0);
        assert_eq!(fields.fault_count_event, 1);

        fields.record_fault(FaultCode::DoubleFault, 0, 0);
        assert_eq!(fields.fault_count_dispatch, 1);

        fields.record_fault(FaultCode::BudgetOverrun, 0, 0);
        assert_eq!(fields.fault_count_budget, 1);

        fields.record_fault(FaultCode::CapabilityViolation, 0, 0);
        assert_eq!(fields.fault_count_capability, 1);
    }
}
