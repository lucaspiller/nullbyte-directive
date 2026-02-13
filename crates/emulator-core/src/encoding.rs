/// Opcode classes with assigned primary opcode values (`OP` field, bits 15..12).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
#[allow(missing_docs)]
pub enum OpcodeClass {
    Control = 0x0,
    Mov = 0x1,
    Load = 0x2,
    Store = 0x3,
    Alu = 0x4,
    MathHelper = 0x5,
    Branch = 0x6,
    Stack = 0x7,
    Mmio = 0x8,
    AtomicMmio = 0x9,
    Event = 0xA,
}

impl OpcodeClass {
    /// Converts a 4-bit primary opcode value into an assigned class.
    #[must_use]
    pub const fn from_u4(op: u8) -> Option<Self> {
        match op {
            0x0 => Some(Self::Control),
            0x1 => Some(Self::Mov),
            0x2 => Some(Self::Load),
            0x3 => Some(Self::Store),
            0x4 => Some(Self::Alu),
            0x5 => Some(Self::MathHelper),
            0x6 => Some(Self::Branch),
            0x7 => Some(Self::Stack),
            0x8 => Some(Self::Mmio),
            0x9 => Some(Self::AtomicMmio),
            0xA => Some(Self::Event),
            _ => None,
        }
    }
}

/// Canonical assigned `(OP, SUB)` encodings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[allow(missing_docs)]
pub enum OpcodeEncoding {
    Nop,
    Sync,
    Halt,
    Trap,
    Swi,
    Mov,
    Load,
    Store,
    Add,
    Sub,
    And,
    Or,
    Xor,
    Shl,
    Shr,
    Cmp,
    Mul,
    Mulh,
    Div,
    Mod,
    Qadd,
    Qsub,
    Scv,
    Beq,
    Bne,
    Blt,
    Ble,
    Bgt,
    Bge,
    Jmp,
    CallOrRet,
    Push,
    Pop,
    In,
    Out,
    Bset,
    Bclr,
    Btest,
    Ewait,
    Eget,
    Eret,
}

/// Single source-of-truth assigned opcode/encoding table.
///
/// Any `(OP, SUB)` pair not present here is illegal by definition.
pub const OPCODE_ENCODING_TABLE: &[(u8, u8, OpcodeEncoding)] = &[
    (0x0, 0x0, OpcodeEncoding::Nop),
    (0x0, 0x1, OpcodeEncoding::Sync),
    (0x0, 0x2, OpcodeEncoding::Halt),
    (0x0, 0x3, OpcodeEncoding::Trap),
    (0x0, 0x4, OpcodeEncoding::Swi),
    (0x1, 0x0, OpcodeEncoding::Mov),
    (0x2, 0x0, OpcodeEncoding::Load),
    (0x3, 0x0, OpcodeEncoding::Store),
    (0x4, 0x0, OpcodeEncoding::Add),
    (0x4, 0x1, OpcodeEncoding::Sub),
    (0x4, 0x2, OpcodeEncoding::And),
    (0x4, 0x3, OpcodeEncoding::Or),
    (0x4, 0x4, OpcodeEncoding::Xor),
    (0x4, 0x5, OpcodeEncoding::Shl),
    (0x4, 0x6, OpcodeEncoding::Shr),
    (0x4, 0x7, OpcodeEncoding::Cmp),
    (0x5, 0x0, OpcodeEncoding::Mul),
    (0x5, 0x1, OpcodeEncoding::Mulh),
    (0x5, 0x2, OpcodeEncoding::Div),
    (0x5, 0x3, OpcodeEncoding::Mod),
    (0x5, 0x4, OpcodeEncoding::Qadd),
    (0x5, 0x5, OpcodeEncoding::Qsub),
    (0x5, 0x6, OpcodeEncoding::Scv),
    (0x6, 0x0, OpcodeEncoding::Beq),
    (0x6, 0x1, OpcodeEncoding::Bne),
    (0x6, 0x2, OpcodeEncoding::Blt),
    (0x6, 0x3, OpcodeEncoding::Ble),
    (0x6, 0x4, OpcodeEncoding::Bgt),
    (0x6, 0x5, OpcodeEncoding::Bge),
    (0x6, 0x6, OpcodeEncoding::Jmp),
    (0x6, 0x7, OpcodeEncoding::CallOrRet),
    (0x7, 0x0, OpcodeEncoding::Push),
    (0x7, 0x1, OpcodeEncoding::Pop),
    (0x8, 0x0, OpcodeEncoding::In),
    (0x8, 0x1, OpcodeEncoding::Out),
    (0x9, 0x0, OpcodeEncoding::Bset),
    (0x9, 0x1, OpcodeEncoding::Bclr),
    (0x9, 0x2, OpcodeEncoding::Btest),
    (0xA, 0x0, OpcodeEncoding::Ewait),
    (0xA, 0x1, OpcodeEncoding::Eget),
    (0xA, 0x2, OpcodeEncoding::Eret),
];

