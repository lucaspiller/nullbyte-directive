#![forbid(unsafe_code)]
#![allow(clippy::multiple_crate_versions)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::large_enum_variant)]
#![allow(clippy::manual_let_else)]
#![allow(clippy::map_unwrap_or)]
#![allow(clippy::match_same_arms)]
#![allow(clippy::missing_const_for_fn)]
#![allow(clippy::needless_pass_by_ref_mut)]
#![allow(clippy::needless_pass_by_value)]
#![allow(clippy::significant_drop_tightening)]
#![allow(clippy::single_match)]
#![allow(clippy::single_match_else)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::unnested_or_patterns)]
#![allow(clippy::unused_self)]
//! Rust-native Debug Adapter Protocol server for Nullbyte Directive.
//!
//! This binary owns DAP stdin/stdout handling and serves a read-only HTTP
//! snapshot endpoint for debug-tool attach mode.

use std::env;
use std::io::{self, BufReader, BufWriter};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::Duration;

use assembler as _;
use dap::events::{Event, OutputEventBody, StoppedEventBody};
use dap::requests::{
    Command, LaunchRequestArguments, Request, RestartArguments, SetBreakpointsArguments,
};
use dap::responses::{
    ContinueResponse, Response, ResponseBody, ScopesResponse, SetBreakpointsResponse,
    ThreadsResponse, VariablesResponse,
};
use dap::server::Server;
use dap::types::{
    Breakpoint, Capabilities, OutputEventCategory, Scope, Source, StackFrame, StoppedEventReason,
    Thread as DapThread, Variable,
};
use emulator_core::{CoreConfig, GeneralRegister, StepOutcome};
use emulator_debug_session::{
    Breakpoint as HostBreakpoint, BreakpointStatus, BreakpointUnresolvedReason, CleanRunReason,
    ContinueStop, DebugSessionHost, HostError, LayoutEntry, ReloadMode,
};
use serde as _;
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::runtime::{Builder, Handle};

const THREAD_ID: i64 = 1;
const STACK_FRAME_ID: i64 = 100;
const REG_SCOPE_ID: i64 = 200;
const MEM_SCOPE_ID: i64 = 201;
const DEFAULT_HTTP_PORT: u16 = 48_765;

#[derive(Debug)]
enum ReaderMessage {
    Request(Request),
    ProtocolError(String),
}

#[derive(Debug)]
enum ContinueMessage {
    Stopped(ContinueStop),
    Error(String),
}

#[derive(Debug)]
struct ActiveContinue {
    stop_flag: Arc<AtomicBool>,
}

struct DapAdapter {
    host: Arc<Mutex<DebugSessionHost>>,
    runtime_handle: Handle,
    continue_tx: mpsc::Sender<ContinueMessage>,
    continue_rx: mpsc::Receiver<ContinueMessage>,
    launch_program: Option<PathBuf>,
    last_layout_id: Option<String>,
    active_continue: Option<ActiveContinue>,
}

impl DapAdapter {
    fn new(
        host: Arc<Mutex<DebugSessionHost>>,
        runtime_handle: Handle,
        continue_tx: mpsc::Sender<ContinueMessage>,
        continue_rx: mpsc::Receiver<ContinueMessage>,
    ) -> Self {
        Self {
            host,
            runtime_handle,
            continue_tx,
            continue_rx,
            launch_program: None,
            last_layout_id: None,
            active_continue: None,
        }
    }

    fn drain_continue_results<W>(&mut self, server: &mut Server<io::Empty, W>) -> io::Result<()>
    where
        W: io::Write,
    {
        loop {
            match self.continue_rx.try_recv() {
                Ok(ContinueMessage::Stopped(stop)) => {
                    self.active_continue = None;
                    match stop {
                        ContinueStop::Breakpoint => {
                            self.send_stopped(server, StoppedEventReason::Breakpoint)?;
                        }
                        ContinueStop::PauseRequested | ContinueStop::HaltedForTick => {
                            self.send_stopped(server, StoppedEventReason::Pause)?;
                        }
                        ContinueStop::Fault { cause } => {
                            self.send_output(
                                server,
                                OutputEventCategory::Stderr,
                                &format!("fault: {cause:?}"),
                            )?;
                            self.send_stopped(server, StoppedEventReason::Exception)?;
                        }
                    }
                }
                Ok(ContinueMessage::Error(message)) => {
                    self.active_continue = None;
                    self.send_output(server, OutputEventCategory::Stderr, &message)?;
                    self.send_stopped(server, StoppedEventReason::Exception)?;
                }
                Err(mpsc::TryRecvError::Empty) | Err(mpsc::TryRecvError::Disconnected) => {
                    return Ok(())
                }
            }
        }
    }

