use crate::FaultCode;

/// Deterministic execution-state machine for host-observable core control flow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum RunState {
    /// Ready to execute the next instruction.
    #[default]
    Running,
    /// Halted for the remainder of the current tick.
    HaltedForTick,
    /// Currently inside a trap/event/fault handler context.
    HandlerContext,
    /// Fault is latched and no further progress is possible without reset/import.
    FaultLatched(FaultCode),
}

impl RunState {
    /// Returns the currently latched fault, if this state is fault-latched.
    #[must_use]
    pub const fn latched_fault(self) -> Option<FaultCode> {
        match self {
            Self::FaultLatched(cause) => Some(cause),
            Self::Running | Self::HaltedForTick | Self::HandlerContext => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::RunState;
    use crate::FaultCode;

    #[test]
    fn run_state_default_is_running() {
        assert_eq!(RunState::default(), RunState::Running);
    }

    #[test]
    fn latched_fault_accessor_reports_only_fault_latched_variant() {
        assert_eq!(RunState::Running.latched_fault(), None);
        assert_eq!(RunState::HaltedForTick.latched_fault(), None);
        assert_eq!(RunState::HandlerContext.latched_fault(), None);
        assert_eq!(
            RunState::FaultLatched(FaultCode::IllegalEncoding).latched_fault(),
            Some(FaultCode::IllegalEncoding)
        );
    }
}