/// Returns true if the primary opcode nibble is in the reserved range (`0xB..=0xF`).
#[must_use]
pub const fn is_reserved_primary_opcode(op: u8) -> bool {
    matches!(op, 0xB..=0xF)
}

/// Returns the assigned opcode encoding for a primary opcode/sub-opcode pair.
///
/// `None` means illegal/reserved encoding.
#[must_use]
pub fn classify_opcode(op: u8, sub: u8) -> Option<OpcodeEncoding> {
    if op > 0xF || sub > 0x7 {
        return None;
    }

    OPCODE_ENCODING_TABLE
        .iter()
        .find_map(|(entry_op, entry_sub, encoding)| {
            ((*entry_op == op) && (*entry_sub == sub)).then_some(*encoding)
        })
}

/// Extracts the `(OP, SUB)` pair from the primary instruction word.
#[must_use]
pub const fn decode_primary_word_op_sub(word: u16) -> (u8, u8) {
    (((word >> 12) & 0x000F) as u8, ((word >> 3) & 0x0007) as u8)
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::{
        classify_opcode, decode_primary_word_op_sub, is_reserved_primary_opcode, OpcodeClass,
        OpcodeEncoding, OPCODE_ENCODING_TABLE,
    };

    #[test]
    fn table_contains_unique_op_sub_pairs() {
        let pairs: HashSet<_> = OPCODE_ENCODING_TABLE
            .iter()
            .map(|(op, sub, _)| (*op, *sub))
            .collect();
        assert_eq!(pairs.len(), OPCODE_ENCODING_TABLE.len());
    }

    #[test]
    fn lookup_matches_known_assigned_encodings() {
        assert_eq!(classify_opcode(0x0, 0x0), Some(OpcodeEncoding::Nop));
        assert_eq!(classify_opcode(0x4, 0x7), Some(OpcodeEncoding::Cmp));
        assert_eq!(classify_opcode(0x6, 0x7), Some(OpcodeEncoding::CallOrRet));
        assert_eq!(classify_opcode(0xA, 0x2), Some(OpcodeEncoding::Eret));
    }

    #[test]
    fn every_table_entry_resolves_via_lookup() {
        for (op, sub, encoding) in OPCODE_ENCODING_TABLE {
            assert_eq!(classify_opcode(*op, *sub), Some(*encoding));
        }
    }

    #[test]
    fn reserved_primary_opcodes_are_illegal() {
        for op in 0xBu8..=0xFu8 {
            assert!(is_reserved_primary_opcode(op));
            for sub in 0x0u8..=0x7u8 {
                assert_eq!(classify_opcode(op, sub), None);
            }
        }
    }

    #[test]
    fn unassigned_sub_opcodes_are_illegal() {
        assert_eq!(classify_opcode(0x0, 0x7), None);
        assert_eq!(classify_opcode(0x1, 0x1), None);
        assert_eq!(classify_opcode(0x2, 0x3), None);
        assert_eq!(classify_opcode(0x3, 0x6), None);
        assert_eq!(classify_opcode(0x5, 0x7), None);
        assert_eq!(classify_opcode(0x7, 0x7), None);
        assert_eq!(classify_opcode(0x8, 0x4), None);
        assert_eq!(classify_opcode(0x9, 0x3), None);
        assert_eq!(classify_opcode(0xA, 0x7), None);
    }

    #[test]
    fn primary_word_decode_extracts_op_and_sub_fields() {
        let word = 0b1010_0010_1101_0101_u16;
        assert_eq!(decode_primary_word_op_sub(word), (0xA, 0x2));
    }

    #[test]
    fn assigned_primary_opcode_classes_roundtrip() {
        assert_eq!(OpcodeClass::from_u4(0x0), Some(OpcodeClass::Control));
        assert_eq!(OpcodeClass::from_u4(0xA), Some(OpcodeClass::Event));
        assert_eq!(OpcodeClass::from_u4(0xB), None);
        assert_eq!(OpcodeClass::from_u4(0xF), None);
    }
}