    fn handle_request<W>(
        &mut self,
        server: &mut Server<io::Empty, W>,
        request: Request,
    ) -> io::Result<bool>
    where
        W: io::Write,
    {
        match request.command.clone() {
            Command::Initialize(_) => {
                self.respond(
                    server,
                    request.success(ResponseBody::Initialize(Capabilities {
                        supports_configuration_done_request: Some(true),
                        supports_terminate_request: Some(true),
                        supports_restart_request: Some(true),
                        supports_single_thread_execution_requests: Some(true),
                        ..Capabilities::default()
                    })),
                )?;
                self.send_event(server, Event::Initialized)?;
                Ok(true)
            }
            Command::ConfigurationDone => {
                self.respond(server, request.success(ResponseBody::ConfigurationDone))?;
                Ok(true)
            }
            Command::Launch(args) => {
                let args = parse_launch_args(&args)
                    .map_err(|msg| io::Error::new(io::ErrorKind::InvalidInput, msg))?;
                let program_path = resolve_program_path(&args.program, args.cwd.as_deref());

                let report = {
                    let mut host = lock_host(&self.host)?;
                    host.launch(&program_path).map_err(host_error_to_io)?
                };

                self.launch_program = Some(program_path);
                self.respond(server, request.success(ResponseBody::Launch))?;
                self.send_output(
                    server,
                    OutputEventCategory::Console,
                    &format!(
                        "session={} build={} layout={}",
                        report.ids.debug_session_id, report.ids.build_id, report.ids.layout_id
                    ),
                )?;

                if self
                    .last_layout_id
                    .as_ref()
                    .is_some_and(|previous| previous != &report.ids.layout_id)
                {
                    self.send_output(
                        server,
                        OutputEventCategory::Stderr,
                        &format!(
                            "layout changed: {} -> {}; breakpoints may be remapped or unresolved",
                            self.last_layout_id.as_deref().unwrap_or_default(),
                            report.ids.layout_id
                        ),
                    )?;
                }
                self.last_layout_id = Some(report.ids.layout_id.clone());

                for warning in report.warnings {
                    self.send_output(server, OutputEventCategory::Console, &warning)?;
                }

                self.send_stopped(server, StoppedEventReason::Entry)?;
                Ok(true)
            }
            Command::SetBreakpoints(args) => {
                let (file, lines) = parse_set_breakpoints(&args)
                    .map_err(|msg| io::Error::new(io::ErrorKind::InvalidInput, msg))?;

                let mapped = {
                    let mut host = lock_host(&self.host)?;
                    host.set_breakpoints(&file, &lines)
                        .map_err(host_error_to_io)?
                        .into_iter()
                        .map(to_dap_breakpoint)
                        .collect::<Vec<_>>()
                };

                self.respond(
                    server,
                    request.success(ResponseBody::SetBreakpoints(SetBreakpointsResponse {
                        breakpoints: mapped.clone(),
                    })),
                )?;

                for breakpoint in mapped {
                    if !breakpoint.verified {
                        if let Some(message) = breakpoint.message {
                            let line = breakpoint.line.unwrap_or_default();
                            self.send_output(
                                server,
                                OutputEventCategory::Stderr,
                                &format!("breakpoint unresolved at line {line}: {message}"),
                            )?;
                        }
                    }
                }
                Ok(true)
            }
            Command::Continue(_) => {
                if self.active_continue.is_some() {
                    self.respond(server, request.error("continue already in progress"))?;
                    return Ok(true);
                }

                let stop_flag = Arc::new(AtomicBool::new(false));
                let host = Arc::clone(&self.host);
                let continue_tx = self.continue_tx.clone();
                let continue_stop = Arc::clone(&stop_flag);
                self.runtime_handle.spawn_blocking(move || {
                    let mut host = match host.lock() {
                        Ok(host) => host,
                        Err(_) => {
                            let _ = continue_tx
                                .send(ContinueMessage::Error("host mutex poisoned".to_string()));
                            return;
                        }
                    };

                    let result = host
                        .continue_until_stop(|| continue_stop.load(Ordering::Relaxed))
                        .map(ContinueMessage::Stopped)
                        .unwrap_or_else(|err| ContinueMessage::Error(host_error_to_string(err)));
                    let _ = continue_tx.send(result);
                });

                self.active_continue = Some(ActiveContinue { stop_flag });
                self.respond(
                    server,
                    request.success(ResponseBody::Continue(ContinueResponse {
                        all_threads_continued: Some(true),
                    })),
                )?;
                Ok(true)
            }
            Command::Next(_) => {
                if self.active_continue.is_some() {
                    self.respond(server, request.error("continue is in progress"))?;
                    return Ok(true);
                }

                let outcome = {
                    let mut host = lock_host(&self.host)?;
                    host.instruction_step().map_err(host_error_to_io)?
                };

                self.respond(server, request.success(ResponseBody::Next))?;
                self.send_stopped(server, step_outcome_reason(outcome))?;
                Ok(true)
            }
            Command::Pause(_) => {
                if let Some(active) = &self.active_continue {
                    active.stop_flag.store(true, Ordering::Relaxed);
                    self.respond(server, request.success(ResponseBody::Pause))?;
                    return Ok(true);
                }

                {
                    let mut host = lock_host(&self.host)?;
                    host.pause().map_err(host_error_to_io)?;
                }
                self.respond(server, request.success(ResponseBody::Pause))?;
                self.send_stopped(server, StoppedEventReason::Pause)?;
                Ok(true)
            }
            Command::StackTrace(_) => {
                let (pc, source) = {
                    let host = lock_host(&self.host)?;
                    let state = host.read_state().map_err(host_error_to_io)?;
                    let pc = state.state.arch.pc();
                    (pc, host.source_for_pc(pc))
                };

                self.respond(
                    server,
                    request.success(ResponseBody::StackTrace(
                        dap::responses::StackTraceResponse {
                            stack_frames: vec![stack_frame_for_pc(pc, source)],
                            total_frames: Some(1),
                        },
                    )),
                )?;
                Ok(true)
            }
            Command::Scopes(_) => {
                self.respond(
                    server,
                    request.success(ResponseBody::Scopes(ScopesResponse {
                        scopes: vec![
                            Scope {
                                name: "Registers".to_string(),
                                presentation_hint: None,
                                variables_reference: REG_SCOPE_ID,
                                named_variables: None,
                                indexed_variables: None,
                                expensive: false,
                                source: None,
                                line: None,
                                column: None,
                                end_line: None,
                                end_column: None,
                            },
                            Scope {
                                name: "Memory".to_string(),
                                presentation_hint: None,
                                variables_reference: MEM_SCOPE_ID,
                                named_variables: None,
                                indexed_variables: None,
                                expensive: false,
                                source: None,
                                line: None,
                                column: None,
                                end_line: None,
                                end_column: None,
                            },
                        ],
                    })),
                )?;
                Ok(true)
            }
            Command::Variables(args) => {
                let variables = if args.variables_reference == REG_SCOPE_ID {
                    let state = {
                        let host = lock_host(&self.host)?;
                        host.read_state().map_err(host_error_to_io)?
                    };
                    registers_to_variables(&state.state.arch)
                } else if args.variables_reference == MEM_SCOPE_ID {
                    let memory = {
                        let host = lock_host(&self.host)?;
                        host.read_memory_window(0, 32).map_err(host_error_to_io)?
                    };
                    memory_to_variables(memory.start, &memory.bytes)
                } else {
                    Vec::new()
                };

                self.respond(
                    server,
                    request.success(ResponseBody::Variables(VariablesResponse { variables })),
                )?;
                Ok(true)
            }
            Command::Threads => {
                self.respond(
                    server,
                    request.success(ResponseBody::Threads(ThreadsResponse {
                        threads: vec![DapThread {
                            id: THREAD_ID,
                            name: "Nullbyte Core".to_string(),
                        }],
                    })),
                )?;
                Ok(true)
            }
            Command::Restart(args) => {
                if self.active_continue.is_some() {
                    self.respond(server, request.error("cannot reload while running"))?;
                    return Ok(true);
                }

                let mode = restart_mode(&args);
                let target = self
                    .launch_program
                    .clone()
                    .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no active launch"))?;

                let report = {
                    let mut host = lock_host(&self.host)?;
                    host.reload(&target, mode).map_err(host_error_to_io)?
                };

                self.respond(server, request.success(ResponseBody::Restart))?;

                if self
                    .last_layout_id
                    .as_ref()
                    .is_some_and(|previous| previous != &report.ids.layout_id)
                {
                    self.send_output(
                        server,
                        OutputEventCategory::Stderr,
                        &format!(
                            "layout changed: {} -> {}; breakpoints may be remapped or unresolved",
                            self.last_layout_id.as_deref().unwrap_or_default(),
                            report.ids.layout_id
                        ),
                    )?;
                }
                self.last_layout_id = Some(report.ids.layout_id.clone());

                for warning in report.warnings {
                    self.send_output(server, OutputEventCategory::Console, &warning)?;
                }

                let applied = reload_mode_name(report.applied_mode);
                let reason = report.clean_run_reason.map(clean_run_reason_name);
                let message = if report.applied_mode == ReloadMode::HotPatch {
                    format!(
                        "hot reload applied (mode={applied}, build={}, layout={})",
                        report.ids.build_id, report.ids.layout_id
                    )
                } else {
                    format!(
                        "reload fell back to clean run ({})",
                        reason.unwrap_or_else(|| "unknown_reason".to_string())
                    )
                };
                self.send_output(
                    server,
                    if report.applied_mode == ReloadMode::HotPatch {
                        OutputEventCategory::Console
                    } else {
                        OutputEventCategory::Stderr
                    },
                    &message,
                )?;
                Ok(true)
            }
            Command::Disconnect(_) => {
                if let Some(active) = &self.active_continue {
                    active.stop_flag.store(true, Ordering::Relaxed);
                }

                {
                    let mut host = lock_host(&self.host)?;
                    host.terminate();
                }

                self.respond(server, request.success(ResponseBody::Disconnect))?;
                self.send_event(server, Event::Terminated(None))?;
                Ok(false)
            }
            _ => {
                self.respond(server, request.error("unsupported request"))?;
                Ok(true)
            }
        }
    }

