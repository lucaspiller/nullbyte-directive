//! Instruction execution pipeline for Nullbyte One ISA.
//!
//! This module implements the 7-step commit sequence from the spec:
//! 1. Read source operands
//! 2. Compute result and/or effective address
//! 3. Perform memory/MMIO reads
//! 4. Perform memory/MMIO writes
//! 5. Write destination register
//! 6. Update FLAGS
//! 7. Advance PC
//!
//! All operations must be precise: faulting instructions produce no partial side effects.

#![allow(
    clippy::pedantic,
    clippy::nursery,
    clippy::similar_names,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::struct_excessive_bools,
    unknown_lints,
    missing_docs
)]

mod flags;
mod helpers;

pub use flags::FlagsUpdate;
pub use helpers::{compute_effective_address, compute_effective_address_with_pc};

use crate::decoder::{AddressingMode, DecodedInstruction, RegisterField};
use crate::encoding::OpcodeEncoding;
use crate::state::registers::FLAGS_ACTIVE_MASK;
use crate::timing::CycleCostKind;
use crate::{CoreConfig, CoreState, Decoder, GeneralRegister, MmioBus, RunState, StepOutcome};

/// Outcome of executing a single instruction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecuteOutcome {
    /// Instruction retired successfully with cycle cost.
    Retired {
        /// Fixed cycle cost consumed.
        cycles: u16,
    },
    /// Core should halt for remainder of tick.
    HaltedForTick,
    /// Trap dispatch triggered.
    TrapDispatch {
        /// Trap cause value.
        cause: u16,
    },
    /// Event dispatch triggered.
    EventDispatch {
        /// Dequeued event ID.
        event_id: u8,
    },
    /// Fault was raised during execution.
    Fault {
        /// Fault code.
        cause: crate::fault::FaultCode,
    },
}

/// Execution state that accumulates side effects during the 7-step commit sequence.
/// This ensures precise faults: no side effects are committed until the sequence completes.
#[allow(clippy::struct_excessive_bools)]
pub struct ExecuteState {
    /// Value read from register during operand phase.
    pub operand_a: Option<u16>,
    /// Value read from register during operand phase.
    pub operand_b: Option<u16>,
    /// Computed result or effective address.
    pub computed_value: Option<u16>,
    /// Value read from memory/MMIO.
    pub memory_read_value: Option<u16>,
    /// Whether a memory write should occur.
    pub memory_write_pending: bool,
    /// Value to write to memory/MMIO.
    pub memory_write_value: Option<u16>,
    /// Address for memory operation.
    pub memory_addr: Option<u16>,
    /// Whether this is an MMIO operation.
    pub is_mmio_operation: bool,
    /// Whether this is an MMIO write.
    pub is_mmio_write: bool,
    /// Destination register for result.
    pub dest_reg: Option<RegisterField>,
    /// Value to write to destination register.
    pub dest_value: Option<u16>,
    /// FLAGS update to apply.
    pub flags_update: FlagsUpdate,
    /// New PC value to set after commit.
    pub next_pc: Option<u16>,
    /// Cycle cost for this instruction.
    pub cycles: u16,
    /// Whether a trap was triggered.
    pub trap_pending: bool,
    /// Trap cause value.
    pub trap_cause: Option<u16>,
    /// Whether an event dispatch is pending.
    pub event_dispatch_pending: bool,
    /// Event ID to dispatch.
    pub event_id: Option<u8>,
    /// Whether execution should halt for tick.
    pub halt_for_tick: bool,
}

impl Default for ExecuteState {
    fn default() -> Self {
        Self {
            operand_a: None,
            operand_b: None,
            computed_value: None,
            memory_read_value: None,
            memory_write_pending: false,
            memory_write_value: None,
            memory_addr: None,
            is_mmio_operation: false,
            is_mmio_write: false,
            dest_reg: None,
            dest_value: None,
            flags_update: FlagsUpdate::None,
            next_pc: None,
            cycles: 0,
            trap_pending: false,
            trap_cause: None,
            event_dispatch_pending: false,
            event_id: None,
            halt_for_tick: false,
        }
    }
}

impl ExecuteState {
    /// Creates a new execute state with the given cycle cost.
    #[must_use]
    pub fn new(cycles: u16) -> Self {
        Self {
            cycles,
            ..Self::default()
        }
    }
}

