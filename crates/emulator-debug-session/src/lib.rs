//! Session host for Nullbyte Directive debugging workflows.
//!
//! This crate provides a transport-agnostic host API that owns runtime state,
//! build/layout metadata, reload policy, and source-first breakpoint mapping.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use assembler::assembler::{assemble, AssembleError, AssembleResult, ListingEntry};
use emulator_core::{
    disassemble_window, run_one, step_one, CompositeMmio, CoreConfig, CoreState, DisassemblyRow,
    RunBoundary, RunOutcome, RunState, StepOutcome, Tele7Config, Tele7Peripheral, ROM_END,
};
use serde::{Deserialize, Serialize};
use serde_json as _;

static NEXT_SESSION_ID: AtomicU64 = AtomicU64::new(1);

/// Reload behavior requested by callers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReloadMode {
    /// Reset execution state and reload the full build artifact.
    CleanRun,
    /// Attempt a ROM-only patch while preserving runtime state.
    HotPatch,
}

/// Reason code explaining why host policy chose `CleanRun`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CleanRunReason {
    /// Clean run was explicitly requested by caller.
    Requested,
    /// Hot patch was denied because host is not paused.
    HotPatchRequiresPause,
    /// Hot patch was denied because program counter is not on an instruction boundary.
    HotPatchRequiresInstructionBoundary,
    /// Hot patch assembly failed; host fell back to deterministic clean run.
    HotPatchAssemblyFailed,
    /// Hot patch layout compatibility check failed.
    HotPatchLayoutIncompatible,
}

/// A source location used by debugger-facing mapping and breakpoints.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct SourceLocation {
    /// Normalized source file path string.
    pub file: String,
    /// 1-indexed source line number.
    pub line: usize,
}

/// Breakpoint resolution state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BreakpointStatus {
    /// Breakpoint currently maps to one or more executable addresses.
    Resolved {
        /// Sorted executable addresses mapped to this source location.
        addresses: Vec<u16>,
    },
    /// Breakpoint is present but currently not bound to executable code.
    Unresolved {
        /// Explanation for unresolved status.
        reason: BreakpointUnresolvedReason,
    },
}

/// Reason for unresolved breakpoint status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BreakpointUnresolvedReason {
    /// No source mapping exists for this source location in the active layout.
    NoSourceMatch,
    /// The layout changed and this location could not be remapped.
    LayoutChanged,
}

/// A source-first breakpoint entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Breakpoint {
    /// Stable identifier within the host session.
    pub id: u32,
    /// Source anchor for this breakpoint.
    pub source: SourceLocation,
    /// Current resolution status.
    pub status: BreakpointStatus,
}

/// Core session metadata identifiers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionIds {
    /// Stable runtime session identifier.
    pub debug_session_id: String,
    /// Hash of assembled binary bytes.
    pub build_id: String,
    /// Hash of layout shape (address/length/source mapping).
    pub layout_id: String,
}

/// Host-visible state snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostStateSnapshot {
    /// Current session identifiers.
    pub ids: SessionIds,
    /// Current emulator core state.
    pub state: CoreState,
    /// Whether host execution control is paused.
    pub paused: bool,
    /// Whether session has been terminated.
    pub terminated: bool,
}

/// A memory window read result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryWindow {
    /// Start address of this memory window.
    pub start: u16,
    /// Window bytes.
    pub bytes: Vec<u8>,
}

/// TELE-7 visualizer data produced from host-owned peripheral state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tele7VisualizerSnapshot {
    /// Whether the TELE-7 output is enabled.
    pub enabled: bool,
    /// Current blink phase.
    pub blink_phase: bool,
    /// Current border color.
    pub border_color: u8,
    /// Current rendered 40x25 display buffer as packed character words.
    pub buffer: Vec<[u8; 2]>,
}

