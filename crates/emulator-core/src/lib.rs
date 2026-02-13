//! Core emulator crate for Nullbyte Directive.

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