    fn respond<W>(
        &mut self,
        server: &mut Server<io::Empty, W>,
        response: Response,
    ) -> io::Result<()>
    where
        W: io::Write,
    {
        server.respond(response).map_err(server_error_to_io)
    }

    fn send_event<W>(&mut self, server: &mut Server<io::Empty, W>, event: Event) -> io::Result<()>
    where
        W: io::Write,
    {
        server.send_event(event).map_err(server_error_to_io)
    }

    fn send_output<W>(
        &mut self,
        server: &mut Server<io::Empty, W>,
        category: OutputEventCategory,
        output: &str,
    ) -> io::Result<()>
    where
        W: io::Write,
    {
        self.send_event(
            server,
            Event::Output(OutputEventBody {
                category: Some(category),
                output: format!("{output}\n"),
                group: None,
                variables_reference: None,
                source: None,
                line: None,
                column: None,
                data: None,
            }),
        )
    }

    fn send_stopped<W>(
        &mut self,
        server: &mut Server<io::Empty, W>,
        reason: StoppedEventReason,
    ) -> io::Result<()>
    where
        W: io::Write,
    {
        self.send_event(
            server,
            Event::Stopped(StoppedEventBody {
                reason,
                description: None,
                thread_id: Some(THREAD_ID),
                preserve_focus_hint: None,
                text: None,
                all_threads_stopped: Some(true),
                hit_breakpoint_ids: None,
            }),
        )
    }
}

