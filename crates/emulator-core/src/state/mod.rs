//! Architectural CPU state model primitives.

/// Architectural register file types and storage model.
pub mod registers;

pub use registers::{
    ArchitecturalState, GeneralRegister, CAP_AUTHORITY_DEFAULT_MASK, GENERAL_REGISTER_COUNT,
};
