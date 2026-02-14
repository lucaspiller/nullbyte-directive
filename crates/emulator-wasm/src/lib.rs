use emulator_core::{
    disassemble_window, run_one, step_one, CoreConfig, CoreState, MmioBus, MmioError,
    MmioWriteResult, RunBoundary, RunOutcome, RunState, StepOutcome,
};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

/// JS-compatible version of `StepOutcome`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum WasmStepOutcome {
    Retired { cycles: u16 },
    HaltedForTick,
    TrapDispatch { cause: u16 },
    EventDispatch { event_id: u8 },
    Fault { cause: u8 },
}

/// JS-compatible version of `RunOutcome`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct WasmRunOutcome {
    pub steps: u32,
    pub final_step: WasmStepOutcome,
}

/// JS-compatible run boundary selector.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum WasmRunBoundary {
    #[default]
    TickBoundary,
    Halted,
    Fault,
}

impl From<StepOutcome> for WasmStepOutcome {
    fn from(value: StepOutcome) -> Self {
        match value {
            StepOutcome::Retired { cycles } => Self::Retired { cycles },
            StepOutcome::HaltedForTick => Self::HaltedForTick,
            StepOutcome::TrapDispatch { cause } => Self::TrapDispatch { cause },
            StepOutcome::EventDispatch { event_id } => Self::EventDispatch { event_id },
            StepOutcome::Fault { cause } => Self::Fault {
                cause: cause.as_u8(),
            },
        }
    }
}

impl From<RunBoundary> for WasmRunBoundary {
    fn from(value: RunBoundary) -> Self {
        match value {
            RunBoundary::TickBoundary => Self::TickBoundary,
            RunBoundary::Halted => Self::Halted,
            RunBoundary::Fault => Self::Fault,
        }
    }
}

impl From<WasmRunBoundary> for RunBoundary {
    fn from(value: WasmRunBoundary) -> Self {
        match value {
            WasmRunBoundary::TickBoundary => Self::TickBoundary,
            WasmRunBoundary::Halted => Self::Halted,
            WasmRunBoundary::Fault => Self::Fault,
        }
    }
}

impl From<RunOutcome> for WasmRunOutcome {
    fn from(value: RunOutcome) -> Self {
        Self {
            steps: value.steps,
            final_step: value.final_step.into(),
        }
    }
}

#[wasm_bindgen]
pub struct WasmCore {
    state: CoreState,
    config: CoreConfig,
    mmio: WebMmio,
}

#[wasm_bindgen]
impl WasmCore {
    #[must_use]
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        console_error_panic_hook::set_once();
        let config = CoreConfig::default();
        Self {
            state: CoreState::with_config(&config),
            config,
            mmio: WebMmio,
        }
    }

    /// Loads a program into memory starting at address 0x0000.
    pub fn load_program(&mut self, program: &[u8]) {
        let len = program.len().min(self.state.memory.len());
        self.state.memory[..len].copy_from_slice(&program[..len]);
    }

    /// Resets the core to its initial state.
    pub fn reset(&mut self) {
        self.state = CoreState::with_config(&self.config);
    }

    /// Executes a single instruction and returns the outcome as a JSON object.
    ///
    /// # Errors
    ///
    /// Returns a JS error value when result serialization fails.
    pub fn step(&mut self) -> Result<JsValue, JsValue> {
        let outcome = self.step_internal();
        serde_wasm_bindgen::to_value(&outcome).map_err(|err| JsValue::from_str(&err.to_string()))
    }

    /// Executes one complete tick (until tick boundary) and returns the outcome.
    /// Resets TICK to 0 and transitions from `HaltedForTick` to Running.
    ///
    /// # Errors
    ///
    /// Returns a JS error value when result serialization fails.
    pub fn tick(&mut self) -> Result<JsValue, JsValue> {
        let outcome = self.tick_internal();
        serde_wasm_bindgen::to_value(&outcome).map_err(|err| JsValue::from_str(&err.to_string()))
    }

    /// Runs until the supplied boundary and returns the run outcome as JSON.
    ///
    /// `boundary_val` accepts serialized `WasmRunBoundary` values, or defaults to
    /// `TickBoundary` if parsing fails.
    ///
    /// # Errors
    ///
    /// Returns a JS error value when result serialization fails.
    pub fn run_until(&mut self, boundary_val: JsValue) -> Result<JsValue, JsValue> {
        let boundary = serde_wasm_bindgen::from_value::<WasmRunBoundary>(boundary_val)
            .unwrap_or_default()
            .into();
        let outcome = self.run_internal(boundary);
        serde_wasm_bindgen::to_value(&outcome).map_err(|err| JsValue::from_str(&err.to_string()))
    }

    /// Returns the full core state as a JSON object.
    ///
    /// # Errors
    ///
    /// Returns a JS error value when state serialization fails.
    pub fn get_state(&self) -> Result<JsValue, JsValue> {
        serde_wasm_bindgen::to_value(&self.state).map_err(|err| JsValue::from_str(&err.to_string()))
    }

    /// Returns the memory contents as a `Uint8Array` view into wasm memory.
    #[must_use]
    pub fn get_memory(&self) -> js_sys::Uint8Array {
        js_sys::Uint8Array::from(self.state.memory.as_ref())
    }

    /// Disassembles a window of instructions around the given program counter.
    ///
    /// Returns a JSON array of disassembly rows. Each row contains:
    /// - `addr_start`: number (instruction address)
    /// - `len_bytes`: number (2 or 4)
    /// - `raw_words`: number (raw encoding)
    /// - `mnemonic`: string
    /// - `operands`: string
    /// - `is_illegal`: boolean
    ///
    /// # Errors
    ///
    /// Returns a JS error value when result serialization fails.
    pub fn disassemble_window(
        &self,
        center_pc: u16,
        before: usize,
        after: usize,
    ) -> Result<JsValue, JsValue> {
        let rows = disassemble_window(center_pc, before, after, &self.state.memory);
        serde_wasm_bindgen::to_value(&rows).map_err(|err| JsValue::from_str(&err.to_string()))
    }
}

