use assembler::assembler::{assemble_from_source, AssembleResult};
use emulator_core::{
    disassemble_window, run_one, step_one, CompositeMmio, CoreConfig, CoreState, RunBoundary,
    RunOutcome, RunState, StepOutcome, Tele7Config, Tele7Peripheral,
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

/// Source map entry for editor integration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceMapEntry {
    /// Address of this entry (start of instruction/data).
    pub address: u16,
    /// Length in bytes (2 or 4 for instructions, variable for data).
    pub len_bytes: usize,
    /// Source file path.
    pub file: String,
    /// 1-indexed source line number.
    pub line: usize,
    /// Source line text.
    pub source: String,
}

/// Diagnostic severity.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
}

/// Structured diagnostic for editor integration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diagnostic {
    /// Severity level.
    pub severity: DiagnosticSeverity,
    /// Source file path.
    pub file: String,
    /// 1-indexed line number (0 if not associated with a line).
    pub line: usize,
    /// Diagnostic message.
    pub message: String,
}

/// Result of assemble-only operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssembleOnlyResult {
    /// Assembled binary bytes.
    pub binary: Vec<u8>,
    /// Source map entries (address-to-source mapping).
    pub source_map: Vec<SourceMapEntry>,
    /// Diagnostics (errors and warnings).
    pub diagnostics: Vec<Diagnostic>,
    /// Build ID (hash of binary for change detection).
    pub build_id: String,
}

/// Execution metadata for editor overlays.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionMetadata {
    /// Current program counter.
    pub pc: u16,
    /// Current tick counter.
    pub tick: u16,
    /// Current run state.
    pub run_state: String,
    /// Changed memory regions as [start, end] pairs (inclusive).
    pub changed_regions: Vec<[u16; 2]>,
    /// Whether a fault is latched.
    pub has_fault: bool,
    /// Latched fault code if any.
    pub fault_code: Option<u8>,
}

#[wasm_bindgen]
pub struct WasmCore {
    state: CoreState,
    config: CoreConfig,
    mmio: CompositeMmio,
    original_binary: Vec<u8>,
}

