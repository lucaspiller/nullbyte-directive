use emulator_core::{
    CoreConfig, CoreProfile, CoreSnapshot, CoreState, MmioBus, MmioError, MmioWriteResult,
    RunBoundary, RunOutcome, SnapshotVersion, StepOutcome, TraceEvent, TraceSink,
    DEFAULT_TICK_BUDGET_CYCLES,
};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

macro_rules! console_log {
    ($($t:tt)*) => (log(&format!($($t)*)))
}

#[wasm_bindgen]
pub struct WasmCore {
    state: CoreState,
    config: CoreConfig,
}

#[wasm_bindgen]
impl WasmCore {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        console_error_panic_hook::set_once();
        Self {
            state: CoreState::default(),
            config: CoreConfig::default(),
        }
    }

    /// Loads a program into memory starting at address 0x0000.
    pub fn load_program(&mut self, program: &[u8]) {
        let len = program.len().min(self.state.memory.len());
        self.state.memory[..len].copy_from_slice(&program[..len]);
        console_log!("Loaded {} bytes into memory", len);
    }

    /// Resets the core to its initial state.
    pub fn reset(&mut self) {
        self.state = CoreState::default();
    }

    /// Executes a single instruction.
    /// Returns the step outcome as a JSON object.
    pub fn step(&mut self) -> JsValue {
        // TODO: Call emulator_core::step_one when available.
        // For now, we simulate a NOP retirement to validate the bridge.

        let outcome = StepOutcome::Retired { cycles: 1 };

        // Simple mock behavior: increment PC to show movement
        self.state.arch.pc = self.state.arch.pc.wrapping_add(1);

        serde_wasm_bindgen::to_value(&outcome).unwrap()
    }

    /// Runs until the specified boundary.
    /// Returns the run outcome as a JSON object.
    pub fn run_until(&mut self, boundary_val: JsValue) -> JsValue {
        // TODO: Call emulator_core::run_until_boundary when available.
        // For now, simulate running 1 step.

        // Deserialize boundary enum from JS (if needed, or just use string/int)
        // For simplicity in this scaffold, we ignore the boundary and run 1 step.

        let step_outcome = StepOutcome::Retired { cycles: 1 };
        self.state.arch.pc = self.state.arch.pc.wrapping_add(1);

        let outcome = RunOutcome {
            steps: 1,
            final_step: step_outcome,
        };

        serde_wasm_bindgen::to_value(&outcome).unwrap()
    }

    /// Returns the full core state as a JSON object.
    pub fn get_state(&self) -> JsValue {
        serde_wasm_bindgen::to_value(&self.state).unwrap()
    }

    /// Returns the memory contents as a Uint8Array.
    /// This is more efficient than serializing the whole memory to JSON.
    pub fn get_memory(&self) -> js_sys::Uint8Array {
        unsafe { js_sys::Uint8Array::view(&self.state.memory) }
    }
}

// Simple dummy MMIO bus for now
struct WebMmio;

impl MmioBus for WebMmio {
    fn read16(&mut self, _addr: u16) -> Result<u16, MmioError> {
        Ok(0)
    }

    fn write16(&mut self, _addr: u16, _value: u16) -> Result<MmioWriteResult, MmioError> {
        Ok(MmioWriteResult::Applied)
    }
}
