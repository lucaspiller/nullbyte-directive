//! Core emulator crate for Nullbyte Directive.

/// Fault taxonomy types for ISA-visible and runtime escalation faults.
pub mod fault;
pub use fault::{FaultClass, FaultCode};

#[cfg(test)]
use proptest as _;
#[cfg(test)]
use rstest as _;
