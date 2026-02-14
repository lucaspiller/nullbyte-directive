//! Instruction disassembly for the Nullbyte One ISA.
//!
//! This module provides utilities for converting raw instruction bytes into
//! human-readable assembly format.

use crate::decoder::{AddressingMode, Decoder, RegisterField};
use crate::encoding::OpcodeEncoding;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// A single disassembled instruction row.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct DisassemblyRow {
    /// The starting address of this instruction.
    pub addr_start: u16,
    /// Length in bytes (2 for single-word, 4 for two-word instructions).
    pub len_bytes: u8,
    /// Raw instruction words (primary word in low bits, extension in high bits if present).
    pub raw_words: u32,
    /// The instruction mnemonic (e.g., "ADD", "MOV", "NOP").
    pub mnemonic: String,
    /// The formatted operands (e.g., "R0, R1" or "#0x1234").
    pub operands: String,
    /// Whether this instruction is an illegal encoding.
    pub is_illegal: bool,
}

/// Disassembles a window of instructions around a given program counter.
///
/// This function reads instructions from memory starting at `center_pc` and
/// produces `before` instructions before and `after` instructions after the center.
/// If memory cannot be read at a given address (e.g., outside address space bounds),
/// that row is omitted from the output.
///
/// The function handles:
/// - Single-word instructions (2 bytes)
/// - Two-word instructions with extension words (4 bytes)
/// - Illegal encodings (displayed as `.word 0xXXXX ; ILLEGAL`)
/// - Special case for `CALL` vs `RET` based on addressing mode
///
/// Note: `after` specifies the number of instructions AFTER the center, not including center.
#[must_use]
pub fn disassemble_window(
    center_pc: u16,
    before: usize,
    after: usize,
    memory: &[u8],
) -> Vec<DisassemblyRow> {
    let target_total = before + 1 + after;
    let mut rows = Vec::with_capacity(target_total);

    // Collect forward instructions first
    let mut pc = center_pc;
    let mut forward_rows: Vec<DisassemblyRow> = Vec::new();

    // First get the center instruction
    if let Some(row) = disassemble_one(pc, memory) {
        let len = row.len_bytes;
        forward_rows.push(row);
        pc = pc.wrapping_add(u16::from(len));
    }

    // Then get more forward instructions up to after
    for _ in 0..after {
        if let Some(row) = disassemble_one(pc, memory) {
            let len = row.len_bytes;
            forward_rows.push(row);
            pc = pc.wrapping_add(u16::from(len));
        } else {
            break;
        }
    }

    // Now try to get backward instructions to fill in
    if before > 0 {
        let mut found_before: Vec<DisassemblyRow> = Vec::new();
        let mut scan_pc = center_pc;

        while scan_pc > 0 && found_before.len() < before {
            let mut found_one = false;

            for len in [4u8, 2u8].iter().copied() {
                if scan_pc < u16::from(len) {
                    continue;
                }
                let try_pc = scan_pc.wrapping_sub(u16::from(len));
                if let Some(row) = disassemble_one(try_pc, memory) {
                    let instr_end = row.addr_start.wrapping_add(u16::from(row.len_bytes));
                    if instr_end == scan_pc && row.len_bytes == len {
                        found_before.push(row);
                        scan_pc = try_pc;
                        found_one = true;
                        break;
                    }
                }
            }

            if !found_one {
                scan_pc = scan_pc.wrapping_sub(1);
            }

            if found_before.len() >= before {
                break;
            }
        }

        found_before.reverse();

        // If we couldn't find enough backward, that's okay - forward already has center + after
        // Just use what we found before (may be fewer than requested)
        rows.extend(found_before);
    }

    // Add forward rows (center + after)
    rows.extend(forward_rows);

    // If we still don't have enough, try to get more forward instructions
    if rows.len() < target_total {
        let mut pc = if let Some(last) = rows.last() {
            last.addr_start.wrapping_add(u16::from(last.len_bytes))
        } else {
            center_pc
        };

        while rows.len() < target_total {
            if let Some(row) = disassemble_one(pc, memory) {
                let len = row.len_bytes;
                rows.push(row);
                pc = pc.wrapping_add(u16::from(len));
            } else {
                break;
            }
        }
    }

    rows
}