/// Executes a single instruction following the 7-step commit sequence.
///
/// Returns both the execution outcome and the execution state. On success, the caller
/// must apply the committed side effects to the core state. On fault, no side effects
/// should be applied (precise fault semantics).
#[allow(clippy::too_many_lines)]
pub fn execute_instruction(
    instr: &DecodedInstruction,
    state: &mut CoreState,
    mmio: &mut dyn MmioBus,
) -> (ExecuteOutcome, ExecuteState) {
    let pc = state.arch.pc();
    let next_pc = pc.wrapping_add(2);

    let mut exec = ExecuteState::default();

    match instr.encoding {
        OpcodeEncoding::Nop => execute_nop(&mut exec, next_pc),
        OpcodeEncoding::Sync => execute_sync(&mut exec, next_pc),
        OpcodeEncoding::Halt => execute_halt(&mut exec, next_pc),
        OpcodeEncoding::Trap => execute_trap(&mut exec, next_pc),
        OpcodeEncoding::Swi => execute_swi(&mut exec, next_pc),
        OpcodeEncoding::Mov => execute_mov(instr, state, &mut exec, next_pc),
        OpcodeEncoding::Load => execute_load(instr, state, mmio, &mut exec, next_pc),
        OpcodeEncoding::Store => execute_store(instr, state, mmio, &mut exec, next_pc),
        OpcodeEncoding::Add => execute_alu(instr, state, &mut exec, next_pc, AluOp::Add),
        OpcodeEncoding::Sub => execute_alu(instr, state, &mut exec, next_pc, AluOp::Sub),
        OpcodeEncoding::And => execute_alu(instr, state, &mut exec, next_pc, AluOp::And),
        OpcodeEncoding::Or => execute_alu(instr, state, &mut exec, next_pc, AluOp::Or),
        OpcodeEncoding::Xor => execute_alu(instr, state, &mut exec, next_pc, AluOp::Xor),
        OpcodeEncoding::Shl => execute_alu(instr, state, &mut exec, next_pc, AluOp::Shl),
        OpcodeEncoding::Shr => execute_alu(instr, state, &mut exec, next_pc, AluOp::Shr),
        OpcodeEncoding::Cmp => execute_cmp(instr, state, &mut exec, next_pc),
        OpcodeEncoding::Mul => execute_math(instr, state, &mut exec, next_pc, MathOp::Mul),
        OpcodeEncoding::Mulh => execute_math(instr, state, &mut exec, next_pc, MathOp::Mulh),
        OpcodeEncoding::Div => execute_math(instr, state, &mut exec, next_pc, MathOp::Div),
        OpcodeEncoding::Mod => execute_math(instr, state, &mut exec, next_pc, MathOp::Mod),
        OpcodeEncoding::Qadd => execute_math(instr, state, &mut exec, next_pc, MathOp::Qadd),
        OpcodeEncoding::Qsub => execute_math(instr, state, &mut exec, next_pc, MathOp::Qsub),
        OpcodeEncoding::Scv => execute_math(instr, state, &mut exec, next_pc, MathOp::Scv),
        OpcodeEncoding::Beq => execute_branch(instr, state, &mut exec, next_pc, BranchOp::Eq),
        OpcodeEncoding::Bne => execute_branch(instr, state, &mut exec, next_pc, BranchOp::Ne),
        OpcodeEncoding::Blt => execute_branch(instr, state, &mut exec, next_pc, BranchOp::Lt),
        OpcodeEncoding::Ble => execute_branch(instr, state, &mut exec, next_pc, BranchOp::Le),
        OpcodeEncoding::Bgt => execute_branch(instr, state, &mut exec, next_pc, BranchOp::Gt),
        OpcodeEncoding::Bge => execute_branch(instr, state, &mut exec, next_pc, BranchOp::Ge),
        OpcodeEncoding::Jmp => execute_jmp(instr, state, &mut exec, next_pc),
        OpcodeEncoding::CallOrRet => execute_call_or_ret(instr, state, &mut exec, next_pc),
        OpcodeEncoding::Push => execute_push(instr, state, &mut exec, next_pc),
        OpcodeEncoding::Pop => execute_pop(instr, state, &mut exec, next_pc),
        OpcodeEncoding::In => execute_mmio_in(instr, state, mmio, &mut exec, next_pc),
        OpcodeEncoding::Out => execute_mmio_out(instr, state, mmio, &mut exec, next_pc),
        OpcodeEncoding::Bset | OpcodeEncoding::Bclr | OpcodeEncoding::Btest => {
            execute_bitop(instr, state, mmio, &mut exec, next_pc)
        }
        OpcodeEncoding::Ewait => execute_ewait(instr, state, &mut exec, next_pc),
        OpcodeEncoding::Eget => execute_eget(instr, state, &mut exec, next_pc),
        OpcodeEncoding::Eret => execute_eret(instr, state, &mut exec, next_pc),
    }

    if exec.trap_pending {
        return (
            ExecuteOutcome::TrapDispatch {
                cause: exec.trap_cause.unwrap_or(0),
            },
            exec,
        );
    }

    if exec.event_dispatch_pending {
        return (
            ExecuteOutcome::EventDispatch {
                event_id: exec.event_id.unwrap_or(0),
            },
            exec,
        );
    }

    if exec.halt_for_tick {
        return (ExecuteOutcome::HaltedForTick, exec);
    }

    (
        ExecuteOutcome::Retired {
            cycles: exec.cycles,
        },
        exec,
    )
}

/// Applies the committed side effects from execution to the core state.
/// This should only be called after a successful `ExecuteOutcome::Retired`.
pub fn commit_execution(state: &mut CoreState, exec: &ExecuteState) {
    if let Some(pc) = exec.next_pc {
        state.arch.set_pc(pc);
    }

    if let Some(dest) = exec.dest_reg {
        if let Some(value) = exec.dest_value {
            let reg = decoder_register_to_general(dest);
            state.arch.set_gpr(reg, value);
        }
    }

    match exec.flags_update {
        FlagsUpdate::None => {}
        FlagsUpdate::Clear => {
            state.arch.set_flags(0);
        }
        FlagsUpdate::Set(flags) => {
            state.arch.set_flags(flags & FLAGS_ACTIVE_MASK);
        }
        FlagsUpdate::UpdateNZ {
            zero,
            negative,
            carry,
            overflow,
        } => {
            let mut new_flags = state.arch.flags();
            new_flags = if zero {
                new_flags | 0x01
            } else {
                new_flags & !0x01
            };
            new_flags = if negative {
                new_flags | 0x02
            } else {
                new_flags & !0x02
            };
            new_flags = if carry {
                new_flags | 0x04
            } else {
                new_flags & !0x04
            };
            new_flags = if overflow {
                new_flags | 0x08
            } else {
                new_flags & !0x08
            };
            state.arch.set_flags(new_flags);
        }
    }

    state
        .arch
        .set_tick(state.arch.tick().wrapping_add(exec.cycles));
}

const fn decoder_register_to_general(field: RegisterField) -> GeneralRegister {
    match field {
        RegisterField::R0 => GeneralRegister::R0,
        RegisterField::R1 => GeneralRegister::R1,
        RegisterField::R2 => GeneralRegister::R2,
        RegisterField::R3 => GeneralRegister::R3,
        RegisterField::R4 => GeneralRegister::R4,
        RegisterField::R5 => GeneralRegister::R5,
        RegisterField::R6 => GeneralRegister::R6,
        RegisterField::R7 => GeneralRegister::R7,
    }
}

fn read_register(state: &CoreState, field: Option<RegisterField>) -> Option<u16> {
    field.map(|f| state.arch.gpr(decoder_register_to_general(f)))
}

fn execute_nop(exec: &mut ExecuteState, next_pc: u16) {
    exec.cycles = crate::timing::cycle_cost(CycleCostKind::Nop).unwrap_or(1);
    exec.next_pc = Some(next_pc);
    exec.flags_update = FlagsUpdate::None;
}

fn execute_sync(exec: &mut ExecuteState, next_pc: u16) {
    exec.cycles = crate::timing::cycle_cost(CycleCostKind::Sync).unwrap_or(1);
    exec.next_pc = Some(next_pc);
    exec.flags_update = FlagsUpdate::None;
}

