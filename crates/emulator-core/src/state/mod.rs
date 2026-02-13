//! Architectural CPU state model primitives.

/// Architectural register file types and storage model.
pub mod registers;

pub use registers::{ArchitecturalState, GeneralRegister, GENERAL_REGISTER_COUNT};