fn disassemble_one(pc: u16, memory: &[u8]) -> Option<DisassemblyRow> {
    let lo = *memory.get(usize::from(pc))?;
    let hi = *memory.get(usize::from(pc.wrapping_add(1)))?;
    let raw_word = u16::from_be_bytes([lo, hi]);

    let decoded = Decoder::decode(raw_word);

    match decoded {
        crate::decoder::DecodedOrFault::Fault(_) => Some(DisassemblyRow {
            addr_start: pc,
            len_bytes: 2,
            raw_words: u32::from(raw_word),
            mnemonic: ".word".to_string(),
            operands: format!("0x{raw_word:04X} ; ILLEGAL"),
            is_illegal: true,
        }),
        crate::decoder::DecodedOrFault::Instruction(instr) => {
            let mut decoded = instr;
            let mut raw_words = u32::from(raw_word);
            let len_bytes = if decoded
                .addressing_mode
                .is_some_and(AddressingMode::requires_extension_word)
            {
                let ext_pc = pc.wrapping_add(2);
                let ext_lo = *memory.get(usize::from(ext_pc))?;
                let ext_hi = *memory.get(usize::from(ext_pc.wrapping_add(1)))?;
                let extension_word = u16::from_be_bytes([ext_lo, ext_hi]);
                decoded.immediate_value = Some(extension_word);
                raw_words |= (u32::from(extension_word)) << 16;
                4
            } else {
                2
            };

            let mnemonic = format_mnemonic(decoded.encoding, decoded.addressing_mode);
            let operands = format_operands(&decoded);

            Some(DisassemblyRow {
                addr_start: pc,
                len_bytes,
                raw_words,
                mnemonic,
                operands,
                is_illegal: false,
            })
        }
    }
}

fn format_mnemonic(encoding: OpcodeEncoding, addressing_mode: Option<AddressingMode>) -> String {
    if encoding == OpcodeEncoding::CallOrRet {
        if addressing_mode == Some(AddressingMode::DirectRegister) {
            return "RET".to_string();
        }
        return "CALL".to_string();
    }

    let name = match encoding {
        OpcodeEncoding::Nop => "NOP",
        OpcodeEncoding::Sync => "SYNC",
        OpcodeEncoding::Halt => "HALT",
        OpcodeEncoding::Trap => "TRAP",
        OpcodeEncoding::Swi => "SWI",
        OpcodeEncoding::Mov => "MOV",
        OpcodeEncoding::Load => "LOAD",
        OpcodeEncoding::Store => "STORE",
        OpcodeEncoding::Add => "ADD",
        OpcodeEncoding::Sub => "SUB",
        OpcodeEncoding::And => "AND",
        OpcodeEncoding::Or => "OR",
        OpcodeEncoding::Xor => "XOR",
        OpcodeEncoding::Shl => "SHL",
        OpcodeEncoding::Shr => "SHR",
        OpcodeEncoding::Cmp => "CMP",
        OpcodeEncoding::Mul => "MUL",
        OpcodeEncoding::Mulh => "MULH",
        OpcodeEncoding::Div => "DIV",
        OpcodeEncoding::Mod => "MOD",
        OpcodeEncoding::Qadd => "QADD",
        OpcodeEncoding::Qsub => "QSUB",
        OpcodeEncoding::Scv => "SCV",
        OpcodeEncoding::Beq => "BEQ",
        OpcodeEncoding::Bne => "BNE",
        OpcodeEncoding::Blt => "BLT",
        OpcodeEncoding::Ble => "BLE",
        OpcodeEncoding::Bgt => "BGT",
        OpcodeEncoding::Bge => "BGE",
        OpcodeEncoding::Jmp => "JMP",
        OpcodeEncoding::CallOrRet => unreachable!(),
        OpcodeEncoding::Push => "PUSH",
        OpcodeEncoding::Pop => "POP",
        OpcodeEncoding::In => "IN",
        OpcodeEncoding::Out => "OUT",
        OpcodeEncoding::Bset => "BSET",
        OpcodeEncoding::Bclr => "BCLR",
        OpcodeEncoding::Btest => "BTEST",
        OpcodeEncoding::Ewait => "EWAIT",
        OpcodeEncoding::Eget => "EGET",
        OpcodeEncoding::Eret => "ERET",
    };

    name.to_string()
}