#[derive(Debug)]
struct LaunchArgs {
    program: String,
    cwd: Option<String>,
}

fn parse_launch_args(args: &LaunchRequestArguments) -> Result<LaunchArgs, String> {
    let map = args
        .additional_data
        .as_ref()
        .and_then(Value::as_object)
        .ok_or_else(|| "launch requires additional data".to_string())?;

    let Some(program) = map
        .get("program")
        .and_then(Value::as_str)
        .map(ToString::to_string)
    else {
        return Err("launch requires 'program'".to_string());
    };

    let cwd = map
        .get("cwd")
        .and_then(Value::as_str)
        .map(ToString::to_string);

    Ok(LaunchArgs { program, cwd })
}

fn parse_set_breakpoints(args: &SetBreakpointsArguments) -> Result<(String, Vec<usize>), String> {
    let Some(path) = args.source.path.clone() else {
        return Err("setBreakpoints requires source.path".to_string());
    };

    let mut lines = Vec::new();
    if let Some(source_breakpoints) = &args.breakpoints {
        for breakpoint in source_breakpoints {
            if breakpoint.line > 0 {
                lines.push(breakpoint.line as usize);
            }
        }
    }

    if lines.is_empty() {
        #[allow(deprecated)]
        if let Some(request_lines) = &args.lines {
            for line in request_lines {
                if *line > 0 {
                    lines.push(*line as usize);
                }
            }
        }
    }

    Ok((path, lines))
}

