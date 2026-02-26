#![forbid(unsafe_code)]
//! Stdio transport for `emulator-debug-session`.
//!
//! This process accepts one JSON request per input line and writes one JSON
//! response per output line. It is intended for local tooling integration
//! (for example, the VS Code DAP adapter) while keeping
//! `emulator-debug-session` as the single runtime authority.

use std::collections::BTreeMap;
use std::io::{self, BufRead, Write};
use std::path::Path;

use assembler as _;
use emulator_core::{CoreConfig, GeneralRegister, RunBoundary, RunState, StepOutcome};
use emulator_debug_session::{
    BreakpointStatus, BreakpointUnresolvedReason, DebugSessionHost, HostError,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[cfg(test)]
use tempfile as _;

#[derive(Debug, Deserialize)]
struct RequestEnvelope {
    id: u64,
    method: String,
    #[serde(default)]
    params: Value,
}

#[derive(Debug, Serialize)]
struct ResponseEnvelope {
    id: u64,
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LaunchParams {
    program: String,
}

#[derive(Debug, Deserialize)]
struct SetBreakpointsParams {
    file: String,
    lines: Vec<usize>,
}

#[derive(Debug, Deserialize)]
struct ReadMemoryWindowParams {
    start: u16,
    len: usize,
}

#[derive(Debug, Serialize)]
#[allow(clippy::struct_field_names)]
struct SessionIdsWire {
    debug_session_id: String,
    build_id: String,
    layout_id: String,
}

#[derive(Debug, Serialize)]
struct LaunchResponse {
    ids: SessionIdsWire,
    warnings: Vec<String>,
}

#[derive(Debug, Serialize)]
struct BreakpointResponse {
    line: usize,
    verified: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
}

#[derive(Debug, Serialize)]
struct SetBreakpointsResponse {
    breakpoints: Vec<BreakpointResponse>,
}

#[derive(Debug, Serialize)]
struct StopOutcomeResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    stop_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    fault: Option<String>,
}

#[derive(Debug, Serialize)]
struct ReadStateResponse {
    paused: bool,
    run_state: String,
    registers: BTreeMap<String, u16>,
}

#[derive(Debug, Serialize)]
struct ReadMemoryWindowResponse {
    start: u16,
    bytes: Vec<u8>,
}

fn main() -> io::Result<()> {
    let mut host = DebugSessionHost::new(CoreConfig::default());
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(line) => line,
            Err(err) => {
                write_response(
                    &mut stdout,
                    &ResponseEnvelope {
                        id: 0,
                        ok: false,
                        result: None,
                        error: Some(format!("stdin read error: {err}")),
                    },
                )?;
                continue;
            }
        };

        if line.trim().is_empty() {
            continue;
        }

        let request: RequestEnvelope = match serde_json::from_str(&line) {
            Ok(request) => request,
            Err(err) => {
                write_response(
                    &mut stdout,
                    &ResponseEnvelope {
                        id: 0,
                        ok: false,
                        result: None,
                        error: Some(format!("invalid request JSON: {err}")),
                    },
                )?;
                continue;
            }
        };

        let response = match handle_request(&mut host, request.id, &request.method, request.params)
        {
            Ok(result) => ResponseEnvelope {
                id: request.id,
                ok: true,
                result: Some(result),
                error: None,
            },
            Err(err) => ResponseEnvelope {
                id: request.id,
                ok: false,
                result: None,
                error: Some(err),
            },
        };

        write_response(&mut stdout, &response)?;
    }

    Ok(())
}

