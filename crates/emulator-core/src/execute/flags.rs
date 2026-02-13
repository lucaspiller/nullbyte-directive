//! FLAGS update behaviors for different instruction classes.

/// Describes how FLAGS should be updated after an instruction executes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FlagsUpdate {
    /// No change to FLAGS.
    #[default]
    None,
    /// Clear all FLAGS.
    Clear,
    /// Set FLAGS to a specific value.
    Set(u16),
    /// Update individual NZCV flags.
    UpdateNZ {
        /// Zero flag.
        zero: bool,
        /// Negative flag.
        negative: bool,
        /// Carry flag.
        carry: bool,
        /// Overflow flag.
        overflow: bool,
    },
}