fn restart_mode(args: &RestartArguments) -> ReloadMode {
    let Some(mode) = args
        .arguments
        .as_ref()
        .and_then(|payload| payload.additional_data.as_ref())
        .and_then(Value::as_object)
        .and_then(|map| map.get("mode"))
        .and_then(Value::as_str)
    else {
        return ReloadMode::HotPatch;
    };

    parse_reload_mode(mode)
}

fn to_dap_breakpoint(breakpoint: HostBreakpoint) -> Breakpoint {
    let source = Source {
        name: file_name_hint(&breakpoint.source.file),
        path: Some(breakpoint.source.file),
        source_reference: None,
        presentation_hint: None,
        origin: None,
        sources: None,
        adapter_data: None,
        checksums: None,
    };

    match breakpoint.status {
        BreakpointStatus::Resolved { .. } => Breakpoint {
            id: Some(i64::from(breakpoint.id)),
            verified: true,
            message: None,
            source: Some(source),
            line: Some(breakpoint.source.line as i64),
            column: None,
            end_line: None,
            end_column: None,
            instruction_reference: None,
            offset: None,
        },
        BreakpointStatus::Unresolved { reason } => Breakpoint {
            id: Some(i64::from(breakpoint.id)),
            verified: false,
            message: Some(match reason {
                BreakpointUnresolvedReason::NoSourceMatch => {
                    "no source mapping for this line".to_string()
                }
                BreakpointUnresolvedReason::LayoutChanged => {
                    "layout changed; breakpoint remap failed".to_string()
                }
            }),
            source: Some(source),
            line: Some(breakpoint.source.line as i64),
            column: None,
            end_line: None,
            end_column: None,
            instruction_reference: None,
            offset: None,
        },
    }
}

fn stack_frame_for_pc(pc: u16, source: Option<LayoutEntry>) -> StackFrame {
    let (line, source) = match source {
        Some(entry) => {
            let source = Source {
                name: file_name_hint(&entry.file),
                path: Some(entry.file),
                source_reference: None,
                presentation_hint: None,
                origin: None,
                sources: None,
                adapter_data: None,
                checksums: None,
            };
            (entry.line as i64, Some(source))
        }
        None => (1, None),
    };

    StackFrame {
        id: STACK_FRAME_ID,
        name: format!("PC=0x{pc:04x}"),
        source,
        line,
        column: 1,
        end_line: None,
        end_column: None,
        can_restart: None,
        instruction_pointer_reference: None,
        module_id: None,
        presentation_hint: None,
    }
}

fn step_outcome_reason(outcome: StepOutcome) -> StoppedEventReason {
    match outcome {
        StepOutcome::Fault { .. } => StoppedEventReason::Exception,
        StepOutcome::HaltedForTick => StoppedEventReason::Pause,
        StepOutcome::Retired { .. }
        | StepOutcome::TrapDispatch { .. }
        | StepOutcome::EventDispatch { .. } => StoppedEventReason::Step,
    }
}

fn registers_to_variables(arch: &emulator_core::ArchitecturalState) -> Vec<Variable> {
    let mut variables = Vec::new();
    for reg in GeneralRegister::ALL {
        variables.push(hex_variable(format!("R{}", reg.index()), arch.gpr(reg), 4));
    }
    variables.push(hex_variable("PC".to_string(), arch.pc(), 4));
    variables.push(hex_variable("SP".to_string(), arch.sp(), 4));
    variables.push(hex_variable("FLAGS".to_string(), arch.flags(), 4));
    variables.push(hex_variable("TICK".to_string(), arch.tick(), 4));
    variables.push(hex_variable("CAP".to_string(), arch.cap(), 4));
    variables.push(hex_variable("CAUSE".to_string(), arch.cause(), 4));
    variables.push(hex_variable("EVP".to_string(), arch.evp(), 4));
    variables
}