fn execute_halt(exec: &mut ExecuteState, next_pc: u16) {
    exec.cycles = crate::timing::cycle_cost(CycleCostKind::Halt).unwrap_or(1);
    exec.next_pc = Some(next_pc);
    exec.halt_for_tick = true;
    exec.flags_update = FlagsUpdate::None;
}

fn execute_trap(exec: &mut ExecuteState, next_pc: u16) {
    exec.cycles = crate::timing::cycle_cost(CycleCostKind::TrapIssue).unwrap_or(1);
    exec.next_pc = Some(next_pc);
    exec.trap_pending = true;
    exec.trap_cause = Some(0);
    exec.flags_update = FlagsUpdate::None;
}

fn execute_swi(exec: &mut ExecuteState, next_pc: u16) {
    exec.cycles = crate::timing::cycle_cost(CycleCostKind::SwiIssue).unwrap_or(1);
    exec.next_pc = Some(next_pc);
    exec.trap_pending = true;
    exec.trap_cause = Some(0);
    exec.flags_update = FlagsUpdate::None;
}

fn execute_mov(
    instr: &DecodedInstruction,
    state: &CoreState,
    exec: &mut ExecuteState,
    next_pc: u16,
) {
    exec.cycles = crate::timing::cycle_cost(CycleCostKind::Mov).unwrap_or(1);
    exec.next_pc = Some(next_pc);

    let Some(rd) = instr.rd else {
        exec.flags_update = FlagsUpdate::None;
        return;
    };

    let value = match instr.addressing_mode {
        Some(AddressingMode::DirectRegister) => read_register(state, instr.rb),
        Some(AddressingMode::Immediate) => instr.immediate_value,
        _ => None,
    };

    if let Some(val) = value {
        exec.dest_reg = Some(rd);
        exec.dest_value = Some(val);
        exec.flags_update = FlagsUpdate::UpdateNZ {
            zero: val == 0,
            negative: (val & 0x8000) != 0,
            carry: false,
            overflow: false,
        };
    } else {
        exec.flags_update = FlagsUpdate::None;
    }
}

fn execute_load(
    instr: &DecodedInstruction,
    state: &CoreState,
    mmio: &mut dyn MmioBus,
    exec: &mut ExecuteState,
    next_pc: u16,
) {
    exec.cycles = crate::timing::cycle_cost(CycleCostKind::Load).unwrap_or(2);
    exec.next_pc = Some(next_pc);

    let Some(rd) = instr.rd else {
        exec.flags_update = FlagsUpdate::None;
        return;
    };

    let Some(ea) = compute_effective_address(instr, state) else {
        exec.flags_update = FlagsUpdate::None;
        return;
    };

    exec.memory_addr = Some(ea);
    exec.is_mmio_operation = false;
    exec.is_mmio_write = false;

    let addr_region = crate::memory::decode_memory_region(ea);
    if matches!(addr_region, crate::memory::MemoryRegion::Mmio) {
        exec.is_mmio_operation = true;
    }

    let value = if exec.is_mmio_operation {
        mmio.read16(ea).unwrap_or_default()
    } else {
        let lo = state.memory[usize::from(ea)];
        let hi = state.memory[usize::from(ea.wrapping_add(1))];
        u16::from_be_bytes([lo, hi])
    };

    exec.dest_reg = Some(rd);
    exec.dest_value = Some(value);
    exec.flags_update = FlagsUpdate::UpdateNZ {
        zero: value == 0,
        negative: (value & 0x8000) != 0,
        carry: false,
        overflow: false,
    };
}

fn execute_store(
    instr: &DecodedInstruction,
    state: &CoreState,
    mmio: &mut dyn MmioBus,
    exec: &mut ExecuteState,
    next_pc: u16,
) {
    exec.cycles = crate::timing::cycle_cost(CycleCostKind::Store).unwrap_or(2);
    exec.next_pc = Some(next_pc);
    exec.flags_update = FlagsUpdate::None;

    let Some(value) = read_register(state, instr.ra) else {
        return;
    };

    let Some(ea) = compute_effective_address(instr, state) else {
        return;
    };

    exec.memory_addr = Some(ea);
    exec.memory_write_pending = true;
    exec.memory_write_value = Some(value);

    let addr_region = crate::memory::decode_memory_region(ea);
    if matches!(addr_region, crate::memory::MemoryRegion::Mmio) {
        exec.is_mmio_operation = true;
        exec.is_mmio_write = true;
        let _ = mmio.write16(ea, value);
    }
}

#[derive(Clone, Copy)]
enum AluOp {
    Add,
    Sub,
    And,
    Or,
    Xor,
    Shl,
    Shr,
}

#[allow(clippy::similar_names)]
fn execute_alu(
    instr: &DecodedInstruction,
    state: &CoreState,
    exec: &mut ExecuteState,
    next_pc: u16,
    op: AluOp,
) {
    exec.cycles = crate::timing::cycle_cost(CycleCostKind::Alu).unwrap_or(1);
    exec.next_pc = Some(next_pc);

    let Some(rd) = instr.rd else {
        exec.flags_update = FlagsUpdate::None;
        return;
    };

    let Some(reg_a) = read_register(state, instr.ra) else {
        exec.flags_update = FlagsUpdate::None;
        return;
    };

    let reg_b = read_register(state, instr.rb).unwrap_or(0);

    let (result, flags) = match op {
        AluOp::Add => {
            let (res, carry) = reg_a.overflowing_add(reg_b);
            let overflow = ((reg_a ^ reg_b) & (reg_a ^ res) & 0x8000) != 0;
            (res, compute_nzcv_flags(res, carry, overflow))
        }
        AluOp::Sub => {
            let (res, carry) = reg_a.overflowing_sub(reg_b);
            let overflow = ((reg_a ^ reg_b) & (reg_a ^ res) & 0x8000) != 0;
            (res, compute_nzcv_flags(res, carry, overflow))
        }
        AluOp::And => {
            let res = reg_a & reg_b;
            (res, compute_nzcv_flags(res, false, false))
        }
        AluOp::Or => {
            let res = reg_a | reg_b;
            (res, compute_nzcv_flags(res, false, false))
        }
        AluOp::Xor => {
            let res = reg_a ^ reg_b;
            (res, compute_nzcv_flags(res, false, false))
        }
        AluOp::Shl => {
            let shift = reg_b & 0x0F;
            let res = reg_a << shift;
            let carry = if shift > 0 {
                (reg_a >> (16 - shift)) & 1
            } else {
                0
            } != 0;
            (res, compute_nzcv_flags(res, carry, false))
        }
        AluOp::Shr => {
            let shift = reg_b & 0x0F;
            let res = reg_a >> shift;
            let carry = if shift > 0 {
                (reg_a >> (shift - 1)) & 1
            } else {
                0
            } != 0;
            (res, compute_nzcv_flags(res, carry, false))
        }
    };

    exec.dest_reg = Some(rd);
    exec.dest_value = Some(result);
    exec.flags_update = flags;
}

