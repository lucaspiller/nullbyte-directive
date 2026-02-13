//! Helper functions for instruction execution.

#![allow(clippy::pedantic, clippy::nursery, unknown_lints, missing_docs)]

use crate::decoder::{AddressingMode, DecodedInstruction};
use crate::state::GeneralRegister;

/// Computes the effective address based on the addressing mode and registers.
#[must_use]
pub fn compute_effective_address(
    instr: &DecodedInstruction,
    state: &crate::CoreState,
) -> Option<u16> {
    let am = instr.addressing_mode?;

    match am {
        AddressingMode::DirectRegister
        | AddressingMode::IndirectRegister
        | AddressingMode::IndirectAutoIncrement => {
            let reg = instr.ra?;
            let reg = decoder_register_to_general(reg);
            Some(state.arch.gpr(reg))
        }
        AddressingMode::SignExtendedDisplacement => {
            let base = read_register_opt(instr.ra, state);
            let disp = sign_extend_6bit(instr.immediate_value?);
            base.map(|b| b.wrapping_add(disp))
        }
        AddressingMode::ZeroExtendedDisplacement => {
            let base = read_register_opt(instr.ra, state);
            let disp = instr.immediate_value? & 0x3F;
            base.map(|b| b.wrapping_add(disp))
        }
        AddressingMode::Immediate => Some(instr.immediate_value?),
        AddressingMode::Reserved110 | AddressingMode::Reserved111 => None,
    }
}

/// Computes the effective address with PC-relative addressing for branches.
#[must_use]
pub fn compute_effective_address_with_pc(
    instr: &DecodedInstruction,
    state: &crate::CoreState,
) -> Option<u16> {
    let am = instr.addressing_mode?;

    let base = match am {
        AddressingMode::DirectRegister => {
            let reg = instr.ra?;
            let reg = decoder_register_to_general(reg);
            state.arch.gpr(reg)
        }
        AddressingMode::SignExtendedDisplacement => {
            let pc = state.arch.pc();
            let disp = sign_extend_6bit(instr.immediate_value?);
            pc.wrapping_add(disp)
        }
        AddressingMode::ZeroExtendedDisplacement => {
            let pc = state.arch.pc();
            let disp = instr.immediate_value? & 0x3F;
            pc.wrapping_add(disp)
        }
        _ => return None,
    };

    Some(base)
}

fn read_register_opt(
    field: Option<crate::decoder::RegisterField>,
    state: &crate::CoreState,
) -> Option<u16> {
    field.map(|f| state.arch.gpr(decoder_register_to_general(f)))
}

const fn decoder_register_to_general(field: crate::decoder::RegisterField) -> GeneralRegister {
    match field {
        crate::decoder::RegisterField::R0 => GeneralRegister::R0,
        crate::decoder::RegisterField::R1 => GeneralRegister::R1,
        crate::decoder::RegisterField::R2 => GeneralRegister::R2,
        crate::decoder::RegisterField::R3 => GeneralRegister::R3,
        crate::decoder::RegisterField::R4 => GeneralRegister::R4,
        crate::decoder::RegisterField::R5 => GeneralRegister::R5,
        crate::decoder::RegisterField::R6 => GeneralRegister::R6,
        crate::decoder::RegisterField::R7 => GeneralRegister::R7,
    }
}

const fn sign_extend_6bit(value: u16) -> u16 {
    if (value & 0x20) != 0 {
        value | 0xFFC0
    } else {
        value & 0x003F
    }
}