fn memory_to_variables(start: u16, bytes: &[u8]) -> Vec<Variable> {
    let mut variables = Vec::new();
    for (index, row) in bytes.chunks(16).enumerate() {
        let address = usize::from(start) + (index * 16);
        let value = row
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<Vec<_>>()
            .join(" ");
        variables.push(Variable {
            name: format!("0x{address:04x}"),
            value,
            type_field: None,
            presentation_hint: None,
            evaluate_name: None,
            variables_reference: 0,
            named_variables: None,
            indexed_variables: None,
            memory_reference: None,
        });
    }
    variables
}

fn hex_variable(name: String, value: u16, width: usize) -> Variable {
    Variable {
        name,
        value: format!("0x{value:0width$x}"),
        type_field: None,
        presentation_hint: None,
        evaluate_name: None,
        variables_reference: 0,
        named_variables: None,
        indexed_variables: None,
        memory_reference: None,
    }
}

fn resolve_program_path(program: &str, cwd: Option<&str>) -> PathBuf {
    let raw = PathBuf::from(program);
    if raw.is_absolute() {
        return raw;
    }

    if let Some(cwd) = cwd {
        return Path::new(cwd).join(raw);
    }

    match env::current_dir() {
        Ok(current_dir) => current_dir.join(raw),
        Err(_) => raw,
    }
}

fn parse_reload_mode(mode: &str) -> ReloadMode {
    match mode {
        "clean_run" | "clean-run" => ReloadMode::CleanRun,
        _ => ReloadMode::HotPatch,
    }
}

fn reload_mode_name(mode: ReloadMode) -> &'static str {
    match mode {
        ReloadMode::CleanRun => "clean_run",
        ReloadMode::HotPatch => "hot_patch",
    }
}

fn clean_run_reason_name(reason: CleanRunReason) -> String {
    match reason {
        CleanRunReason::Requested => "requested".to_string(),
        CleanRunReason::HotPatchRequiresPause => "hot_patch_requires_pause".to_string(),
        CleanRunReason::HotPatchRequiresInstructionBoundary => {
            "hot_patch_requires_instruction_boundary".to_string()
        }
        CleanRunReason::HotPatchAssemblyFailed => "hot_patch_assembly_failed".to_string(),
        CleanRunReason::HotPatchLayoutIncompatible => "hot_patch_layout_incompatible".to_string(),
    }
}

fn file_name_hint(path: &str) -> Option<String> {
    Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .map(ToString::to_string)
}

fn host_error_to_string(err: HostError) -> String {
    match err {
        HostError::Assemble(inner) => format!("assembly error: {inner}"),
        _ => err.to_string(),
    }
}

fn lock_host(
    host: &Arc<Mutex<DebugSessionHost>>,
) -> io::Result<std::sync::MutexGuard<'_, DebugSessionHost>> {
    host.lock()
        .map_err(|_| io::Error::other("host mutex poisoned"))
}

fn host_error_to_io(err: HostError) -> io::Error {
    io::Error::other(host_error_to_string(err))
}

fn server_error_to_io(err: dap::errors::ServerError) -> io::Error {
    io::Error::other(err.to_string())
}

fn spawn_request_reader(tx: mpsc::Sender<ReaderMessage>) {
    thread::spawn(move || {
        let stdin = io::stdin();
        let reader = BufReader::new(stdin.lock());
        let writer = BufWriter::new(io::sink());
        let mut server = Server::new(reader, writer);

        loop {
            match server.poll_request() {
                Ok(Some(request)) => {
                    if tx.send(ReaderMessage::Request(request)).is_err() {
                        break;
                    }
                }
                Ok(None) => break,
                Err(err) => {
                    let _ = tx.send(ReaderMessage::ProtocolError(err.to_string()));
                    break;
                }
            }
        }
    });
}

async fn run_snapshot_server(host: Arc<Mutex<DebugSessionHost>>, shutdown: Arc<AtomicBool>) {
    let port = env::var("NULLBYTE_SESSION_BRIDGE_PORT")
        .ok()
        .and_then(|raw| raw.parse::<u16>().ok())
        .unwrap_or(DEFAULT_HTTP_PORT);
    let bind_addr = format!("127.0.0.1:{port}");

    let listener = match TcpListener::bind(&bind_addr).await {
        Ok(listener) => listener,
        Err(_) => return,
    };

    loop {
        if shutdown.load(Ordering::Relaxed) {
            break;
        }

        match tokio::time::timeout(Duration::from_millis(100), listener.accept()).await {
            Ok(Ok((mut stream, _))) => {
                let host = Arc::clone(&host);
                tokio::spawn(async move {
                    let _ = handle_http_connection(&mut stream, host).await;
                });
            }
            Ok(Err(_)) | Err(_) => {}
        }
    }
}