fn handle_request(
    host: &mut DebugSessionHost,
    _id: u64,
    method: &str,
    params: Value,
) -> Result<Value, String> {
    match method {
        "launch" => {
            let params: LaunchParams = parse_params(params)?;
            let report = host
                .launch(Path::new(&params.program))
                .map_err(host_error_to_string)?;
            to_json_value(LaunchResponse {
                ids: SessionIdsWire {
                    debug_session_id: report.ids.debug_session_id,
                    build_id: report.ids.build_id,
                    layout_id: report.ids.layout_id,
                },
                warnings: report.warnings,
            })
        }
        "set_breakpoints" => {
            let params: SetBreakpointsParams = parse_params(params)?;
            let breakpoints = host
                .set_breakpoints(&params.file, &params.lines)
                .map_err(host_error_to_string)?;
            let mapped = breakpoints
                .into_iter()
                .map(|bp| match bp.status {
                    BreakpointStatus::Resolved { .. } => BreakpointResponse {
                        line: bp.source.line,
                        verified: true,
                        message: None,
                    },
                    BreakpointStatus::Unresolved { reason } => BreakpointResponse {
                        line: bp.source.line,
                        verified: false,
                        message: Some(match reason {
                            BreakpointUnresolvedReason::NoSourceMatch => {
                                "no source mapping for this line".to_string()
                            }
                            BreakpointUnresolvedReason::LayoutChanged => {
                                "layout changed; breakpoint remap failed".to_string()
                            }
                        }),
                    },
                })
                .collect();
            to_json_value(SetBreakpointsResponse {
                breakpoints: mapped,
            })
        }
        "continue_execution" => {
            let outcome = host
                .continue_execution(RunBoundary::Halted)
                .map_err(host_error_to_string)?;
            to_json_value(step_outcome_to_response(outcome.final_step))
        }
        "instruction_step" => {
            let outcome = host.instruction_step().map_err(host_error_to_string)?;
            to_json_value(step_outcome_to_response(outcome))
        }
        "pause" => {
            host.pause().map_err(host_error_to_string)?;
            to_json_value(serde_json::json!({}))
        }
        "read_state" => {
            let snapshot = host.read_state().map_err(host_error_to_string)?;
            to_json_value(ReadStateResponse {
                paused: snapshot.paused,
                run_state: run_state_name(snapshot.state.run_state),
                registers: registers_map(&snapshot.state.arch),
            })
        }
        "read_memory_window" => {
            let params: ReadMemoryWindowParams = parse_params(params)?;
            let window = host
                .read_memory_window(params.start, params.len)
                .map_err(host_error_to_string)?;
            to_json_value(ReadMemoryWindowResponse {
                start: window.start,
                bytes: window.bytes,
            })
        }
        "disconnect" => {
            host.terminate();
            to_json_value(serde_json::json!({}))
        }
        other => Err(format!("unknown method: {other}")),
    }
}

fn parse_params<T: for<'de> Deserialize<'de>>(params: Value) -> Result<T, String> {
    serde_json::from_value(params).map_err(|err| format!("invalid params: {err}"))
}

fn to_json_value<T: Serialize>(value: T) -> Result<Value, String> {
    serde_json::to_value(value).map_err(|err| format!("response serialization failed: {err}"))
}

fn write_response(stdout: &mut io::Stdout, response: &ResponseEnvelope) -> io::Result<()> {
    serde_json::to_writer(&mut *stdout, response)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
    stdout.write_all(b"\n")?;
    stdout.flush()
}

fn host_error_to_string(err: HostError) -> String {
    match err {
        HostError::Assemble(inner) => format!("assembly error: {inner}"),
        _ => err.to_string(),
    }
}

fn step_outcome_to_response(step: StepOutcome) -> StopOutcomeResponse {
    match step {
        StepOutcome::Fault { cause } => StopOutcomeResponse {
            stop_reason: Some("exception".to_string()),
            fault: Some(format!("{cause:?}")),
        },
        StepOutcome::HaltedForTick => StopOutcomeResponse {
            stop_reason: Some("pause".to_string()),
            fault: None,
        },
        StepOutcome::Retired { .. }
        | StepOutcome::TrapDispatch { .. }
        | StepOutcome::EventDispatch { .. } => StopOutcomeResponse {
            stop_reason: Some("step".to_string()),
            fault: None,
        },
    }
}

fn run_state_name(run_state: RunState) -> String {
    match run_state {
        RunState::Running => "Running".to_string(),
        RunState::HaltedForTick => "HaltedForTick".to_string(),
        RunState::HandlerContext => "HandlerContext".to_string(),
        RunState::FaultLatched(code) => format!("FaultLatched({code:?})"),
    }
}

fn registers_map(arch: &emulator_core::ArchitecturalState) -> BTreeMap<String, u16> {
    let mut registers = BTreeMap::new();
    for reg in GeneralRegister::ALL {
        registers.insert(format!("R{}", reg.index()), arch.gpr(reg));
    }
    registers.insert("PC".to_string(), arch.pc());
    registers.insert("SP".to_string(), arch.sp());
    registers.insert("FLAGS".to_string(), arch.flags());
    registers.insert("TICK".to_string(), arch.tick());
    registers.insert("CAP".to_string(), arch.cap());
    registers.insert("CAUSE".to_string(), arch.cause());
    registers.insert("EVP".to_string(), arch.evp());
    registers
}