fn execute_cmp(
    instr: &DecodedInstruction,
    state: &CoreState,
    exec: &mut ExecuteState,
    next_pc: u16,
) {
    exec.cycles = crate::timing::cycle_cost(CycleCostKind::Alu).unwrap_or(1);
    exec.next_pc = Some(next_pc);
    exec.flags_update = FlagsUpdate::None;

    let Some(reg_a) = read_register(state, instr.ra) else {
        return;
    };

    let reg_b = read_register(state, instr.rb).unwrap_or(0);

    let (result, carry) = reg_a.overflowing_sub(reg_b);
    let overflow = ((reg_a ^ reg_b) & (reg_a ^ result) & 0x8000) != 0;

    exec.flags_update = compute_nzcv_flags(result, carry, overflow);
}

#[derive(Clone, Copy)]
enum MathOp {
    Mul,
    Mulh,
    Div,
    Mod,
    Qadd,
    Qsub,
    Scv,
}

#[allow(clippy::similar_names)]
fn execute_math(
    instr: &DecodedInstruction,
    state: &CoreState,
    exec: &mut ExecuteState,
    next_pc: u16,
    op: MathOp,
) {
    let cost_kind = match op {
        MathOp::Mul | MathOp::Mulh => CycleCostKind::Mul,
        MathOp::Div | MathOp::Mod => CycleCostKind::Div,
        MathOp::Qadd | MathOp::Qsub | MathOp::Scv => CycleCostKind::SaturatingHelper,
    };
    exec.cycles = crate::timing::cycle_cost(cost_kind).unwrap_or(1);
    exec.next_pc = Some(next_pc);

    let Some(rd) = instr.rd else {
        exec.flags_update = FlagsUpdate::None;
        return;
    };

    let Some(reg_a) = read_register(state, instr.ra) else {
        exec.flags_update = FlagsUpdate::None;
        return;
    };

    let reg_b = read_register(state, instr.rb).unwrap_or(0);

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let (result, flags) = match op {
        MathOp::Mul => {
            let res = u16::try_from(u32::from(reg_a) * u32::from(reg_b)).unwrap_or(0);
            (res, compute_nzcv_flags(res, false, false))
        }
        MathOp::Mulh => {
            let res = u16::try_from((u32::from(reg_a) * u32::from(reg_b)) >> 16).unwrap_or(0);
            (res, compute_nzcv_flags(res, false, false))
        }
        MathOp::Div => {
            let res = if reg_b == 0 { 0 } else { reg_a / reg_b };
            (res, compute_nzcv_flags(res, false, false))
        }
        MathOp::Mod => {
            let res = if reg_b == 0 { 0 } else { reg_a % reg_b };
            (res, compute_nzcv_flags(res, false, false))
        }
        MathOp::Qadd => {
            let a_i16 = reg_a as i16;
            let b_i16 = reg_b as i16;
            let sum = i32::from(a_i16) + i32::from(b_i16);
            let res = if sum > 32767 {
                0x7FFF
            } else if sum < -32768 {
                0x8000
            } else {
                u16::try_from(sum).unwrap_or(0)
            };
            let overflow = !(-32768..=32767).contains(&sum);
            (res, compute_nzcv_flags(res, false, overflow))
        }
        MathOp::Qsub => {
            let a_i16 = reg_a as i16;
            let b_i16 = reg_b as i16;
            let diff = i32::from(a_i16) - i32::from(b_i16);
            let res = if diff > 32767 {
                0x7FFF
            } else if diff < -32768 {
                0x8000
            } else {
                u16::try_from(diff).unwrap_or(0)
            };
            let overflow = !(-32768..=32767).contains(&diff);
            (res, compute_nzcv_flags(res, false, overflow))
        }
        MathOp::Scv => {
            let res = (reg_a as i16) as u16;
            (res, compute_nzcv_flags(res, false, false))
        }
    };

    exec.dest_reg = Some(rd);
    exec.dest_value = Some(result);
    exec.flags_update = flags;
}

#[derive(Clone, Copy)]
enum BranchOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

fn execute_branch(
    instr: &DecodedInstruction,
    state: &CoreState,
    exec: &mut ExecuteState,
    next_pc: u16,
    op: BranchOp,
) {
    let taken = match op {
        BranchOp::Eq => state.arch.flag_is_set(0x01),
        BranchOp::Ne => !state.arch.flag_is_set(0x01),
        BranchOp::Lt => state.arch.flag_is_set(0x02) != state.arch.flag_is_set(0x08),
        BranchOp::Le => {
            state.arch.flag_is_set(0x01)
                || state.arch.flag_is_set(0x02) != state.arch.flag_is_set(0x08)
        }
        BranchOp::Gt => {
            !state.arch.flag_is_set(0x01)
                && (state.arch.flag_is_set(0x02) == state.arch.flag_is_set(0x08))
        }
        BranchOp::Ge => state.arch.flag_is_set(0x02) == state.arch.flag_is_set(0x08),
    };

    if taken {
        exec.cycles = crate::timing::cycle_cost(CycleCostKind::BranchTaken).unwrap_or(2);
        let Some(ea) = compute_effective_address(instr, state) else {
            exec.next_pc = Some(next_pc);
            exec.flags_update = FlagsUpdate::None;
            return;
        };
        exec.next_pc = Some(ea);
    } else {
        exec.cycles = crate::timing::cycle_cost(CycleCostKind::BranchNotTaken).unwrap_or(1);
        exec.next_pc = Some(next_pc);
    }
    exec.flags_update = FlagsUpdate::None;
}