#[allow(clippy::too_many_lines)]
fn format_operands(instr: &crate::decoder::DecodedInstruction) -> String {
    let Some(am) = instr.addressing_mode else {
        return String::new();
    };

    let no_operand_encoding = matches!(
        instr.encoding,
        OpcodeEncoding::Nop
            | OpcodeEncoding::Sync
            | OpcodeEncoding::Halt
            | OpcodeEncoding::Trap
            | OpcodeEncoding::Swi
            | OpcodeEncoding::Eret
    );
    if no_operand_encoding {
        return String::new();
    }

    let is_jump = matches!(
        instr.encoding,
        OpcodeEncoding::Jmp
            | OpcodeEncoding::Beq
            | OpcodeEncoding::Bne
            | OpcodeEncoding::Blt
            | OpcodeEncoding::Ble
            | OpcodeEncoding::Bgt
            | OpcodeEncoding::Bge
    );

    let rd = instr.rd.map(format_register);
    let ra = instr.ra.map(format_register);
    let rb = instr.rb.map(format_register);

    let is_alu_op = matches!(
        instr.encoding,
        OpcodeEncoding::Add
            | OpcodeEncoding::Sub
            | OpcodeEncoding::And
            | OpcodeEncoding::Or
            | OpcodeEncoding::Xor
            | OpcodeEncoding::Shl
            | OpcodeEncoding::Shr
            | OpcodeEncoding::Cmp
            | OpcodeEncoding::Mul
            | OpcodeEncoding::Mulh
            | OpcodeEncoding::Div
            | OpcodeEncoding::Mod
            | OpcodeEncoding::Qadd
            | OpcodeEncoding::Qsub
            | OpcodeEncoding::Scv
    );

    match am {
        #[allow(clippy::match_same_arms)]
        AddressingMode::DirectRegister => match instr.encoding {
            OpcodeEncoding::Push | OpcodeEncoding::Pop => rd.unwrap_or_default(),
            OpcodeEncoding::CallOrRet => rb.unwrap_or_default(),
            OpcodeEncoding::In => match (&rd, &ra) {
                (Some(d), Some(s)) => format!("{d}, {s}"),
                (_, Some(s)) => s.clone(),
                _ => String::new(),
            },
            _ => {
                if is_alu_op {
                    match (&rd, &ra, &rb) {
                        (Some(d), Some(a), Some(b)) => format!("{d}, {a}, {b}"),
                        (Some(d), Some(a), _) => format!("{d}, {a}"),
                        (Some(d), _, Some(b)) => format!("{d}, {b}"),
                        (Some(d), _, _) => d.clone(),
                        _ => String::new(),
                    }
                } else {
                    match (&rd, &ra) {
                        (Some(d), Some(a)) => format!("{d}, {a}"),
                        (Some(d), _) => d.clone(),
                        _ => String::new(),
                    }
                }
            }
        },
        AddressingMode::IndirectRegister => match (&rd, &ra) {
            (Some(d), Some(a)) => format!("{d}, [{a}]"),
            (_, Some(a)) => format!("[{a}]"),
            _ => String::new(),
        },
        AddressingMode::IndirectAutoIncrement => {
            let imm = instr.immediate_value.unwrap_or(0);
            match (&rd, &ra) {
                (Some(d), Some(a)) => format!("{d}, [{a}]+{imm}"),
                (_, Some(a)) => format!("[{a}]+{imm}"),
                _ => String::new(),
            }
        }
        AddressingMode::SignExtendedDisplacement => {
            let imm = instr.immediate_value.unwrap_or(0);
            let disp = i16::from_be_bytes([(imm >> 8) as u8, u8::try_from(imm).unwrap_or(0)]);
            if is_jump {
                format!("{disp:+}")
            } else {
                match (&rd, &ra) {
                    (Some(d), Some(a)) => format!("{d}, [{a} {disp:+}]"),
                    (_, Some(a)) => format!("[{a} {disp:+}]"),
                    (Some(d), _) => format!("{d}, 0x{imm:04X}"),
                    _ => format!("0x{imm:04X}"),
                }
            }
        }
        AddressingMode::ZeroExtendedDisplacement => {
            let imm = instr.immediate_value.unwrap_or(0);
            match (&rd, &ra) {
                (Some(d), Some(a)) => format!("{d}, [{a} + 0x{imm:02X}]"),
                (_, Some(a)) => format!("[{a} + 0x{imm:02X}]"),
                (Some(d), _) => format!("{d}, 0x{imm:04X}"),
                _ => format!("0x{imm:04X}"),
            }
        }
        AddressingMode::Immediate => {
            let imm = instr.immediate_value.unwrap_or(0);
            if is_jump {
                format!("#0x{imm:04X}")
            } else {
                rd.as_ref()
                    .map_or_else(|| format!("#0x{imm:04X}"), |d| format!("{d}, #0x{imm:04X}"))
            }
        }
        AddressingMode::Reserved110 | AddressingMode::Reserved111 => String::new(),
    }
}

