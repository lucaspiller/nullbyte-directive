//! Architectural CPU state model primitives.

/// Architectural register file types and storage model.
pub mod registers;
/// Host-visible run-state machine types.
pub mod run_state;

pub use registers::{
    ArchitecturalState, GeneralRegister, CAP_AUTHORITY_DEFAULT_MASK, CAP_RESTRICTED_DEFAULT_MASK,
    GENERAL_REGISTER_COUNT,
};
pub use run_state::RunState;