async fn handle_http_connection(
    stream: &mut tokio::net::TcpStream,
    host: Arc<Mutex<DebugSessionHost>>,
) -> io::Result<()> {
    let mut buffer = vec![0_u8; 8192];
    let mut total = 0_usize;

    loop {
        if total >= buffer.len() {
            break;
        }

        let read = stream.read(&mut buffer[total..]).await?;
        if read == 0 {
            break;
        }
        total += read;

        if buffer[..total]
            .windows(4)
            .any(|window| window == b"\r\n\r\n")
        {
            break;
        }
    }

    let request = String::from_utf8_lossy(&buffer[..total]);
    let first_line = request.lines().next().unwrap_or_default();
    let mut parts = first_line.split_whitespace();
    let method = parts.next().unwrap_or_default();
    let target = parts.next().unwrap_or("/");

    if method == "OPTIONS" {
        write_http_json(stream, 204, &json!({})).await?;
        return Ok(());
    }

    if method != "GET" {
        write_http_json(stream, 405, &json!({ "error": "method not allowed" })).await?;
        return Ok(());
    }

    let (path, query) = split_path_query(target);
    match path {
        "/v1/health" => {
            write_http_json(
                stream,
                200,
                &json!({ "ok": true, "port": DEFAULT_HTTP_PORT }),
            )
            .await?;
        }
        "/v1/snapshot" => {
            let before = parse_natural(query_param(query, "before"), 3);
            let after = parse_natural(query_param(query, "after"), 4);

            let snapshot = {
                let host = lock_host(&host)?;
                host.read_visualizer_snapshot(before, after)
            };

            match snapshot {
                Ok(snapshot) => {
                    write_http_json(
                        stream,
                        200,
                        &json!({
                            "controlOwner": "vscode-dap",
                            "snapshot": snapshot,
                        }),
                    )
                    .await?;
                }
                Err(HostError::NotLaunched) => {
                    write_http_json(
                        stream,
                        409,
                        &json!({
                            "error": "program not launched",
                            "detail": "Start a VS Code Nullbyte Directive launch session first.",
                        }),
                    )
                    .await?;
                }
                Err(err) => {
                    write_http_json(stream, 500, &json!({ "error": host_error_to_string(err) }))
                        .await?;
                }
            }
        }
        _ => {
            write_http_json(stream, 404, &json!({ "error": "not found" })).await?;
        }
    }

    Ok(())
}

fn split_path_query(target: &str) -> (&str, &str) {
    match target.split_once('?') {
        Some((path, query)) => (path, query),
        None => (target, ""),
    }
}

fn query_param<'a>(query: &'a str, key: &str) -> Option<&'a str> {
    query.split('&').find_map(|pair| {
        let (name, value) = pair.split_once('=')?;
        if name == key {
            Some(value)
        } else {
            None
        }
    })
}

fn parse_natural(value: Option<&str>, fallback: usize) -> usize {
    value
        .and_then(|raw| raw.parse::<usize>().ok())
        .unwrap_or(fallback)
}

async fn write_http_json(
    stream: &mut tokio::net::TcpStream,
    status: u16,
    body: &Value,
) -> io::Result<()> {
    let payload = serde_json::to_vec(body).unwrap_or_else(|_| b"{}".to_vec());
    let status_text = match status {
        200 => "OK",
        204 => "No Content",
        404 => "Not Found",
        405 => "Method Not Allowed",
        409 => "Conflict",
        500 => "Internal Server Error",
        _ => "OK",
    };

    let header = format!(
        "HTTP/1.1 {status} {status_text}\r\n\
         content-type: application/json; charset=utf-8\r\n\
         access-control-allow-origin: *\r\n\
         access-control-allow-methods: GET,OPTIONS\r\n\
         access-control-allow-headers: content-type\r\n\
         content-length: {}\r\n\
         connection: close\r\n\
         \r\n",
        payload.len()
    );

    stream.write_all(header.as_bytes()).await?;
    if status != 204 {
        stream.write_all(&payload).await?;
    }
    Ok(())
}