/// Visualization-focused host snapshot for debug-tool attach mode.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VisualizerSnapshot {
    /// Current session identifiers.
    pub ids: SessionIds,
    /// Current emulator core state.
    pub state: CoreState,
    /// Full memory snapshot for memory-map visualization.
    pub memory: Vec<u8>,
    /// Disassembly rows around the active PC.
    pub disassembly: Vec<DisassemblyRow>,
    /// TELE-7 visualizer snapshot when peripheral is present.
    pub tele7: Option<Tele7VisualizerSnapshot>,
}

/// Source map row persisted by the host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutEntry {
    /// Start address of row.
    pub address: u16,
    /// Row length in bytes.
    pub len_bytes: usize,
    /// Source file path.
    pub file: String,
    /// 1-indexed source line.
    pub line: usize,
}

/// Launch/reload report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildReport {
    /// Active reload mode applied by host.
    pub applied_mode: ReloadMode,
    /// Optional fallback reason when host selected `CleanRun`.
    pub clean_run_reason: Option<CleanRunReason>,
    /// Active session identifiers after operation.
    pub ids: SessionIds,
    /// Active source map entries.
    pub source_map: Vec<LayoutEntry>,
    /// Any non-fatal warnings produced during assembly.
    pub warnings: Vec<String>,
}

/// Error returned by host operations.
#[derive(Debug)]
pub enum HostError {
    /// Session has not launched any program yet.
    NotLaunched,
    /// Session has been terminated.
    Terminated,
    /// Operation requires paused control state.
    NotPaused,
    /// Requested memory window is out of bounds.
    MemoryWindowOutOfBounds,
    /// Assembly failed.
    Assemble(Box<AssembleError>),
}

