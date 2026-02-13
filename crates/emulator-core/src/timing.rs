/// Instruction and dispatch forms that have fixed cycle costs in the core.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CycleCostKind {
    /// No-operation instruction.
    Nop,
    /// Memory/MMIO visibility barrier instruction.
    Sync,
    /// Halt-for-remainder-of-tick instruction.
    Halt,
    /// Trap issue instruction path.
    TrapIssue,
    /// Software-interrupt issue instruction path.
    SwiIssue,
    /// Register/immediate move.
    Mov,
    /// Memory read instruction.
    Load,
    /// Memory write instruction.
    Store,
    /// Integer add/sub/logic/compare class.
    Alu,
    /// Integer multiply low/high class.
    Mul,
    /// Integer divide/modulo class.
    Div,
    /// Saturating helper class (`QADD`, `QSUB`, `SCV`).
    SaturatingHelper,
    /// Conditional branch when predicate is false.
    BranchNotTaken,
    /// Conditional branch when predicate is true.
    BranchTaken,
    /// Unconditional jump.
    Jump,
    /// Subroutine call.
    Call,
    /// Subroutine return.
    Ret,
    /// Stack push.
    Push,
    /// Stack pop.
    Pop,
    /// MMIO input read.
    MmioIn,
    /// MMIO output write.
    MmioOut,
    /// MMIO atomic bit-set.
    MmioBitSet,
    /// MMIO atomic bit-clear.
    MmioBitClear,
    /// MMIO atomic bit-test.
    MmioBitTest,
    /// Wait-for-event instruction.
    Ewait,
    /// Event dequeue instruction.
    Eget,
    /// Trap dispatch entry sequence.
    TrapDispatchEntry,
    /// Event dispatch entry sequence.
    EventDispatchEntry,
    /// Fault dispatch entry sequence.
    FaultDispatchEntry,
    /// Handler return sequence.
    EretReturn,
}

/// Single source-of-truth cycle-cost table for fixed-cost instruction/dispatch forms.
pub const CYCLE_COST_TABLE: &[(CycleCostKind, u16)] = &[
    (CycleCostKind::Nop, 1),
    (CycleCostKind::Sync, 1),
    (CycleCostKind::Halt, 1),
    (CycleCostKind::TrapIssue, 1),
    (CycleCostKind::SwiIssue, 1),
    (CycleCostKind::Mov, 1),
    (CycleCostKind::Load, 2),
    (CycleCostKind::Store, 2),
    (CycleCostKind::Alu, 1),
    (CycleCostKind::Mul, 2),
    (CycleCostKind::Div, 3),
    (CycleCostKind::SaturatingHelper, 1),
    (CycleCostKind::BranchNotTaken, 1),
    (CycleCostKind::BranchTaken, 2),
    (CycleCostKind::Jump, 2),
    (CycleCostKind::Call, 2),
    (CycleCostKind::Ret, 2),
    (CycleCostKind::Push, 1),
    (CycleCostKind::Pop, 1),
    (CycleCostKind::MmioIn, 4),
    (CycleCostKind::MmioOut, 4),
    (CycleCostKind::MmioBitSet, 4),
    (CycleCostKind::MmioBitClear, 4),
    (CycleCostKind::MmioBitTest, 4),
    (CycleCostKind::Ewait, 1),
    (CycleCostKind::Eget, 1),
    (CycleCostKind::TrapDispatchEntry, 5),
    (CycleCostKind::EventDispatchEntry, 5),
    (CycleCostKind::FaultDispatchEntry, 5),
    (CycleCostKind::EretReturn, 4),
];

/// Looks up the cycle cost for a cycle-cost kind.
#[must_use]
pub fn cycle_cost(kind: CycleCostKind) -> Option<u16> {
    CYCLE_COST_TABLE
        .iter()
        .find_map(|(entry_kind, cycles)| (*entry_kind == kind).then_some(*cycles))
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::{cycle_cost, CycleCostKind, CYCLE_COST_TABLE};

    #[test]
    fn table_contains_unique_kinds() {
        let kinds: HashSet<_> = CYCLE_COST_TABLE.iter().map(|(kind, _)| *kind).collect();
        assert_eq!(kinds.len(), CYCLE_COST_TABLE.len());
    }

    #[test]
    fn table_values_match_canonical_costs() {
        assert_eq!(cycle_cost(CycleCostKind::Nop), Some(1));
        assert_eq!(cycle_cost(CycleCostKind::Load), Some(2));
        assert_eq!(cycle_cost(CycleCostKind::Div), Some(3));
        assert_eq!(cycle_cost(CycleCostKind::BranchNotTaken), Some(1));
        assert_eq!(cycle_cost(CycleCostKind::BranchTaken), Some(2));
        assert_eq!(cycle_cost(CycleCostKind::MmioOut), Some(4));
        assert_eq!(cycle_cost(CycleCostKind::FaultDispatchEntry), Some(5));
        assert_eq!(cycle_cost(CycleCostKind::EretReturn), Some(4));
    }

    #[test]
    fn every_table_entry_resolves_via_lookup() {
        for (kind, expected_cycles) in CYCLE_COST_TABLE {
            assert_eq!(cycle_cost(*kind), Some(*expected_cycles));
        }
    }
}