fn format_register(r: RegisterField) -> String {
    match r {
        RegisterField::R0 => "R0".to_string(),
        RegisterField::R1 => "R1".to_string(),
        RegisterField::R2 => "R2".to_string(),
        RegisterField::R3 => "R3".to_string(),
        RegisterField::R4 => "R4".to_string(),
        RegisterField::R5 => "R5".to_string(),
        RegisterField::R6 => "R6".to_string(),
        RegisterField::R7 => "R7".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disassemble_nop() {
        let memory = [0x00, 0x00, 0x00, 0x00];
        let rows = disassemble_window(0, 0, 0, &memory);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].mnemonic, "NOP");
        assert_eq!(rows[0].operands, "");
        assert!(!rows[0].is_illegal);
    }

    #[test]
    fn disassemble_mov_register() {
        let memory = [0x10, 0x00, 0x00, 0x00];
        let rows = disassemble_window(0, 0, 0, &memory);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].mnemonic, "MOV");
        assert_eq!(rows[0].operands, "R0, R0");
    }

    #[test]
    fn disassemble_halt() {
        let memory = [0x00, 0x10, 0x00, 0x00];
        let rows = disassemble_window(0, 0, 0, &memory);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].mnemonic, "HALT");
    }

    #[test]
    fn disassemble_illegal() {
        let memory = [0xF0, 0x00, 0x00, 0x00];
        let rows = disassemble_window(0, 0, 0, &memory);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].mnemonic, ".word");
        assert!(rows[0].is_illegal);
    }

    #[test]
    fn disassemble_call_ret_direct_register() {
        let memory = [0x60, 0x38, 0x00, 0x00];
        let rows = disassemble_window(0, 0, 0, &memory);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].mnemonic, "RET");
        assert_eq!(rows[0].operands, "R7");
    }

    #[test]
    fn disassemble_call_immediate() {
        let memory = [0x60, 0x3D, 0x34, 0x12];
        let rows = disassemble_window(0, 0, 0, &memory);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].mnemonic, "CALL");
        assert!(rows[0].operands.contains("0x"));
    }

    #[test]
    fn disassemble_mov_immediate_correct_value() {
        let memory = [0x12, 0x05, 0x40, 0x00];
        let rows = disassemble_window(0, 0, 0, &memory);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].mnemonic, "MOV");
        assert_eq!(rows[0].operands, "R1, #0x4000");
        assert_eq!(rows[0].len_bytes, 4);
    }

    #[test]
    fn disassemble_xor_three_operands() {
        let memory = [0x46, 0xE0, 0x00, 0x00];
        let rows = disassemble_window(0, 0, 0, &memory);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].mnemonic, "XOR");
        assert_eq!(rows[0].operands, "R3, R3, R4");
    }

    #[test]
    fn disassemble_store_indirect() {
        let memory = [0x36, 0x41, 0x00, 0x00];
        let rows = disassemble_window(0, 0, 0, &memory);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].mnemonic, "STORE");
        assert_eq!(rows[0].operands, "R3, [R1]");
    }

    #[test]
    fn disassemble_jmp_immediate() {
        let memory = [0x60, 0x35, 0xFF, 0xF6];
        let rows = disassemble_window(0, 0, 0, &memory);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].mnemonic, "JMP");
        assert_eq!(rows[0].operands, "#0xFFF6");
        assert_eq!(rows[0].len_bytes, 4);
    }

    #[test]
    fn disassemble_blinker_program() {
        let memory = [
            0x12, 0x05, 0x40, 0x00, // MOV R1, #0x4000
            0x18, 0x05, 0x00, 0xFF, // MOV R4, #0x00FF
            0x16, 0x05, 0x00, 0x00, // MOV R3, #0x0000
            0x00, 0x10, // HALT
            0x46, 0xE0, // XOR R3, R3, R4
            0x36, 0x41, // STORE R3, [R1]
            0x00, 0x10, // HALT
            0x60, 0x35, 0xFF, 0xF6, // JMP #-10 (PC-relative)
        ];
        let rows = disassemble_window(0, 0, 8, &memory);
        assert_eq!(rows.len(), 8);
        assert_eq!(rows[0].addr_start, 0);
        assert_eq!(rows[0].mnemonic, "MOV");
        assert_eq!(rows[0].operands, "R1, #0x4000");
        assert_eq!(rows[1].addr_start, 4);
        assert_eq!(rows[1].mnemonic, "MOV");
        assert_eq!(rows[1].operands, "R4, #0x00FF");
        assert_eq!(rows[2].addr_start, 8);
        assert_eq!(rows[2].mnemonic, "MOV");
        assert_eq!(rows[2].operands, "R3, #0x0000");
        assert_eq!(rows[3].addr_start, 12);
        assert_eq!(rows[3].mnemonic, "HALT");
        assert_eq!(rows[4].addr_start, 14);
        assert_eq!(rows[4].mnemonic, "XOR");
        assert_eq!(rows[4].operands, "R3, R3, R4");
        assert_eq!(rows[5].addr_start, 16);
        assert_eq!(rows[5].mnemonic, "STORE");
        assert_eq!(rows[5].operands, "R3, [R1]");
        assert_eq!(rows[6].addr_start, 18);
        assert_eq!(rows[6].mnemonic, "HALT");
        assert_eq!(rows[7].addr_start, 20);
        assert_eq!(rows[7].mnemonic, "JMP");
    }

    #[test]
    fn disassemble_window_before_after() {
        let memory = [0x00, 0x00, 0x00, 0x10, 0x00, 0x00];
        let rows = disassemble_window(2, 1, 1, &memory);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].addr_start, 0);
        assert_eq!(rows[1].addr_start, 2);
        assert_eq!(rows[2].addr_start, 4);
    }
}