fn execute_jmp(
    instr: &DecodedInstruction,
    state: &CoreState,
    exec: &mut ExecuteState,
    next_pc: u16,
) {
    exec.cycles = crate::timing::cycle_cost(CycleCostKind::Jump).unwrap_or(2);

    let Some(ea) = compute_effective_address(instr, state) else {
        exec.next_pc = Some(next_pc);
        exec.flags_update = FlagsUpdate::None;
        return;
    };
    exec.next_pc = Some(ea);
    exec.flags_update = FlagsUpdate::None;
}

fn execute_call_or_ret(
    instr: &DecodedInstruction,
    state: &mut CoreState,
    exec: &mut ExecuteState,
    next_pc: u16,
) {
    let Some(target) = read_register(state, instr.ra) else {
        exec.cycles = crate::timing::cycle_cost(CycleCostKind::Ret).unwrap_or(2);
        let sp = state.arch.sp().wrapping_add(2);
        state.arch.set_sp(sp);
        exec.next_pc = Some(state.arch.sp());
        exec.flags_update = FlagsUpdate::None;
        return;
    };

    exec.cycles = crate::timing::cycle_cost(CycleCostKind::Call).unwrap_or(2);
    let sp = state.arch.sp().wrapping_sub(2);
    state.arch.set_sp(sp);
    exec.memory_addr = Some(sp);
    exec.memory_write_pending = true;
    exec.memory_write_value = Some(next_pc);
    exec.next_pc = Some(target);
    exec.flags_update = FlagsUpdate::None;
}

fn execute_push(
    instr: &DecodedInstruction,
    state: &mut CoreState,
    exec: &mut ExecuteState,
    next_pc: u16,
) {
    exec.cycles = crate::timing::cycle_cost(CycleCostKind::Push).unwrap_or(1);
    exec.next_pc = Some(next_pc);
    exec.flags_update = FlagsUpdate::None;

    let Some(value) = read_register(state, instr.ra) else {
        return;
    };

    let sp = state.arch.sp().wrapping_sub(2);
    state.arch.set_sp(sp);
    exec.memory_addr = Some(sp);
    exec.memory_write_pending = true;
    exec.memory_write_value = Some(value);
}

fn execute_pop(
    instr: &DecodedInstruction,
    state: &mut CoreState,
    exec: &mut ExecuteState,
    next_pc: u16,
) {
    exec.cycles = crate::timing::cycle_cost(CycleCostKind::Pop).unwrap_or(1);
    exec.next_pc = Some(next_pc);

    let Some(rd) = instr.rd else {
        exec.flags_update = FlagsUpdate::None;
        return;
    };

    let sp = state.arch.sp();
    let lo = state.memory[usize::from(sp)];
    let hi = state.memory[usize::from(sp.wrapping_add(1))];
    let value = u16::from_be_bytes([lo, hi]);

    state.arch.set_sp(sp.wrapping_add(2));
    exec.dest_reg = Some(rd);
    exec.dest_value = Some(value);
    exec.flags_update = FlagsUpdate::UpdateNZ {
        zero: value == 0,
        negative: (value & 0x8000) != 0,
        carry: false,
        overflow: false,
    };
}

fn execute_mmio_in(
    instr: &DecodedInstruction,
    state: &CoreState,
    mmio: &mut dyn MmioBus,
    exec: &mut ExecuteState,
    next_pc: u16,
) {
    exec.cycles = crate::timing::cycle_cost(CycleCostKind::MmioIn).unwrap_or(4);
    exec.next_pc = Some(next_pc);

    let Some(rd) = instr.rd else {
        exec.flags_update = FlagsUpdate::None;
        return;
    };

    let Some(ea) = compute_effective_address(instr, state) else {
        exec.flags_update = FlagsUpdate::None;
        return;
    };

    let value = mmio.read16(ea).unwrap_or_default();

    exec.dest_reg = Some(rd);
    exec.dest_value = Some(value);
    exec.flags_update = FlagsUpdate::UpdateNZ {
        zero: value == 0,
        negative: (value & 0x8000) != 0,
        carry: false,
        overflow: false,
    };
}

fn execute_mmio_out(
    instr: &DecodedInstruction,
    state: &CoreState,
    mmio: &mut dyn MmioBus,
    exec: &mut ExecuteState,
    next_pc: u16,
) {
    exec.cycles = crate::timing::cycle_cost(CycleCostKind::MmioOut).unwrap_or(4);
    exec.next_pc = Some(next_pc);
    exec.flags_update = FlagsUpdate::None;

    let Some(value) = read_register(state, instr.ra) else {
        return;
    };

    let Some(ea) = compute_effective_address(instr, state) else {
        return;
    };

    let _ = mmio.write16(ea, value);
}

fn execute_bitop(
    instr: &DecodedInstruction,
    state: &CoreState,
    mmio: &mut dyn MmioBus,
    exec: &mut ExecuteState,
    next_pc: u16,
) {
    let cost_kind = match instr.encoding {
        OpcodeEncoding::Bset => CycleCostKind::MmioBitSet,
        OpcodeEncoding::Bclr => CycleCostKind::MmioBitClear,
        OpcodeEncoding::Btest => CycleCostKind::MmioBitTest,
        _ => CycleCostKind::MmioIn,
    };
    exec.cycles = crate::timing::cycle_cost(cost_kind).unwrap_or(4);
    exec.next_pc = Some(next_pc);

    let Some(ea) = compute_effective_address(instr, state) else {
        exec.flags_update = FlagsUpdate::None;
        return;
    };

    let bit = instr.immediate_value.map_or(0, |v| v & 0x0F);

    let Ok(value) = mmio.read16(ea) else {
        exec.flags_update = FlagsUpdate::None;
        return;
    };

    let result = match instr.encoding {
        OpcodeEncoding::Bset => value | (1 << bit),
        OpcodeEncoding::Bclr => value & !(1 << bit),
        OpcodeEncoding::Btest => value,
        _ => value,
    };

    if matches!(instr.encoding, OpcodeEncoding::Bset | OpcodeEncoding::Bclr) {
        let _ = mmio.write16(ea, result);
    }

    exec.flags_update = FlagsUpdate::UpdateNZ {
        zero: (result & (1 << bit)) == 0,
        negative: false,
        carry: false,
        overflow: false,
    };
}