#[wasm_bindgen]
impl WasmCore {
    #[must_use]
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        console_error_panic_hook::set_once();
        let config = CoreConfig::default();
        let mmio = CompositeMmio::new().with_tele7(Tele7Peripheral::new(Tele7Config::default()));
        Self {
            state: CoreState::with_config(&config),
            config,
            mmio,
            original_binary: Vec::new(),
        }
    }

    fn load_program_with_tracking(&mut self, program: &[u8]) {
        let len = program.len().min(self.state.memory.len());
        self.state.memory[..len].copy_from_slice(&program[..len]);
        self.original_binary = program.to_vec();
        while self.original_binary.len() < self.state.memory.len() {
            self.original_binary.push(0);
        }
    }

    /// Loads a program into memory starting at address 0x0000.
    pub fn load_program(&mut self, program: &[u8]) {
        let len = program.len().min(self.state.memory.len());
        self.state.memory[..len].copy_from_slice(&program[..len]);
    }

    /// Assembles assembly source text (`.n1` or `.n1.md`) and loads it.
    ///
    /// `file_name` is used to select plain vs literate extraction semantics.
    ///
    /// # Errors
    ///
    /// Returns a JS error value when parsing, address assignment, or encoding
    /// fails.
    pub fn assemble_and_load_program(
        &mut self,
        source: &str,
        file_name: &str,
    ) -> Result<(), JsValue> {
        let result = assemble_from_source(source, file_name)
            .map_err(|err| JsValue::from_str(&err.to_string()))?;

        self.load_program_with_tracking(&result.binary);
        Ok(())
    }

    /// Assembles source text without loading into memory.
    ///
    /// Returns a JSON object containing:
    /// - `binary`: array of bytes
    /// - `source_map`: array of {address, `len_bytes`, file, line, source}
    /// - `diagnostics`: array of {severity, file, line, message}
    /// - `build_id`: hash string for change detection
    ///
    /// # Errors
    ///
    /// Returns a JS error value when assembly fails.
    pub fn assemble_only(&self, source: &str, file_name: &str) -> Result<JsValue, JsValue> {
        let result = assemble_from_source(source, file_name)
            .map_err(|err| JsValue::from_str(&err.to_string()))?;

        let assemble_result = convert_assemble_result(result, file_name);

        serde_wasm_bindgen::to_value(&assemble_result)
            .map_err(|err| JsValue::from_str(&err.to_string()))
    }

    /// Patches memory at a specific address range.
    ///
    /// This is a targeted update that only modifies the specified range,
    /// preserving execution state (registers, flags, etc.).
    ///
    /// # Errors
    ///
    /// Returns a JS error if the address range is invalid.
    #[allow(clippy::cast_possible_truncation)]
    pub fn patch_memory(&mut self, address: u16, data: &[u8]) -> Result<(), JsValue> {
        let start = address as usize;
        let end = start.saturating_add(data.len());

        if end > self.state.memory.len() {
            return Err(JsValue::from_str(&format!(
                "patch range 0x{:04X}-0x{:04X} exceeds memory bounds",
                address,
                (end.saturating_sub(1)) as u16
            )));
        }

        self.state.memory[start..end].copy_from_slice(data);
        Ok(())
    }

    /// Returns execution metadata for editor overlays.
    ///
    /// Includes current PC, tick, run state, changed memory regions,
    /// and fault status.
    ///
    /// # Errors
    ///
    /// Returns a JS error value when serialization fails.
    pub fn get_execution_metadata(&self) -> Result<JsValue, JsValue> {
        let metadata = self.get_metadata_internal();
        serde_wasm_bindgen::to_value(&metadata).map_err(|err| JsValue::from_str(&err.to_string()))
    }

    /// Resets the core to its initial state.
    pub fn reset(&mut self) {
        self.state = CoreState::with_config(&self.config);
    }

    /// Resets the core and reloads the last loaded program.
    ///
    /// This is a "clean run" that resets all state.
    pub fn reset_and_reload(&mut self) {
        self.state = CoreState::with_config(&self.config);
        if !self.original_binary.is_empty() {
            let len = self.original_binary.len().min(self.state.memory.len());
            self.state.memory[..len].copy_from_slice(&self.original_binary[..len]);
        }
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

    /// Returns whether TELE-7 is currently enabled.
    #[must_use]
    pub fn tele7_enabled(&self) -> bool {
        self.mmio
            .tele7()
            .is_some_and(|tele7| tele7.state().is_enabled())
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

    /// Returns the TELE-7 display state for rendering.
    ///
    /// Returns a JSON object containing:
    /// - `enabled`: boolean - display is enabled
    /// - `pageMapped`: boolean - page buffer is valid
    /// - `fault`: boolean - device has a fault
    /// - `blinkPhase`: boolean - current blink phase
    /// - `origin`: number - scroll origin
    /// - `borderColor`: number - border color (0-7)
    /// - `buffer`: array of [high, low] byte pairs (500 words)
    ///
    /// # Errors
    ///
    /// Returns a JS error value when result serialization fails.
    pub fn get_tele7_state(&self) -> Result<JsValue, JsValue> {
        #[derive(Serialize)]
        #[allow(clippy::struct_excessive_bools)]
        struct Tele7DisplayState<'a> {
            enabled: bool,
            page_mapped: bool,
            fault: bool,
            blink_phase: bool,
            origin: u16,
            border_color: u8,
            buffer: &'a [[u8; 2]],
        }

        let Some(t7) = self.mmio.tele7() else {
            return Err(JsValue::from_str("TELE-7 not available"));
        };

        let state = t7.state();
        let buffer = t7.get_display_buffer(&self.state.memory);

        let display_state = Tele7DisplayState {
            enabled: state.is_enabled(),
            page_mapped: state.page_mapped(),
            fault: false,
            blink_phase: state.blink_phase(),
            origin: state.origin(),
            border_color: state.border_color(),
            buffer: &buffer,
        };

        serde_wasm_bindgen::to_value(&display_state)
            .map_err(|err| JsValue::from_str(&err.to_string()))
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
        self.mmio.tick();
        if matches!(self.state.run_state, RunState::HaltedForTick) {
            self.state.run_state = RunState::Running;
        }
        outcome.into()
    }

    fn run_internal(&mut self, boundary: RunBoundary) -> WasmRunOutcome {
        run_one(&mut self.state, &mut self.mmio, &self.config, boundary).into()
    }

    fn get_metadata_internal(&self) -> ExecutionMetadata {
        let changed_regions = compute_changed_regions(&self.state.memory, &self.original_binary);

        let (has_fault, fault_code) = match self.state.run_state {
            RunState::FaultLatched(code) => (true, Some(code.as_u8())),
            _ => (false, None),
        };

        let run_state = match &self.state.run_state {
            RunState::Running => "Running".to_string(),
            RunState::HaltedForTick => "HaltedForTick".to_string(),
            RunState::HandlerContext => "HandlerContext".to_string(),
            RunState::FaultLatched(code) => format!("FaultLatched({})", code.as_u8()),
        };

        ExecutionMetadata {
            pc: self.state.arch.pc(),
            tick: self.state.arch.tick(),
            run_state,
            changed_regions,
            has_fault,
            fault_code,
        }
    }
}