impl std::fmt::Display for HostError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotLaunched => write!(f, "program not launched"),
            Self::Terminated => write!(f, "session is terminated"),
            Self::NotPaused => write!(f, "operation requires paused session"),
            Self::MemoryWindowOutOfBounds => write!(f, "memory window out of bounds"),
            Self::Assemble(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for HostError {}

impl From<AssembleError> for HostError {
    fn from(value: AssembleError) -> Self {
        Self::Assemble(Box::new(value))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BuildArtifact {
    binary: Vec<u8>,
    source_map: Vec<LayoutEntry>,
    by_source: BTreeMap<SourceLocation, Vec<u16>>,
    build_id: String,
    layout_id: String,
    source_path: PathBuf,
}

impl BuildArtifact {
    fn from_assemble(source_path: &Path, result: AssembleResult) -> (Self, Vec<String>) {
        let source_map: Vec<LayoutEntry> = result
            .listing
            .iter()
            .map(Self::layout_entry_from_listing)
            .collect();
        let by_source = build_source_index(&source_map);
        let build_id = compute_hash_hex(&result.binary);
        let layout_id = compute_layout_id(&source_map);
        let warnings = result
            .warnings
            .iter()
            .map(std::string::ToString::to_string)
            .collect();

        (
            Self {
                binary: result.binary,
                source_map,
                by_source,
                build_id,
                layout_id,
                source_path: source_path.to_path_buf(),
            },
            warnings,
        )
    }

    fn layout_entry_from_listing(entry: &ListingEntry) -> LayoutEntry {
        LayoutEntry {
            address: entry.address,
            len_bytes: entry.bytes.len(),
            file: entry.file.clone(),
            line: entry.line,
        }
    }
}

#[derive(Debug, Default, Clone)]
struct BreakpointStore {
    by_source: BTreeMap<SourceLocation, Breakpoint>,
    next_id: u32,
}

impl BreakpointStore {
    fn set_for_file(
        &mut self,
        file: &str,
        lines: &[usize],
        artifact: Option<&BuildArtifact>,
    ) -> Vec<Breakpoint> {
        self.by_source.retain(|key, _| key.file != file);

        let deduped_lines: BTreeSet<usize> = lines.iter().copied().collect();
        for line in deduped_lines {
            self.next_id = self.next_id.saturating_add(1);
            let source = SourceLocation {
                file: file.to_string(),
                line,
            };
            let status = resolve_source(&source, artifact, false);
            let breakpoint = Breakpoint {
                id: self.next_id,
                source: source.clone(),
                status,
            };
            self.by_source.insert(source, breakpoint);
        }

        self.by_source
            .values()
            .filter(|bp| bp.source.file == file)
            .cloned()
            .collect()
    }

    fn remap(&mut self, artifact: Option<&BuildArtifact>, layout_changed: bool) {
        for (source, breakpoint) in &mut self.by_source {
            breakpoint.status = resolve_source(source, artifact, layout_changed);
        }
    }

    fn all(&self) -> Vec<Breakpoint> {
        self.by_source.values().cloned().collect()
    }
}

fn resolve_source(
    source: &SourceLocation,
    artifact: Option<&BuildArtifact>,
    layout_changed: bool,
) -> BreakpointStatus {
    let Some(artifact) = artifact else {
        return BreakpointStatus::Unresolved {
            reason: BreakpointUnresolvedReason::NoSourceMatch,
        };
    };

    let Some(addresses) = artifact.by_source.get(source) else {
        return BreakpointStatus::Unresolved {
            reason: if layout_changed {
                BreakpointUnresolvedReason::LayoutChanged
            } else {
                BreakpointUnresolvedReason::NoSourceMatch
            },
        };
    };

    BreakpointStatus::Resolved {
        addresses: addresses.clone(),
    }
}

/// Transport-agnostic debug-session host.
pub struct DebugSessionHost {
    config: CoreConfig,
    state: CoreState,
    mmio: CompositeMmio,
    paused: bool,
    terminated: bool,
    session_id: String,
    active_artifact: Option<BuildArtifact>,
    breakpoints: BreakpointStore,
}

impl Default for DebugSessionHost {
    fn default() -> Self {
        Self::new(CoreConfig::default())
    }
}

impl DebugSessionHost {
    /// Creates a new host session with the provided core config.
    #[must_use]
    pub fn new(config: CoreConfig) -> Self {
        let mmio = CompositeMmio::new().with_tele7(Tele7Peripheral::new(Tele7Config::default()));
        Self {
            state: CoreState::with_config(&config),
            config,
            mmio,
            paused: true,
            terminated: false,
            session_id: format!("{:016x}", NEXT_SESSION_ID.fetch_add(1, Ordering::Relaxed)),
            active_artifact: None,
            breakpoints: BreakpointStore::default(),
        }
    }

    /// Returns current session identifiers.
    #[must_use]
    pub fn ids(&self) -> SessionIds {
        let (build_id, layout_id) = self.active_artifact.as_ref().map_or_else(
            || (String::new(), String::new()),
            |artifact| (artifact.build_id.clone(), artifact.layout_id.clone()),
        );

        SessionIds {
            debug_session_id: self.session_id.clone(),
            build_id,
            layout_id,
        }
    }

    /// Launches a program from disk using include-aware file assembly.
    ///
    /// This operation always performs a clean run.
    ///
    /// # Errors
    ///
    /// Returns an error when session is terminated or assembly fails.
    pub fn launch(&mut self, source_path: &Path) -> Result<BuildReport, HostError> {
        self.ensure_not_terminated()?;
        let assembled = assemble(source_path)?;
        let (artifact, warnings) = BuildArtifact::from_assemble(source_path, assembled);
        self.apply_clean_run(artifact, false);
        Ok(self.build_report(
            ReloadMode::CleanRun,
            Some(CleanRunReason::Requested),
            warnings,
        ))
    }

    /// Reloads source from disk using the supplied policy.
    ///
    /// # Errors
    ///
    /// Returns an error when session is terminated or no program has been launched.
    pub fn reload(
        &mut self,
        source_path: &Path,
        mode: ReloadMode,
    ) -> Result<BuildReport, HostError> {
        self.ensure_not_terminated()?;
        self.ensure_launched()?;

        match mode {
            ReloadMode::CleanRun => {
                let assembled = assemble(source_path)?;
                let (artifact, warnings) = BuildArtifact::from_assemble(source_path, assembled);
                self.apply_clean_run(artifact, true);
                Ok(self.build_report(
                    ReloadMode::CleanRun,
                    Some(CleanRunReason::Requested),
                    warnings,
                ))
            }
            ReloadMode::HotPatch => self.reload_hot_patch(source_path),
        }
    }

    fn reload_hot_patch(&mut self, source_path: &Path) -> Result<BuildReport, HostError> {
        if !self.paused {
            self.fallback_clean_run(CleanRunReason::HotPatchRequiresPause)
        } else if !self.on_instruction_boundary() {
            self.fallback_clean_run(CleanRunReason::HotPatchRequiresInstructionBoundary)
        } else {
            match assemble(source_path) {
                Ok(assembled) => {
                    let (artifact, warnings) = BuildArtifact::from_assemble(source_path, assembled);
                    let current_build = self
                        .active_artifact
                        .as_ref()
                        .map(|a| a.build_id.as_str())
                        .unwrap_or_default();
                    let current_layout = self
                        .active_artifact
                        .as_ref()
                        .map(|a| a.layout_id.as_str())
                        .unwrap_or_default();

                    if artifact.build_id == current_build {
                        Ok(self.build_report(ReloadMode::HotPatch, None, warnings))
                    } else if artifact.layout_id == current_layout {
                        self.apply_hot_patch(artifact);
                        Ok(self.build_report(ReloadMode::HotPatch, None, warnings))
                    } else {
                        self.apply_clean_run(artifact, true);
                        Ok(self.build_report(
                            ReloadMode::CleanRun,
                            Some(CleanRunReason::HotPatchLayoutIncompatible),
                            warnings,
                        ))
                    }
                }
                Err(_) => self.fallback_clean_run(CleanRunReason::HotPatchAssemblyFailed),
            }
        }
    }

    fn fallback_clean_run(&mut self, reason: CleanRunReason) -> Result<BuildReport, HostError> {
        let Some(existing_path) = self.active_artifact.as_ref().map(|a| a.source_path.clone())
        else {
            return Err(HostError::NotLaunched);
        };
        let assembled = assemble(&existing_path)?;
        let (artifact, warnings) = BuildArtifact::from_assemble(&existing_path, assembled);
        self.apply_clean_run(artifact, true);
        Ok(self.build_report(ReloadMode::CleanRun, Some(reason), warnings))
    }

    fn apply_hot_patch(&mut self, artifact: BuildArtifact) {
        let Some(current) = &self.active_artifact else {
            self.apply_clean_run(artifact, false);
            return;
        };

        let rom_limit = usize::from(ROM_END) + 1;
        let max_len = current
            .binary
            .len()
            .max(artifact.binary.len())
            .min(rom_limit);

        for index in 0..max_len {
            let old_value = current.binary.get(index).copied().unwrap_or_default();
            let new_value = artifact.binary.get(index).copied().unwrap_or_default();
            if old_value != new_value {
                self.state.memory[index] = new_value;
            }
        }

        self.active_artifact = Some(artifact);
        self.breakpoints.remap(self.active_artifact.as_ref(), false);
    }

    fn apply_clean_run(&mut self, artifact: BuildArtifact, remap_as_layout_change: bool) {
        let layout_changed = self
            .active_artifact
            .as_ref()
            .is_some_and(|old| old.layout_id != artifact.layout_id);

        self.state = CoreState::with_config(&self.config);
        self.mmio = CompositeMmio::new().with_tele7(Tele7Peripheral::new(Tele7Config::default()));

        let len = artifact.binary.len().min(self.state.memory.len());
        self.state.memory[..len].copy_from_slice(&artifact.binary[..len]);

        self.paused = true;
        self.active_artifact = Some(artifact);
        self.breakpoints.remap(
            self.active_artifact.as_ref(),
            remap_as_layout_change && layout_changed,
        );
    }

    /// Pauses host execution control.
    ///
    /// # Errors
    ///
    /// Returns an error when session is terminated or not launched.
    pub fn pause(&mut self) -> Result<(), HostError> {
        self.ensure_not_terminated()?;
        self.ensure_launched()?;
        self.paused = true;
        Ok(())
    }

    /// Continues execution until the selected boundary.
    ///
    /// Host control transitions to running mode before this call.
    ///
    /// # Errors
    ///
    /// Returns an error when session is terminated or not launched.
    pub fn continue_execution(&mut self, boundary: RunBoundary) -> Result<RunOutcome, HostError> {
        self.ensure_not_terminated()?;
        self.ensure_launched()?;
        self.resume_from_halted();
        self.paused = false;
        Ok(run_one(
            &mut self.state,
            &mut self.mmio,
            &self.config,
            boundary,
        ))
    }

    /// Executes one instruction while paused.
    ///
    /// # Errors
    ///
    /// Returns an error when not paused, terminated, or not launched.
    pub fn instruction_step(&mut self) -> Result<StepOutcome, HostError> {
        self.ensure_not_terminated()?;
        self.ensure_launched()?;
        if !self.paused {
            return Err(HostError::NotPaused);
        }
        self.resume_from_halted();
        Ok(step_one(&mut self.state, &mut self.mmio, &self.config))
    }

    /// Executes one tick while paused.
    ///
    /// # Errors
    ///
    /// Returns an error when not paused, terminated, or not launched.
    pub fn tick_step(&mut self) -> Result<RunOutcome, HostError> {
        self.ensure_not_terminated()?;
        self.ensure_launched()?;
        if !self.paused {
            return Err(HostError::NotPaused);
        }

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
        Ok(outcome)
    }

    /// Returns a full host-visible snapshot.
    ///
    /// # Errors
    ///
    /// Returns an error when session is terminated.
    pub fn read_state(&self) -> Result<HostStateSnapshot, HostError> {
        self.ensure_not_terminated()?;
        Ok(HostStateSnapshot {
            ids: self.ids(),
            state: self.state.clone(),
            paused: self.paused,
            terminated: self.terminated,
        })
    }

    /// Reads a bounded memory window from the active state.
    ///
    /// # Errors
    ///
    /// Returns an error when session is terminated, not launched, or out of bounds.
    pub fn read_memory_window(&self, start: u16, len: usize) -> Result<MemoryWindow, HostError> {
        self.ensure_not_terminated()?;
        self.ensure_launched()?;

        let start_idx = usize::from(start);
        let end = start_idx.saturating_add(len);
        if end > self.state.memory.len() {
            return Err(HostError::MemoryWindowOutOfBounds);
        }

        Ok(MemoryWindow {
            start,
            bytes: self.state.memory[start_idx..end].to_vec(),
        })
    }

    /// Returns a visualization-focused snapshot for attach clients.
    ///
    /// Disassembly rows are centered around the current PC and include the
    /// requested `before` and `after` instruction counts.
    ///
    /// # Errors
    ///
    /// Returns an error when session is terminated or not launched.
    pub fn read_visualizer_snapshot(
        &self,
        before: usize,
        after: usize,
    ) -> Result<VisualizerSnapshot, HostError> {
        self.ensure_not_terminated()?;
        self.ensure_launched()?;

        let pc = self.state.arch.pc();
        let disassembly = disassemble_window(pc, before, after, &self.state.memory);
        let tele7 = self.mmio.tele7().map(|tele7| Tele7VisualizerSnapshot {
            enabled: tele7.state().is_enabled(),
            blink_phase: tele7.state().blink_phase(),
            border_color: tele7.state().border_color(),
            buffer: tele7.get_display_buffer(&self.state.memory),
        });

        Ok(VisualizerSnapshot {
            ids: self.ids(),
            state: self.state.clone(),
            memory: self.state.memory.to_vec(),
            disassembly,
            tele7,
        })
    }

    /// Replaces all breakpoints for a file using source-first anchors.
    ///
    /// # Errors
    ///
    /// Returns an error when session is terminated.
    pub fn set_breakpoints(
        &mut self,
        file: &str,
        lines: &[usize],
    ) -> Result<Vec<Breakpoint>, HostError> {
        self.ensure_not_terminated()?;
        Ok(self
            .breakpoints
            .set_for_file(file, lines, self.active_artifact.as_ref()))
    }

    /// Returns all breakpoints in source sort order.
    #[must_use]
    pub fn breakpoints(&self) -> Vec<Breakpoint> {
        self.breakpoints.all()
    }

    /// Terminates this session host.
    pub const fn terminate(&mut self) {
        self.terminated = true;
        self.paused = true;
    }

    fn build_report(
        &self,
        applied_mode: ReloadMode,
        clean_run_reason: Option<CleanRunReason>,
        warnings: Vec<String>,
    ) -> BuildReport {
        BuildReport {
            applied_mode,
            clean_run_reason,
            ids: self.ids(),
            source_map: self
                .active_artifact
                .as_ref()
                .map_or_else(Vec::new, |a| a.source_map.clone()),
            warnings,
        }
    }

    const fn ensure_launched(&self) -> Result<(), HostError> {
        if self.active_artifact.is_none() {
            Err(HostError::NotLaunched)
        } else {
            Ok(())
        }
    }

    const fn ensure_not_terminated(&self) -> Result<(), HostError> {
        if self.terminated {
            Err(HostError::Terminated)
        } else {
            Ok(())
        }
    }

    const fn on_instruction_boundary(&self) -> bool {
        self.state.arch.pc().is_multiple_of(2)
    }

    #[allow(clippy::missing_const_for_fn)]
    fn resume_from_halted(&mut self) {
        if matches!(self.state.run_state, RunState::HaltedForTick) {
            self.state.arch.set_tick(0);
            self.state.run_state = RunState::Running;
        }
    }
}

fn build_source_index(source_map: &[LayoutEntry]) -> BTreeMap<SourceLocation, Vec<u16>> {
    let mut map: BTreeMap<SourceLocation, Vec<u16>> = BTreeMap::new();
    for entry in source_map {
        let key = SourceLocation {
            file: entry.file.clone(),
            line: entry.line,
        };
        map.entry(key).or_default().push(entry.address);
    }

    for addresses in map.values_mut() {
        addresses.sort_unstable();
        addresses.dedup();
    }
    map
}

fn compute_layout_id(source_map: &[LayoutEntry]) -> String {
    let mut bytes = Vec::new();
    for entry in source_map {
        bytes.extend_from_slice(&entry.address.to_le_bytes());
        bytes.extend_from_slice(&(entry.len_bytes as u64).to_le_bytes());
        bytes.extend_from_slice(entry.file.as_bytes());
        bytes.push(0xFF);
        bytes.extend_from_slice(&(entry.line as u64).to_le_bytes());
        bytes.push(0x00);
    }
    compute_hash_hex(&bytes)
}

fn compute_hash_hex(bytes: &[u8]) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{
        BreakpointStatus, BreakpointUnresolvedReason, CleanRunReason, DebugSessionHost, HostError,
        ReloadMode,
    };
    use emulator_core::{CoreConfig, RunBoundary};

    fn write_file(path: &std::path::Path, contents: &str) {
        fs::write(path, contents).expect("test source write must succeed");
    }

    #[test]
    fn launch_reports_real_source_lines() {
        let tmp = tempfile::tempdir().expect("tempdir must be created");
        let src = tmp.path().join("prog.n1");
        write_file(&src, "; comment\nNOP\nHALT\n");

        let mut host = DebugSessionHost::new(CoreConfig::default());
        let report = host.launch(&src).expect("launch should succeed");

        assert_eq!(report.source_map.len(), 2);
        assert_eq!(report.source_map[0].line, 2);
        assert_eq!(report.source_map[1].line, 3);
    }

    #[test]
    fn hot_patch_requires_pause_and_falls_back_clean() {
        let tmp = tempfile::tempdir().expect("tempdir must be created");
        let src = tmp.path().join("prog.n1");
        write_file(&src, "NOP\nHALT\n");

        let mut host = DebugSessionHost::new(CoreConfig::default());
        host.launch(&src).expect("launch should succeed");

        let _ = host
            .continue_execution(RunBoundary::TickBoundary)
            .expect("continue should succeed");

        let report = host
            .reload(&src, ReloadMode::HotPatch)
            .expect("hot patch fallback should succeed");

        assert_eq!(report.applied_mode, ReloadMode::CleanRun);
        assert_eq!(
            report.clean_run_reason,
            Some(CleanRunReason::HotPatchRequiresPause)
        );
    }

    #[test]
    fn layout_change_marks_unresolved_breakpoints() {
        let tmp = tempfile::tempdir().expect("tempdir must be created");
        let v1 = tmp.path().join("prog_v1.n1");
        let v2 = tmp.path().join("prog_v2.n1");

        write_file(&v1, "NOP\nHALT\n");
        write_file(&v2, "NOP\nNOP\nHALT\n");

        let mut host = DebugSessionHost::new(CoreConfig::default());
        host.launch(&v1).expect("launch should succeed");

        let file = v1.to_string_lossy().to_string();
        let bps = host
            .set_breakpoints(&file, &[2, 3])
            .expect("set breakpoints should succeed");
        assert_eq!(bps.len(), 2);

        let report = host
            .reload(&v2, ReloadMode::HotPatch)
            .expect("reload should succeed");
        assert_eq!(report.applied_mode, ReloadMode::CleanRun);
        assert_eq!(
            report.clean_run_reason,
            Some(CleanRunReason::HotPatchLayoutIncompatible)
        );

        let updated = host.breakpoints();
        assert!(matches!(
            updated[1].status,
            BreakpointStatus::Unresolved {
                reason: BreakpointUnresolvedReason::LayoutChanged
            }
        ));
    }

    #[test]
    fn deterministic_repeated_patch_cycles() {
        let tmp = tempfile::tempdir().expect("tempdir must be created");
        let v1 = tmp.path().join("prog_v1.n1");
        let v2 = tmp.path().join("prog_v2.n1");

        write_file(&v1, "MOV R0, #1\nHALT\n");
        write_file(&v2, "MOV R0, #2\nHALT\n");

        let mut host_a = DebugSessionHost::new(CoreConfig::default());
        let mut host_b = DebugSessionHost::new(CoreConfig::default());

        host_a.launch(&v1).expect("launch should succeed");
        host_b.launch(&v1).expect("launch should succeed");

        for idx in 0..8 {
            let target = if idx % 2 == 0 { &v2 } else { &v1 };
            host_a
                .reload(target, ReloadMode::HotPatch)
                .expect("hot patch should succeed");
            host_b
                .reload(target, ReloadMode::HotPatch)
                .expect("hot patch should succeed");

            let _ = host_a
                .instruction_step()
                .expect("instruction step should succeed");
            let _ = host_b
                .instruction_step()
                .expect("instruction step should succeed");
        }

        let state_a = host_a.read_state().expect("state read should succeed");
        let state_b = host_b.read_state().expect("state read should succeed");

        assert_eq!(state_a.state, state_b.state);
    }

    #[test]
    fn read_memory_window_validates_bounds() {
        let tmp = tempfile::tempdir().expect("tempdir must be created");
        let src = tmp.path().join("prog.n1");
        write_file(&src, "NOP\nHALT\n");

        let mut host = DebugSessionHost::new(CoreConfig::default());
        host.launch(&src).expect("launch should succeed");

        let err = host
            .read_memory_window(0xFFFE, 4)
            .expect_err("window should fail");
        assert!(matches!(err, HostError::MemoryWindowOutOfBounds));
    }

    #[test]
    fn conformance_visualizer_snapshot_tracks_active_state() {
        let tmp = tempfile::tempdir().expect("tempdir must be created");
        let src = tmp.path().join("prog.n1");
        write_file(&src, "NOP\nHALT\n");

        let mut host = DebugSessionHost::new(CoreConfig::default());
        host.launch(&src).expect("launch should succeed");

        let snapshot = host
            .read_visualizer_snapshot(3, 3)
            .expect("visualizer snapshot should succeed");

        assert_eq!(snapshot.state.arch.pc(), 0);
        assert_eq!(snapshot.memory.len(), 65_536);
        assert!(!snapshot.disassembly.is_empty());
        assert!(snapshot.tele7.is_some());
    }

    #[test]
    fn hardening_visualizer_snapshot_is_deterministic_after_patch_cycles() {
        let tmp = tempfile::tempdir().expect("tempdir must be created");
        let v1 = tmp.path().join("prog_v1.n1");
        let v2 = tmp.path().join("prog_v2.n1");

        write_file(&v1, "MOV R0, #1\nHALT\n");
        write_file(&v2, "MOV R0, #2\nHALT\n");

        let mut host_a = DebugSessionHost::new(CoreConfig::default());
        let mut host_b = DebugSessionHost::new(CoreConfig::default());
        host_a.launch(&v1).expect("launch should succeed");
        host_b.launch(&v1).expect("launch should succeed");

        for idx in 0..6 {
            let target = if idx % 2 == 0 { &v2 } else { &v1 };
            host_a
                .reload(target, ReloadMode::HotPatch)
                .expect("reload should succeed");
            host_b
                .reload(target, ReloadMode::HotPatch)
                .expect("reload should succeed");

            let _ = host_a
                .instruction_step()
                .expect("instruction step should succeed");
            let _ = host_b
                .instruction_step()
                .expect("instruction step should succeed");
        }

        let snapshot_a = host_a
            .read_visualizer_snapshot(4, 4)
            .expect("snapshot should succeed");
        let snapshot_b = host_b
            .read_visualizer_snapshot(4, 4)
            .expect("snapshot should succeed");

        assert_eq!(snapshot_a.state, snapshot_b.state);
        assert_eq!(snapshot_a.memory, snapshot_b.memory);
        assert_eq!(snapshot_a.disassembly, snapshot_b.disassembly);
        assert_eq!(snapshot_a.tele7, snapshot_b.tele7);
    }

    #[test]
    fn hot_patch_no_op_when_build_id_unchanged() {
        let tmp = tempfile::tempdir().expect("tempdir must be created");
        let src = tmp.path().join("prog.n1");
        write_file(&src, "MOV R0, #1\nHALT\n");

        let mut host = DebugSessionHost::new(CoreConfig::default());
        host.launch(&src).expect("launch should succeed");

        let _ = host
            .instruction_step()
            .expect("instruction step should succeed");
        let state_before = host.read_state().expect("state read should succeed");
        let ids_before = state_before.ids.clone();

        let report = host
            .reload(&src, ReloadMode::HotPatch)
            .expect("hot patch should succeed");
        let state_after = host.read_state().expect("state read should succeed");

        assert_eq!(report.applied_mode, ReloadMode::HotPatch);
        assert_eq!(state_before.state, state_after.state);
        assert_eq!(ids_before, state_after.ids);
    }
}
