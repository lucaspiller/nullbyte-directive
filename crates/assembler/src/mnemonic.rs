//! Mnemonic resolution derived from emulator opcode tables.

use std::sync::OnceLock;

use emulator_core::{OpcodeEncoding, OPCODE_ENCODING_TABLE};

/// Lookup result for a parsed mnemonic.
pub type MnemonicResolution = (u8, u8, OpcodeEncoding);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MnemonicEntry {
    name: &'static str,
    op: u8,
    sub: u8,
    encoding: OpcodeEncoding,
}

const CALL_MNEMONIC: &str = "CALL";
const RET_MNEMONIC: &str = "RET";

const MNEMONIC_ENTRIES: &[MnemonicEntry] = &[
    MnemonicEntry {
        name: "NOP",
        op: 0x0,
        sub: 0x0,
        encoding: OpcodeEncoding::Nop,
    },
    MnemonicEntry {
        name: "SYNC",
        op: 0x0,
        sub: 0x1,
        encoding: OpcodeEncoding::Sync,
    },
    MnemonicEntry {
        name: "HALT",
        op: 0x0,
        sub: 0x2,
        encoding: OpcodeEncoding::Halt,
    },
    MnemonicEntry {
        name: "TRAP",
        op: 0x0,
        sub: 0x3,
        encoding: OpcodeEncoding::Trap,
    },
    MnemonicEntry {
        name: "SWI",
        op: 0x0,
        sub: 0x4,
        encoding: OpcodeEncoding::Swi,
    },
    MnemonicEntry {
        name: "MOV",
        op: 0x1,
        sub: 0x0,
        encoding: OpcodeEncoding::Mov,
    },
    MnemonicEntry {
        name: "LOAD",
        op: 0x2,
        sub: 0x0,
        encoding: OpcodeEncoding::Load,
    },
    MnemonicEntry {
        name: "STORE",
        op: 0x3,
        sub: 0x0,
        encoding: OpcodeEncoding::Store,
    },
    MnemonicEntry {
        name: "ADD",
        op: 0x4,
        sub: 0x0,
        encoding: OpcodeEncoding::Add,
    },
    MnemonicEntry {
        name: "SUB",
        op: 0x4,
        sub: 0x1,
        encoding: OpcodeEncoding::Sub,
    },
    MnemonicEntry {
        name: "AND",
        op: 0x4,
        sub: 0x2,
        encoding: OpcodeEncoding::And,
    },
    MnemonicEntry {
        name: "OR",
        op: 0x4,
        sub: 0x3,
        encoding: OpcodeEncoding::Or,
    },
    MnemonicEntry {
        name: "XOR",
        op: 0x4,
        sub: 0x4,
        encoding: OpcodeEncoding::Xor,
    },
    MnemonicEntry {
        name: "SHL",
        op: 0x4,
        sub: 0x5,
        encoding: OpcodeEncoding::Shl,
    },
    MnemonicEntry {
        name: "SHR",
        op: 0x4,
        sub: 0x6,
        encoding: OpcodeEncoding::Shr,
    },
    MnemonicEntry {
        name: "CMP",
        op: 0x4,
        sub: 0x7,
        encoding: OpcodeEncoding::Cmp,
    },
    MnemonicEntry {
        name: "MUL",
        op: 0x5,
        sub: 0x0,
        encoding: OpcodeEncoding::Mul,
    },
    MnemonicEntry {
        name: "MULH",
        op: 0x5,
        sub: 0x1,
        encoding: OpcodeEncoding::Mulh,
    },
    MnemonicEntry {
        name: "DIV",
        op: 0x5,
        sub: 0x2,
        encoding: OpcodeEncoding::Div,
    },
    MnemonicEntry {
        name: "MOD",
        op: 0x5,
        sub: 0x3,
        encoding: OpcodeEncoding::Mod,
    },
    MnemonicEntry {
        name: "QADD",
        op: 0x5,
        sub: 0x4,
        encoding: OpcodeEncoding::Qadd,
    },
    MnemonicEntry {
        name: "QSUB",
        op: 0x5,
        sub: 0x5,
        encoding: OpcodeEncoding::Qsub,
    },
    MnemonicEntry {
        name: "SCV",
        op: 0x5,
        sub: 0x6,
        encoding: OpcodeEncoding::Scv,
    },
    MnemonicEntry {
        name: "BEQ",
        op: 0x6,
        sub: 0x0,
        encoding: OpcodeEncoding::Beq,
    },
    MnemonicEntry {
        name: "BNE",
        op: 0x6,
        sub: 0x1,
        encoding: OpcodeEncoding::Bne,
    },
    MnemonicEntry {
        name: "BLT",
        op: 0x6,
        sub: 0x2,
        encoding: OpcodeEncoding::Blt,
    },
    MnemonicEntry {
        name: "BLE",
        op: 0x6,
        sub: 0x3,
        encoding: OpcodeEncoding::Ble,
    },
    MnemonicEntry {
        name: "BGT",
        op: 0x6,
        sub: 0x4,
        encoding: OpcodeEncoding::Bgt,
    },
    MnemonicEntry {
        name: "BGE",
        op: 0x6,
        sub: 0x5,
        encoding: OpcodeEncoding::Bge,
    },
    MnemonicEntry {
        name: "JMP",
        op: 0x6,
        sub: 0x6,
        encoding: OpcodeEncoding::Jmp,
    },
    MnemonicEntry {
        name: CALL_MNEMONIC,
        op: 0x6,
        sub: 0x7,
        encoding: OpcodeEncoding::CallOrRet,
    },
    MnemonicEntry {
        name: RET_MNEMONIC,
        op: 0x6,
        sub: 0x7,
        encoding: OpcodeEncoding::CallOrRet,
    },
    MnemonicEntry {
        name: "PUSH",
        op: 0x7,
        sub: 0x0,
        encoding: OpcodeEncoding::Push,
    },
    MnemonicEntry {
        name: "POP",
        op: 0x7,
        sub: 0x1,
        encoding: OpcodeEncoding::Pop,
    },
    MnemonicEntry {
        name: "IN",
        op: 0x8,
        sub: 0x0,
        encoding: OpcodeEncoding::In,
    },
    MnemonicEntry {
        name: "OUT",
        op: 0x8,
        sub: 0x1,
        encoding: OpcodeEncoding::Out,
    },
    MnemonicEntry {
        name: "BSET",
        op: 0x9,
        sub: 0x0,
        encoding: OpcodeEncoding::Bset,
    },
    MnemonicEntry {
        name: "BCLR",
        op: 0x9,
        sub: 0x1,
        encoding: OpcodeEncoding::Bclr,
    },
    MnemonicEntry {
        name: "BTEST",
        op: 0x9,
        sub: 0x2,
        encoding: OpcodeEncoding::Btest,
    },
    MnemonicEntry {
        name: "EWAIT",
        op: 0xA,
        sub: 0x0,
        encoding: OpcodeEncoding::Ewait,
    },
    MnemonicEntry {
        name: "EGET",
        op: 0xA,
        sub: 0x1,
        encoding: OpcodeEncoding::Eget,
    },
    MnemonicEntry {
        name: "ERET",
        op: 0xA,
        sub: 0x2,
        encoding: OpcodeEncoding::Eret,
    },
];