fn execute_ewait(
    _instr: &DecodedInstruction,
    state: &CoreState,
    exec: &mut ExecuteState,
    next_pc: u16,
) {
    exec.cycles = crate::timing::cycle_cost(CycleCostKind::Ewait).unwrap_or(1);

    if state.event_queue.is_empty() {
        exec.next_pc = Some(state.arch.pc());
    } else {
        exec.next_pc = Some(next_pc);
    }
    exec.flags_update = FlagsUpdate::None;
}

fn execute_eget(
    instr: &DecodedInstruction,
    state: &mut CoreState,
    exec: &mut ExecuteState,
    next_pc: u16,
) {
    exec.cycles = crate::timing::cycle_cost(CycleCostKind::Eget).unwrap_or(1);
    exec.next_pc = Some(next_pc);

    let Some(rd) = instr.rd else {
        exec.flags_update = FlagsUpdate::None;
        return;
    };

    if state.event_queue.is_empty() {
        exec.dest_reg = Some(rd);
        exec.dest_value = Some(0);
        exec.flags_update = FlagsUpdate::UpdateNZ {
            zero: true,
            negative: false,
            carry: false,
            overflow: false,
        };
    } else {
        let event_id = state.event_queue.events[0];
        let mut events = state.event_queue.events;
        for i in 0..(events.len() - 1) {
            events[i] = events[i + 1];
        }
        events[3] = 0;
        state.event_queue.events = events;
        state.event_queue.len = state.event_queue.len.saturating_sub(1);

        exec.dest_reg = Some(rd);
        exec.dest_value = Some(u16::from(event_id));
        exec.flags_update = FlagsUpdate::UpdateNZ {
            zero: event_id == 0,
            negative: (event_id & 0x80) != 0,
            carry: false,
            overflow: false,
        };
    }
}

fn execute_eret(
    _instr: &DecodedInstruction,
    state: &mut CoreState,
    exec: &mut ExecuteState,
    next_pc: u16,
) {
    exec.cycles = crate::timing::cycle_cost(CycleCostKind::EretReturn).unwrap_or(4);

    if !matches!(state.run_state, crate::state::RunState::HandlerContext) {
        exec.flags_update = FlagsUpdate::None;
        exec.next_pc = Some(next_pc);
        return;
    }

    let sp = state.arch.sp();
    let lo = state.memory[usize::from(sp)];
    let hi = state.memory[usize::from(sp.wrapping_add(1))];
    let return_pc = u16::from_be_bytes([lo, hi]);

    state.arch.set_sp(sp.wrapping_add(2));
    exec.next_pc = Some(return_pc);
    exec.flags_update = FlagsUpdate::None;
}

const fn compute_nzcv_flags(result: u16, carry: bool, overflow: bool) -> FlagsUpdate {
    FlagsUpdate::UpdateNZ {
        zero: result == 0,
        negative: (result & 0x8000) != 0,
        carry,
        overflow,
    }
}

/// Runs a single instruction step with budget enforcement and boundary transitions.
///
/// This function handles:
/// - Boundary transitions (HaltedForTick -> Running)
/// - Instruction decode and execution
/// - Tick budget checking after commit
/// - Budget fault handling
pub fn step_one(state: &mut CoreState, mmio: &mut dyn MmioBus, config: &CoreConfig) -> StepOutcome {
    match state.run_state {
        RunState::FaultLatched(_) => {
            return StepOutcome::Fault {
                cause: state
                    .run_state
                    .latched_fault()
                    .unwrap_or(crate::fault::FaultCode::IllegalEncoding),
            };
        }
        RunState::HandlerContext => {}
        RunState::HaltedForTick => {
            let current_tick = state.arch.tick();
            if current_tick >= config.tick_budget_cycles {
                state.run_state =
                    crate::state::RunState::FaultLatched(crate::fault::FaultCode::BudgetOverrun);
                return StepOutcome::Fault {
                    cause: crate::fault::FaultCode::BudgetOverrun,
                };
            }
            state.run_state = crate::state::RunState::Running;
        }
        RunState::Running => {}
    }

    let pc = state.arch.pc();
    let fetch_result = fetch_and_decode(pc, &state.memory);
    let instruction = match fetch_result {
        Ok(instr) => instr,
        Err(cause) => {
            return StepOutcome::Fault { cause };
        }
    };

    let (outcome, exec_state) = execute_instruction(&instruction, state, mmio);

    match outcome {
        ExecuteOutcome::Retired { cycles } => {
            commit_execution(state, &exec_state);

            let new_tick = state.arch.tick();
            if new_tick >= config.tick_budget_cycles {
                state.run_state = crate::state::RunState::HaltedForTick;
                return StepOutcome::HaltedForTick;
            }

            StepOutcome::Retired { cycles }
        }
        ExecuteOutcome::HaltedForTick => {
            commit_execution(state, &exec_state);
            state.run_state = crate::state::RunState::HaltedForTick;
            StepOutcome::HaltedForTick
        }
        ExecuteOutcome::TrapDispatch { cause } => {
            commit_execution(state, &exec_state);
            state.run_state = crate::state::RunState::HandlerContext;
            StepOutcome::TrapDispatch { cause }
        }
        ExecuteOutcome::EventDispatch { event_id } => {
            commit_execution(state, &exec_state);
            state.run_state = crate::state::RunState::HandlerContext;
            StepOutcome::EventDispatch { event_id }
        }
        ExecuteOutcome::Fault { cause } => {
            state.run_state = crate::state::RunState::FaultLatched(cause);
            StepOutcome::Fault { cause }
        }
    }
}