impl Default for WasmCore {
    fn default() -> Self {
        Self::new()
    }
}

impl WasmCore {
    const fn resume_from_halted(&mut self) {
        if matches!(self.state.run_state, RunState::HaltedForTick) {
            self.state.arch.set_tick(0);
            self.state.run_state = RunState::Running;
        }
    }

    fn step_internal(&mut self) -> WasmStepOutcome {
        self.resume_from_halted();
        step_one(&mut self.state, &mut self.mmio, &self.config).into()
    }

    fn tick_internal(&mut self) -> WasmRunOutcome {
        self.resume_from_halted();
        let outcome = run_one(
            &mut self.state,
            &mut self.mmio,
            &self.config,
            RunBoundary::TickBoundary,
        );
        self.state.arch.set_tick(0);
        if matches!(self.state.run_state, RunState::HaltedForTick) {
            self.state.run_state = RunState::Running;
        }
        outcome.into()
    }

    fn run_internal(&mut self, boundary: RunBoundary) -> WasmRunOutcome {
        run_one(&mut self.state, &mut self.mmio, &self.config, boundary).into()
    }
}

struct WebMmio;

impl MmioBus for WebMmio {
    fn read16(&mut self, _addr: u16) -> Result<u16, MmioError> {
        Ok(0)
    }

    fn write16(&mut self, _addr: u16, _value: u16) -> Result<MmioWriteResult, MmioError> {
        Ok(MmioWriteResult::Applied)
    }
}

#[cfg(test)]
mod tests {
    use super::{WasmCore, WasmRunBoundary, WasmStepOutcome};

    #[test]
    fn step_executes_loaded_nop_and_advances_pc_tick() {
        let mut core = WasmCore::new();
        // NOP uses opcode 0x0 in this encoding table.
        core.load_program(&[0x00, 0x00]);

        let outcome = core.step_internal();
        assert_eq!(outcome, WasmStepOutcome::Retired { cycles: 1 });
        assert_eq!(core.state.arch.pc(), 2);
        assert_eq!(core.state.arch.tick(), 1);
    }

    #[test]
    fn run_until_fault_boundary_reports_fault_for_reserved_opcode() {
        let mut core = WasmCore::new();
        // 0xF000 encodes a reserved primary opcode and must fault immediately.
        core.load_program(&[0xF0, 0x00]);

        let outcome = core.run_internal(WasmRunBoundary::Fault.into());
        assert_eq!(outcome.steps, 1);
        assert!(matches!(outcome.final_step, WasmStepOutcome::Fault { .. }));
    }
}