fn entries_verified_against_core() -> &'static [MnemonicEntry] {
    static VERIFIED_ENTRIES: OnceLock<Vec<MnemonicEntry>> = OnceLock::new();
    VERIFIED_ENTRIES.get_or_init(|| {
        for entry in MNEMONIC_ENTRIES {
            let matches_core = OPCODE_ENCODING_TABLE.iter().any(|(op, sub, encoding)| {
                (*op == entry.op) && (*sub == entry.sub) && (*encoding == entry.encoding)
            });
            assert!(
                matches_core,
                "mnemonic table diverged from emulator-core table"
            );
        }
        MNEMONIC_ENTRIES.to_vec()
    })
}

/// Resolves a mnemonic string to its `(OP, SUB, OpcodeEncoding)` tuple.
///
/// Matching is ASCII case-insensitive.
#[must_use]
pub fn resolve_mnemonic(name: &str) -> Option<MnemonicResolution> {
    entries_verified_against_core()
        .iter()
        .find(|entry| entry.name.eq_ignore_ascii_case(name))
        .map(|entry| (entry.op, entry.sub, entry.encoding))
}

/// Resolves a mnemonic while disambiguating `CALL` and `RET` by operand presence.
///
/// `CALL` requires an operand and `RET` requires no operand.
#[must_use]
pub fn resolve_mnemonic_with_operand_form(
    name: &str,
    has_operand: bool,
) -> Option<MnemonicResolution> {
    if name.eq_ignore_ascii_case(CALL_MNEMONIC) {
        return has_operand.then(|| resolve_mnemonic(name)).flatten();
    }
    if name.eq_ignore_ascii_case(RET_MNEMONIC) {
        return (!has_operand).then(|| resolve_mnemonic(name)).flatten();
    }
    resolve_mnemonic(name)
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use emulator_core::{OpcodeEncoding, OPCODE_ENCODING_TABLE};

    use super::{
        resolve_mnemonic, resolve_mnemonic_with_operand_form, MnemonicEntry, MnemonicResolution,
        MNEMONIC_ENTRIES,
    };

    fn expected_resolution(entry: &MnemonicEntry) -> MnemonicResolution {
        (entry.op, entry.sub, entry.encoding)
    }

    #[test]
    fn every_mnemonic_resolves_to_expected_op_sub_encoding() {
        for entry in MNEMONIC_ENTRIES {
            assert_eq!(
                resolve_mnemonic(entry.name),
                Some(expected_resolution(entry))
            );
        }
    }

    #[test]
    fn lookup_is_case_insensitive() {
        assert_eq!(
            resolve_mnemonic("add"),
            Some((0x4, 0x0, OpcodeEncoding::Add))
        );
        assert_eq!(
            resolve_mnemonic("qSuB"),
            Some((0x5, 0x5, OpcodeEncoding::Qsub))
        );
    }

    #[test]
    fn unknown_mnemonic_returns_none() {
        assert_eq!(resolve_mnemonic("NOTAREALOP"), None);
        assert_eq!(resolve_mnemonic(""), None);
    }

    #[test]
    fn call_and_ret_are_disambiguated_by_operand_presence() {
        assert_eq!(
            resolve_mnemonic_with_operand_form("CALL", true),
            Some((0x6, 0x7, OpcodeEncoding::CallOrRet))
        );
        assert_eq!(resolve_mnemonic_with_operand_form("CALL", false), None);

        assert_eq!(
            resolve_mnemonic_with_operand_form("RET", false),
            Some((0x6, 0x7, OpcodeEncoding::CallOrRet))
        );
        assert_eq!(resolve_mnemonic_with_operand_form("RET", true), None);
    }

    #[test]
    fn mnemonic_table_covers_all_opcode_encodings() {
        let encoded_variants: HashSet<_> = MNEMONIC_ENTRIES
            .iter()
            .map(|entry| entry.encoding)
            .collect();
        let core_variants: HashSet<_> = OPCODE_ENCODING_TABLE
            .iter()
            .map(|(_, _, encoding)| *encoding)
            .collect();

        assert_eq!(core_variants.len(), 41);
        assert_eq!(encoded_variants.len(), core_variants.len());
        assert_eq!(encoded_variants, core_variants);
    }
}