fn main() -> io::Result<()> {
    let runtime = Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|err| io::Error::other(err.to_string()))?;

    let host = Arc::new(Mutex::new(DebugSessionHost::new(CoreConfig::default())));
    let shutdown = Arc::new(AtomicBool::new(false));
    let http_host = Arc::clone(&host);
    let http_shutdown = Arc::clone(&shutdown);
    let http_task = runtime.spawn(async move {
        run_snapshot_server(http_host, http_shutdown).await;
    });

    let (reader_tx, reader_rx) = mpsc::channel();
    spawn_request_reader(reader_tx);

    let (continue_tx, continue_rx) = mpsc::channel();
    let mut adapter = DapAdapter::new(
        Arc::clone(&host),
        runtime.handle().clone(),
        continue_tx,
        continue_rx,
    );

    let stdout = io::stdout();
    let reader = BufReader::new(io::empty());
    let writer = BufWriter::new(stdout.lock());
    let mut dap = Server::new(reader, writer);

    let mut running = true;
    while running {
        adapter.drain_continue_results(&mut dap)?;

        match reader_rx.recv_timeout(Duration::from_millis(20)) {
            Ok(ReaderMessage::Request(request)) => {
                running = adapter.handle_request(&mut dap, request)?;
            }
            Ok(ReaderMessage::ProtocolError(message)) => {
                adapter.send_output(&mut dap, OutputEventCategory::Stderr, &message)?;
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }

    shutdown.store(true, Ordering::Relaxed);
    let _ = runtime
        .block_on(async { tokio::time::timeout(Duration::from_millis(250), http_task).await });

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{
        stack_frame_for_pc, to_dap_breakpoint, ContinueStop, DebugSessionHost, HostBreakpoint,
    };
    use emulator_core::CoreConfig;
    use emulator_debug_session::{BreakpointStatus, BreakpointUnresolvedReason, SourceLocation};

    fn write_file(path: &std::path::Path, contents: &str) {
        fs::write(path, contents).expect("test source write must succeed");
    }

    #[test]
    fn dap_breakpoint_mapping_sets_verified_and_messages() {
        let resolved = to_dap_breakpoint(HostBreakpoint {
            id: 1,
            source: SourceLocation {
                file: "/tmp/main.n1".to_string(),
                line: 5,
            },
            status: BreakpointStatus::Resolved {
                addresses: vec![0x20],
            },
        });
        assert!(resolved.verified);
        assert!(resolved.message.is_none());

        let unresolved = to_dap_breakpoint(HostBreakpoint {
            id: 2,
            source: SourceLocation {
                file: "/tmp/main.n1".to_string(),
                line: 9,
            },
            status: BreakpointStatus::Unresolved {
                reason: BreakpointUnresolvedReason::LayoutChanged,
            },
        });
        assert!(!unresolved.verified);
        assert!(unresolved
            .message
            .as_deref()
            .is_some_and(|msg| msg.contains("layout changed")));
    }

    #[test]
    fn continue_loop_stops_on_breakpoint_hit() {
        let tmp = tempfile::tempdir().expect("tempdir must be created");
        let source = tmp.path().join("prog.n1");
        write_file(&source, "NOP\nNOP\nHALT\n");

        let mut host = DebugSessionHost::new(CoreConfig::default());
        host.launch(&source).expect("launch should succeed");
        host.set_breakpoints(&source.to_string_lossy(), &[2])
            .expect("set breakpoints should succeed");

        let stop = host
            .continue_until_stop(|| false)
            .expect("continue should succeed");

        assert!(matches!(stop, ContinueStop::Breakpoint));
    }

    #[test]
    fn stack_trace_frame_resolves_real_source_line() {
        let tmp = tempfile::tempdir().expect("tempdir must be created");
        let source = tmp.path().join("prog.n1");
        write_file(&source, "; comment\nNOP\nHALT\n");

        let mut host = DebugSessionHost::new(CoreConfig::default());
        host.launch(&source).expect("launch should succeed");

        let source_row = host.source_for_pc(0).expect("source row should resolve");
        let frame = stack_frame_for_pc(0, Some(source_row));

        assert_eq!(frame.line, 2);
        assert!(frame
            .source
            .as_ref()
            .and_then(|src| src.path.as_ref())
            .is_some_and(|path| path.ends_with("prog.n1")));
    }
}