fn fetch_and_decode(pc: u16, memory: &[u8]) -> Result<DecodedInstruction, crate::fault::FaultCode> {
    let lo = memory[usize::from(pc)];
    let hi = memory[usize::from(pc.wrapping_add(1))];
    let raw_word = u16::from_be_bytes([lo, hi]);

    Decoder::decode(raw_word).into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decoder::Decoder;
    use crate::encoding::OpcodeEncoding;

    fn decode_instr(word: u16) -> DecodedInstruction {
        let result = Decoder::decode(word);
        result.instruction().expect("should decode")
    }

    #[test]
    fn nop_cycles_are_correct() {
        let instr = decode_instr(0x0000);
        assert_eq!(instr.encoding, OpcodeEncoding::Nop);
    }

    #[test]
    fn halt_cycles_are_correct() {
        let instr = decode_instr(0x0010);
        assert_eq!(instr.encoding, OpcodeEncoding::Halt);
    }

    #[test]
    fn mov_register_form_works() {
        let mut state = CoreState::default();
        state.arch.set_gpr(GeneralRegister::R1, 0x1234);

        // MOV R0, R1 - OP=1, SUB=0, RD=0, RA=1, RB=0, AM=0
        let instr = decode_instr(0x0488);
        let mut exec = ExecuteState::new(0);
        execute_mov(&instr, &state, &mut exec, 0x0002);

        assert!(exec.dest_reg.is_some());
        assert_eq!(exec.dest_value, Some(0x1234));
    }

    #[test]
    fn add_computes_correct_flags() {
        let mut state = CoreState::default();
        state.arch.set_gpr(GeneralRegister::R0, 5);
        state.arch.set_gpr(GeneralRegister::R1, 7);

        // ADD R0, R0, R1 - OP=4, SUB=0, RD=0, RA=0, RB=1, AM=0
        let instr = decode_instr(0x0208);
        let mut exec = ExecuteState::new(0);
        execute_alu(&instr, &state, &mut exec, 0x0002, AluOp::Add);

        assert_eq!(exec.dest_value, Some(12));
    }

    #[test]
    fn div_by_zero_returns_zero() {
        let mut state = CoreState::default();
        state.arch.set_gpr(GeneralRegister::R0, 10);
        state.arch.set_gpr(GeneralRegister::R1, 0);

        let instr = decode_instr(0x0288);
        let mut exec = ExecuteState::new(0);
        execute_math(&instr, &state, &mut exec, 0x0002, MathOp::Div);

        assert_eq!(exec.dest_value, Some(0));
    }

    #[test]
    fn mod_by_zero_returns_zero() {
        let mut state = CoreState::default();
        state.arch.set_gpr(GeneralRegister::R0, 10);
        state.arch.set_gpr(GeneralRegister::R1, 0);

        let instr = decode_instr(0x0300);
        let mut exec = ExecuteState::new(0);
        execute_math(&instr, &state, &mut exec, 0x0300, MathOp::Mod);

        assert_eq!(exec.dest_value, Some(0));
    }

    #[test]
    fn step_one_executes_nop_instruction() {
        let mut state = CoreState::default();
        state.memory[0x0000] = 0x00;
        state.memory[0x0001] = 0x00;

        struct NoMmio;
        impl MmioBus for NoMmio {
            fn read16(&mut self, _addr: u16) -> Result<u16, crate::api::MmioError> {
                Err(crate::api::MmioError::ReadFailed)
            }
            fn write16(
                &mut self,
                _addr: u16,
                _value: u16,
            ) -> Result<crate::api::MmioWriteResult, crate::api::MmioError> {
                Err(crate::api::MmioError::WriteFailed)
            }
        }

        let mut mmio = NoMmio;
        let config = CoreConfig::default();

        let outcome = step_one(&mut state, &mut mmio, &config);

        assert!(matches!(outcome, StepOutcome::Retired { cycles: 1 }));
        assert_eq!(state.arch.pc(), 0x0002);
        assert_eq!(state.arch.tick(), 1);
    }

    #[test]
    fn step_one_tick_increments_by_cycle_cost() {
        let mut state = CoreState::default();
        state.memory[0x0000] = 0x00;
        state.memory[0x0001] = 0x00;

        struct NoMmio;
        impl MmioBus for NoMmio {
            fn read16(&mut self, _addr: u16) -> Result<u16, crate::api::MmioError> {
                Err(crate::api::MmioError::ReadFailed)
            }
            fn write16(
                &mut self,
                _addr: u16,
                _value: u16,
            ) -> Result<crate::api::MmioWriteResult, crate::api::MmioError> {
                Err(crate::api::MmioError::WriteFailed)
            }
        }

        let mut mmio = NoMmio;
        let config = CoreConfig::default();

        step_one(&mut state, &mut mmio, &config);

        assert_eq!(state.arch.tick(), 1);
    }

    #[test]
    fn step_one_halt_advances_pc_and_sets_halted_for_tick() {
        let mut state = CoreState::default();
        state.memory[0x0000] = 0x00;
        state.memory[0x0001] = 0x10;

        struct NoMmio;
        impl MmioBus for NoMmio {
            fn read16(&mut self, _addr: u16) -> Result<u16, crate::api::MmioError> {
                Err(crate::api::MmioError::ReadFailed)
            }
            fn write16(
                &mut self,
                _addr: u16,
                _value: u16,
            ) -> Result<crate::api::MmioWriteResult, crate::api::MmioError> {
                Err(crate::api::MmioError::WriteFailed)
            }
        }

        let mut mmio = NoMmio;
        let config = CoreConfig::default();

        let outcome = step_one(&mut state, &mut mmio, &config);

        assert!(matches!(outcome, StepOutcome::HaltedForTick));
        assert_eq!(state.arch.pc(), 0x0002);
        assert_eq!(state.run_state, RunState::HaltedForTick);
    }

    #[test]
    fn step_one_resumes_from_halted_for_tick() {
        let mut state = CoreState {
            run_state: RunState::HaltedForTick,
            ..CoreState::default()
        };
        state.arch.set_tick(100);
        state.memory[0x0000] = 0x00;
        state.memory[0x0001] = 0x00;

        struct NoMmio;
        impl MmioBus for NoMmio {
            fn read16(&mut self, _addr: u16) -> Result<u16, crate::api::MmioError> {
                Err(crate::api::MmioError::ReadFailed)
            }
            fn write16(
                &mut self,
                _addr: u16,
                _value: u16,
            ) -> Result<crate::api::MmioWriteResult, crate::api::MmioError> {
                Err(crate::api::MmioError::WriteFailed)
            }
        }

        let mut mmio = NoMmio;
        let config = CoreConfig::default();

        let outcome = step_one(&mut state, &mut mmio, &config);

        assert!(matches!(outcome, StepOutcome::Retired { .. }));
        assert_eq!(state.run_state, RunState::Running);
    }

    #[test]
    fn step_one_budget_exceeded_triggers_halt() {
        let mut state = CoreState::default();
        state.arch.set_tick(639);
        state.memory[0x0000] = 0x00;
        state.memory[0x0001] = 0x00;

        struct NoMmio;
        impl MmioBus for NoMmio {
            fn read16(&mut self, _addr: u16) -> Result<u16, crate::api::MmioError> {
                Err(crate::api::MmioError::ReadFailed)
            }
            fn write16(
                &mut self,
                _addr: u16,
                _value: u16,
            ) -> Result<crate::api::MmioWriteResult, crate::api::MmioError> {
                Err(crate::api::MmioError::WriteFailed)
            }
        }

        let mut mmio = NoMmio;
        let config = CoreConfig::default();

        let outcome = step_one(&mut state, &mut mmio, &config);

        assert!(matches!(outcome, StepOutcome::HaltedForTick));
        assert_eq!(state.arch.tick(), 640);
        assert_eq!(state.run_state, RunState::HaltedForTick);
    }

    #[test]
    fn step_one_budget_exceeded_on_already_halted_faults() {
        let mut state = CoreState {
            run_state: RunState::HaltedForTick,
            ..CoreState::default()
        };
        state.arch.set_tick(640);
        state.memory[0x0000] = 0x00;
        state.memory[0x0001] = 0x00;

        struct NoMmio;
        impl MmioBus for NoMmio {
            fn read16(&mut self, _addr: u16) -> Result<u16, crate::api::MmioError> {
                Err(crate::api::MmioError::ReadFailed)
            }
            fn write16(
                &mut self,
                _addr: u16,
                _value: u16,
            ) -> Result<crate::api::MmioWriteResult, crate::api::MmioError> {
                Err(crate::api::MmioError::WriteFailed)
            }
        }

        let mut mmio = NoMmio;
        let config = CoreConfig::default();

        let outcome = step_one(&mut state, &mut mmio, &config);

        assert!(matches!(
            outcome,
            StepOutcome::Fault {
                cause: crate::fault::FaultCode::BudgetOverrun
            }
        ));
        assert_eq!(
            state.run_state,
            RunState::FaultLatched(crate::fault::FaultCode::BudgetOverrun)
        );
    }

    #[test]
    fn ewait_with_empty_queue_keeps_pc() {
        let mut state = CoreState::default();
        state.memory[0x0000] = 0xA0;
        state.memory[0x0001] = 0x00;

        struct NoMmio;
        impl MmioBus for NoMmio {
            fn read16(&mut self, _addr: u16) -> Result<u16, crate::api::MmioError> {
                Err(crate::api::MmioError::ReadFailed)
            }
            fn write16(
                &mut self,
                _addr: u16,
                _value: u16,
            ) -> Result<crate::api::MmioWriteResult, crate::api::MmioError> {
                Err(crate::api::MmioError::WriteFailed)
            }
        }

        let mut mmio = NoMmio;
        let config = CoreConfig::default();

        let outcome = step_one(&mut state, &mut mmio, &config);

        assert!(matches!(outcome, StepOutcome::Retired { cycles: 1 }));
        assert_eq!(state.arch.pc(), 0x0000);
    }

    #[test]
    fn ewait_with_event_advances_pc() {
        let mut state = CoreState::default();
        state.event_queue.events[0] = 0x42;
        state.event_queue.len = 1;
        state.memory[0x0000] = 0xA0;
        state.memory[0x0001] = 0x00;

        struct NoMmio;
        impl MmioBus for NoMmio {
            fn read16(&mut self, _addr: u16) -> Result<u16, crate::api::MmioError> {
                Err(crate::api::MmioError::ReadFailed)
            }
            fn write16(
                &mut self,
                _addr: u16,
                _value: u16,
            ) -> Result<crate::api::MmioWriteResult, crate::api::MmioError> {
                Err(crate::api::MmioError::WriteFailed)
            }
        }

        let mut mmio = NoMmio;
        let config = CoreConfig::default();

        let outcome = step_one(&mut state, &mut mmio, &config);

        assert!(matches!(outcome, StepOutcome::Retired { cycles: 1 }));
        assert_eq!(state.arch.pc(), 0x0002);
    }

    #[test]
    fn step_one_decode_fault_returns_fault_outcome() {
        let mut state = CoreState::default();
        state.memory[0x0000] = 0xB0;
        state.memory[0x0001] = 0x00;

        struct NoMmio;
        impl MmioBus for NoMmio {
            fn read16(&mut self, _addr: u16) -> Result<u16, crate::api::MmioError> {
                Err(crate::api::MmioError::ReadFailed)
            }
            fn write16(
                &mut self,
                _addr: u16,
                _value: u16,
            ) -> Result<crate::api::MmioWriteResult, crate::api::MmioError> {
                Err(crate::api::MmioError::WriteFailed)
            }
        }

        let mut mmio = NoMmio;
        let config = CoreConfig::default();

        let outcome = step_one(&mut state, &mut mmio, &config);

        assert!(matches!(
            outcome,
            StepOutcome::Fault {
                cause: crate::fault::FaultCode::IllegalEncoding
            }
        ));
    }

    #[test]
    fn step_one_fault_latched_returns_fault_immediately() {
        let mut state = CoreState {
            run_state: RunState::FaultLatched(crate::fault::FaultCode::IllegalEncoding),
            ..CoreState::default()
        };

        struct NoMmio;
        impl MmioBus for NoMmio {
            fn read16(&mut self, _addr: u16) -> Result<u16, crate::api::MmioError> {
                unreachable!()
            }
            fn write16(
                &mut self,
                _addr: u16,
                _value: u16,
            ) -> Result<crate::api::MmioWriteResult, crate::api::MmioError> {
                unreachable!()
            }
        }

        let mut mmio = NoMmio;
        let config = CoreConfig::default();

        let outcome = step_one(&mut state, &mut mmio, &config);

        assert!(matches!(
            outcome,
            StepOutcome::Fault {
                cause: crate::fault::FaultCode::IllegalEncoding
            }
        ));
    }
}