fn convert_assemble_result(result: AssembleResult, _file_name: &str) -> AssembleOnlyResult {
    let source_map: Vec<SourceMapEntry> = result
        .listing
        .into_iter()
        .map(|entry| SourceMapEntry {
            address: entry.address,
            len_bytes: entry.bytes.len(),
            file: entry.location.clone(),
            line: 0,
            source: entry.source,
        })
        .collect();

    let mut diagnostics = Vec::new();

    for warning in &result.warnings {
        diagnostics.push(Diagnostic {
            severity: DiagnosticSeverity::Warning,
            file: warning
                .location
                .as_ref()
                .map(|l| l.file.clone())
                .unwrap_or_default(),
            line: warning.location.as_ref().map_or(0, |l| l.line),
            message: warning.to_string(),
        });
    }

    let build_id = format!("{:016x}", compute_build_id(&result.binary));

    AssembleOnlyResult {
        binary: result.binary,
        source_map,
        diagnostics,
        build_id,
    }
}

fn compute_build_id(binary: &[u8]) -> u64 {
    let mut hash: u64 = 0;
    for chunk in binary.chunks(8) {
        let mut arr = [0u8; 8];
        arr[..chunk.len()].copy_from_slice(chunk);
        hash = hash.wrapping_add(u64::from_le_bytes(arr));
        hash = hash.wrapping_mul(0x517c_c1b7_2722_0a95);
    }
    hash
}

#[allow(clippy::cast_possible_truncation)]
fn compute_changed_regions(current: &[u8], original: &[u8]) -> Vec<[u16; 2]> {
    let mut regions = Vec::new();
    let mut in_region = false;
    let mut region_start: u16 = 0;

    for (i, (c, o)) in current.iter().zip(original.iter()).enumerate() {
        let changed = c != o;
        let addr = i as u16;

        if changed && !in_region {
            region_start = addr;
            in_region = true;
        } else if !changed && in_region {
            regions.push([region_start, addr.saturating_sub(1)]);
            in_region = false;
        }
    }

    if in_region {
        let end = (current.len().min(original.len()) as u16).saturating_sub(1);
        regions.push([region_start, end]);
    }

    coalesce_adjacent_regions(regions)
}

fn coalesce_adjacent_regions(regions: Vec<[u16; 2]>) -> Vec<[u16; 2]> {
    if regions.is_empty() {
        return regions;
    }

    let mut result = Vec::with_capacity(regions.len());
    let mut current = regions[0];

    for region in regions.into_iter().skip(1) {
        if region[0].saturating_sub(1) <= current[1] {
            current[1] = region[1].max(current[1]);
        } else {
            result.push(current);
            current = region;
        }
    }
    result.push(current);
    result
}

#[cfg(test)]
mod tests {
    use super::{
        assemble_from_source, compute_changed_regions, convert_assemble_result, WasmCore,
        WasmRunBoundary, WasmStepOutcome,
    };

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

    #[test]
    fn tele7_self_test_source_enables_display_via_wasm_api() {
        let mut core = WasmCore::new();
        let source = include_str!("../../../programs/tele7_self_test.n1.md");

        core.assemble_and_load_program(source, "tele7_self_test.n1.md")
            .expect("source assembly should succeed");

        for _ in 0..4 {
            let _ = core.step_internal();
        }

        assert!(
            core.tele7_enabled(),
            "TELE-7 should be enabled after self-test init sequence"
        );
    }

    #[test]
    fn tele7_self_test_markdown_raw_bytes_do_not_enable_display() {
        let mut core = WasmCore::new();
        let source = include_str!("../../../programs/tele7_self_test.n1.md");

        core.load_program(source.as_bytes());
        for _ in 0..4 {
            let _ = core.step_internal();
        }

        assert!(
            !core.tele7_enabled(),
            "raw markdown bytes should not accidentally enable TELE-7"
        );
    }

    #[test]
    fn patch_memory_writes_to_specified_address() {
        let mut core = WasmCore::new();
        core.load_program(&[0x00, 0x00, 0x00, 0x10]);

        core.patch_memory(2, &[0x12, 0x34]).unwrap();

        assert_eq!(core.state.memory[2], 0x12);
        assert_eq!(core.state.memory[3], 0x34);
    }

    #[test]
    fn patch_memory_validates_bounds() {
        let mut core = WasmCore::new();
        core.load_program_with_tracking(&[0x00, 0x00]);

        let valid_result = core.patch_memory(0x0000, &[0xFF]);
        assert!(valid_result.is_ok());

        core.state.memory[0] = 0xFF;
        assert_eq!(core.state.memory[0], 0xFF);
    }

    #[test]
    fn get_execution_metadata_returns_current_state() {
        let mut core = WasmCore::new();
        core.load_program_with_tracking(&[0x00, 0x00, 0x00, 0x10]);

        let metadata = core.get_metadata_internal();

        assert_eq!(metadata.pc, 0);
        assert_eq!(metadata.tick, 0);
        assert!(!metadata.has_fault);
        assert!(metadata.changed_regions.is_empty());
    }

    #[test]
    fn get_execution_metadata_detects_memory_changes() {
        let mut core = WasmCore::new();
        core.load_program_with_tracking(&[0x00, 0x00, 0x00, 0x10]);

        core.state.memory[0] = 0xFF;

        let metadata = core.get_metadata_internal();

        assert!(!metadata.changed_regions.is_empty());
        assert_eq!(metadata.changed_regions[0][0], 0);
    }

    #[test]
    fn reset_and_reload_restores_original_program() {
        let mut core = WasmCore::new();
        core.load_program_with_tracking(&[0x00, 0x00, 0x00, 0x10]);

        core.state.memory[0] = 0xFF;
        core.state.arch.set_pc(4);

        core.reset_and_reload();

        assert_eq!(core.state.memory[0], 0x00);
        assert_eq!(core.state.arch.pc(), 0);
    }

    #[test]
    fn assemble_and_load_with_metadata_loads_binary() {
        let mut core = WasmCore::new();
        let result = assemble_from_source("NOP\nHALT\n", "test.n1");
        assert!(result.is_ok());

        let res = result.unwrap();
        core.load_program_with_tracking(&res.binary);

        assert!(!core.original_binary.is_empty());
        assert_eq!(core.original_binary[0], 0x00);
        assert_eq!(core.original_binary[2], 0x00);
        assert_eq!(core.original_binary[3], 0x10);
    }

    #[test]
    fn convert_assemble_result_produces_valid_source_map() {
        let result = assemble_from_source("NOP\nHALT\n", "test.n1").unwrap();
        let converted = convert_assemble_result(result, "test.n1");

        assert!(!converted.binary.is_empty());
        assert_eq!(converted.source_map.len(), 2);
        assert!(!converted.build_id.is_empty());
    }

    #[test]
    fn compute_changed_regions_detects_single_byte_change() {
        let current = [0xFF, 0x00, 0x00, 0x00];
        let original = [0x00, 0x00, 0x00, 0x00];

        let regions = compute_changed_regions(&current, &original);

        assert_eq!(regions.len(), 1);
        assert_eq!(regions[0], [0, 0]);
    }

    #[test]
    fn compute_changed_regions_coalesces_adjacent() {
        let current = [0xFF, 0xFF, 0x00, 0x00];
        let original = [0x00, 0x00, 0x00, 0x00];

        let regions = compute_changed_regions(&current, &original);

        assert_eq!(regions.len(), 1);
        assert_eq!(regions[0], [0, 1]);
    }

    #[test]
    fn compute_changed_regions_handles_multiple_regions() {
        let current = [0xFF, 0x00, 0xFF, 0xFF];
        let original = [0x00, 0x00, 0x00, 0x00];

        let regions = compute_changed_regions(&current, &original);

        assert_eq!(regions.len(), 2);
        assert_eq!(regions[0], [0, 0]);
        assert_eq!(regions[1], [2, 3]);
    }
}
