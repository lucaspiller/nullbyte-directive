//! Performance harness for emulator-core benchmarking.
//!
//! Measures instruction throughput using production stepping patterns.
//!
//! ## Usage
//!
//! ```sh
//! cargo run -p emulator-core --example performance_harness
//! ```
//!
//! ## Metrics
//!
//! - Instructions per second
//! - Cycles per second
//! - Core-equivalents at 100 Hz (how many cores could run at 100 Hz based on measured throughput)
//!
//! ## Production Context
//!
//! The production target is 3,000+ active cores at 100 Hz. At 100 Hz, each tick is 10ms.
//! With a tick budget of 640 cycles, each core needs to complete 640 cycles in 10ms.
//! So 3,000 cores need 3,000 * 640 = 1,920,000 cycles per 10ms = 192,000,000 cycles/second.
//!
//! The benchmark runs on multiple threads to reflect real multi-core usage.

#![allow(clippy::pedantic)]

use emulator_core::{
    run_one, CoreConfig, CoreProfile, CoreState, MmioBus, MmioError, MmioWriteResult, RunBoundary,
};
use proptest as _;
use rstest as _;
#[cfg(feature = "serde")]
use serde as _;
use thiserror as _;

use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

const TICK_BUDGET_CYCLES: u16 = 640;
const TICK_DURATION_MS: u64 = 10;
const TICKS_PER_SECOND: u64 = 100;
const NUM_THREADS: usize = 4;

#[allow(clippy::cast_possible_truncation)]
fn encode(op: u8, rd: u8, ra: u8, sub: u8, am: u8) -> u16 {
    (u16::from(op) << 12)
        | (u16::from(rd) << 9)
        | (u16::from(ra) << 6)
        | (u16::from(sub) << 3)
        | u16::from(am)
}

fn load_word(state: &mut CoreState, addr: u16, word: u16) {
    let [hi, lo] = word.to_be_bytes();
    state.memory[usize::from(addr)] = hi;
    state.memory[usize::from(addr.wrapping_add(1))] = lo;
}

#[derive(Default)]
struct NoopMmio;

impl MmioBus for NoopMmio {
    fn read16(&mut self, _addr: u16) -> Result<u16, MmioError> {
        Ok(0)
    }

    fn write16(&mut self, _addr: u16, _value: u16) -> Result<MmioWriteResult, MmioError> {
        Ok(MmioWriteResult::Applied)
    }
}

#[derive(Debug, Clone, Copy)]
struct BenchmarkResult {
    name: &'static str,
    instructions_per_second: f64,
    cycles_per_second: f64,
    core_equivalents_100hz: f64,
}

fn benchmark_nop_loop(duration: Duration) -> BenchmarkResult {
    let (tx, rx) = mpsc::channel();

    let handles: Vec<_> = (0..NUM_THREADS)
        .map(|_| {
            let tx = tx.clone();
            thread::spawn(move || {
                let mut state = CoreState::default();

                load_word(&mut state, 0x0000, encode(0x0, 0, 0, 0x0, 0));
                load_word(&mut state, 0x0002, encode(0x2, 0, 0, 0x0, 0));

                let config = CoreConfig {
                    profile: CoreProfile::Authority,
                    tick_budget_cycles: TICK_BUDGET_CYCLES,
                    tracing_enabled: false,
                };
                let mut mmio = NoopMmio;

                let mut total_instructions = 0u64;
                let mut total_cycles = 0u64;
                let start = Instant::now();

                while start.elapsed() < duration {
                    state.arch.set_tick(0);
                    state.run_state = emulator_core::RunState::Running;
                    state.arch.set_pc(0x0000);

                    let outcome =
                        run_one(&mut state, &mut mmio, &config, RunBoundary::TickBoundary);
                    total_instructions += u64::from(outcome.steps);
                    total_cycles += u64::from(state.arch.tick());
                }

                tx.send((total_instructions, total_cycles)).ok();
            })
        })
        .collect();

    for h in handles {
        h.join().ok();
    }

    drop(tx);

    let mut total_instructions = 0u64;
    let mut total_cycles = 0u64;
    for (inst, cyc) in rx {
        total_instructions += inst;
        total_cycles += cyc;
    }

    let elapsed_secs = duration.as_secs_f64();
    let instructions_per_second = total_instructions as f64 / elapsed_secs;
    let cycles_per_second = total_cycles as f64 / elapsed_secs;
    let cores_at_100hz =
        instructions_per_second / (f64::from(TICK_BUDGET_CYCLES) * TICKS_PER_SECOND as f64);

    BenchmarkResult {
        name: "nop_loop",
        instructions_per_second,
        cycles_per_second,
        core_equivalents_100hz: cores_at_100hz,
    }
}

fn benchmark_alu_loop(duration: Duration) -> BenchmarkResult {
    let (tx, rx) = mpsc::channel();

    let handles: Vec<_> = (0..NUM_THREADS)
        .map(|_| {
            let tx = tx.clone();
            thread::spawn(move || {
                let mut state = CoreState::default();

                load_word(&mut state, 0x0000, encode(0x4, 0, 1, 0x0, 0));
                load_word(&mut state, 0x0002, encode(0x5, 0, 1, 0x0, 0));
                load_word(&mut state, 0x0004, encode(0x6, 0, 1, 0x0, 0));
                load_word(&mut state, 0x0006, encode(0x7, 0, 1, 0x0, 0));
                load_word(&mut state, 0x0008, encode(0x2, 0, 0, 0x0, 0));

                let config = CoreConfig {
                    profile: CoreProfile::Authority,
                    tick_budget_cycles: TICK_BUDGET_CYCLES,
                    tracing_enabled: false,
                };
                let mut mmio = NoopMmio;

                let mut total_instructions = 0u64;
                let mut total_cycles = 0u64;
                let start = Instant::now();

                while start.elapsed() < duration {
                    state.arch.set_tick(0);
                    state.run_state = emulator_core::RunState::Running;
                    state.arch.set_pc(0x0000);

                    let outcome =
                        run_one(&mut state, &mut mmio, &config, RunBoundary::TickBoundary);
                    total_instructions += u64::from(outcome.steps);
                    total_cycles += u64::from(state.arch.tick());
                }

                tx.send((total_instructions, total_cycles)).ok();
            })
        })
        .collect();

    for h in handles {
        h.join().ok();
    }

    drop(tx);

    let mut total_instructions = 0u64;
    let mut total_cycles = 0u64;
    for (inst, cyc) in rx {
        total_instructions += inst;
        total_cycles += cyc;
    }

    let elapsed_secs = duration.as_secs_f64();
    let instructions_per_second = total_instructions as f64 / elapsed_secs;
    let cycles_per_second = total_cycles as f64 / elapsed_secs;
    let cores_at_100hz =
        instructions_per_second / (f64::from(TICK_BUDGET_CYCLES) * TICKS_PER_SECOND as f64);

    BenchmarkResult {
        name: "alu_loop",
        instructions_per_second,
        cycles_per_second,
        core_equivalents_100hz: cores_at_100hz,
    }
}

fn benchmark_memory_loop(duration: Duration) -> BenchmarkResult {
    let (tx, rx) = mpsc::channel();

    let handles: Vec<_> = (0..NUM_THREADS)
        .map(|_| {
            let tx = tx.clone();
            thread::spawn(move || {
                let mut state = CoreState::default();

                let ram_base: u16 = 0x4000;

                load_word(&mut state, 0x0000, encode(0x8, 0, 0, 0x0, 1));
                state.arch.set_sp(ram_base);
                load_word(&mut state, 0x0002, encode(0x9, 0, 0, 0x0, 1));
                load_word(&mut state, 0x0004, encode(0x2, 0, 0, 0x0, 0));

                let config = CoreConfig {
                    profile: CoreProfile::Authority,
                    tick_budget_cycles: TICK_BUDGET_CYCLES,
                    tracing_enabled: false,
                };
                let mut mmio = NoopMmio;

                let mut total_instructions = 0u64;
                let mut total_cycles = 0u64;
                let start = Instant::now();

                while start.elapsed() < duration {
                    state.arch.set_tick(0);
                    state.run_state = emulator_core::RunState::Running;
                    state.arch.set_pc(0x0000);
                    state.arch.set_sp(ram_base);

                    let outcome =
                        run_one(&mut state, &mut mmio, &config, RunBoundary::TickBoundary);
                    total_instructions += u64::from(outcome.steps);
                    total_cycles += u64::from(state.arch.tick());
                }

                tx.send((total_instructions, total_cycles)).ok();
            })
        })
        .collect();

    for h in handles {
        h.join().ok();
    }

    drop(tx);

    let mut total_instructions = 0u64;
    let mut total_cycles = 0u64;
    for (inst, cyc) in rx {
        total_instructions += inst;
        total_cycles += cyc;
    }

    let elapsed_secs = duration.as_secs_f64();
    let instructions_per_second = total_instructions as f64 / elapsed_secs;
    let cycles_per_second = total_cycles as f64 / elapsed_secs;
    let cores_at_100hz =
        instructions_per_second / (f64::from(TICK_BUDGET_CYCLES) * TICKS_PER_SECOND as f64);

    BenchmarkResult {
        name: "memory_loop",
        instructions_per_second,
        cycles_per_second,
        core_equivalents_100hz: cores_at_100hz,
    }
}

fn benchmark_mixed_loop(duration: Duration) -> BenchmarkResult {
    let (tx, rx) = mpsc::channel();

    let handles: Vec<_> = (0..NUM_THREADS)
        .map(|_| {
            let tx = tx.clone();
            thread::spawn(move || {
                let mut state = CoreState::default();

                load_word(&mut state, 0x0000, encode(0x0, 0, 0, 0x0, 0));
                load_word(&mut state, 0x0002, encode(0x4, 0, 1, 0x0, 0));
                load_word(&mut state, 0x0004, encode(0x5, 1, 0, 0x0, 0));
                load_word(&mut state, 0x0006, encode(0x6, 2, 1, 0x0, 0));
                load_word(&mut state, 0x0008, encode(0x3, 0, 0, 0x0, 0));
                load_word(&mut state, 0x000A, encode(0x2, 0, 0, 0x0, 0));

                let config = CoreConfig {
                    profile: CoreProfile::Authority,
                    tick_budget_cycles: TICK_BUDGET_CYCLES,
                    tracing_enabled: false,
                };
                let mut mmio = NoopMmio;

                let mut total_instructions = 0u64;
                let mut total_cycles = 0u64;
                let start = Instant::now();

                while start.elapsed() < duration {
                    state.arch.set_tick(0);
                    state.run_state = emulator_core::RunState::Running;
                    state.arch.set_pc(0x0000);

                    let outcome =
                        run_one(&mut state, &mut mmio, &config, RunBoundary::TickBoundary);
                    total_instructions += u64::from(outcome.steps);
                    total_cycles += u64::from(state.arch.tick());
                }

                tx.send((total_instructions, total_cycles)).ok();
            })
        })
        .collect();

    for h in handles {
        h.join().ok();
    }

    drop(tx);

    let mut total_instructions = 0u64;
    let mut total_cycles = 0u64;
    for (inst, cyc) in rx {
        total_instructions += inst;
        total_cycles += cyc;
    }

    let elapsed_secs = duration.as_secs_f64();
    let instructions_per_second = total_instructions as f64 / elapsed_secs;
    let cycles_per_second = total_cycles as f64 / elapsed_secs;
    let cores_at_100hz =
        instructions_per_second / (f64::from(TICK_BUDGET_CYCLES) * TICKS_PER_SECOND as f64);

    BenchmarkResult {
        name: "mixed_loop",
        instructions_per_second,
        cycles_per_second,
        core_equivalents_100hz: cores_at_100hz,
    }
}

fn format_number(n: f64) -> String {
    if n >= 1_000_000.0 {
        format!("{:.2}M", n / 1_000_000.0)
    } else if n >= 1_000.0 {
        format!("{:.2}K", n / 1_000.0)
    } else {
        format!("{:.2}", n)
    }
}

fn print_results(results: &[BenchmarkResult]) {
    println!("\n╔═════════════════════════════════════════════════════════════════╗");
    println!("║              EMULATOR-CORE PERFORMANCE HARNESS                  ║");
    println!("╠═════════════════════════════════════════════════════════════════╣");
    println!("║ Configuration:                                                  ║");
    println!(
        "║   Threads:       {:>5}                                          ║",
        NUM_THREADS
    );
    println!(
        "║   Tick budget:   {:>5} cycles/tick                              ║",
        TICK_BUDGET_CYCLES
    );
    println!(
        "║   Tick duration: {:>5} ms                                       ║",
        TICK_DURATION_MS
    );
    println!(
        "║   Target rate:   {:>5} Hz                                       ║",
        TICKS_PER_SECOND
    );
    println!("║   Target cores:  3,000+ active cores                            ║");
    println!("╠═════════════════════════════════════════════════════════════════╣");
    println!(
        "║ {:12} │ {:>15} │ {:>15} │ {:>12} ║",
        "Benchmark", "Instr/sec", "Cycles/sec", "Cores@100Hz"
    );
    println!("╟──────────────┼─────────────────┼─────────────────┼──────────────╢");

    for result in results {
        println!(
            "║ {:12} │ {:>15} │ {:>15} │ {:>12} ║",
            result.name,
            format_number(result.instructions_per_second),
            format_number(result.cycles_per_second),
            format_number(result.core_equivalents_100hz)
        );
    }

    println!("╚═════════════════════════════════════════════════════════════════╝");

    println!("\nProduction Requirements:");
    for result in results {
        let status = if result.core_equivalents_100hz >= 3000.0 {
            "✓ PASS"
        } else if result.core_equivalents_100hz >= 1500.0 {
            "~ MARGINAL"
        } else {
            "✗ FAIL"
        };
        println!(
            "  {} {}: {} cores (target: 3,000)",
            status,
            result.name,
            format_number(result.core_equivalents_100hz)
        );
    }
}

fn main() {
    let warmup = Duration::from_millis(500);
    let benchmark_duration = Duration::from_secs(3);

    println!("Running warmup for {:?}...", warmup);
    let _ = benchmark_nop_loop(warmup);

    println!("Running benchmarks for {:?} each...\n", benchmark_duration);

    let nop_result = benchmark_nop_loop(benchmark_duration);
    let alu_result = benchmark_alu_loop(benchmark_duration);
    let memory_result = benchmark_memory_loop(benchmark_duration);
    let mixed_result = benchmark_mixed_loop(benchmark_duration);

    print_results(&[nop_result, alu_result, memory_result, mixed_result]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_benchmark_nop_loop_runs() {
        let result = benchmark_nop_loop(Duration::from_millis(100));
        assert!(result.instructions_per_second > 0.0);
        assert!(result.cycles_per_second > 0.0);
        assert!(result.core_equivalents_100hz > 0.0);
    }

    #[test]
    fn test_benchmark_alu_loop_runs() {
        let result = benchmark_alu_loop(Duration::from_millis(100));
        assert!(result.instructions_per_second > 0.0);
    }

    #[test]
    fn test_benchmark_memory_loop_runs() {
        let result = benchmark_memory_loop(Duration::from_millis(100));
        assert!(result.instructions_per_second > 0.0);
    }

    #[test]
    fn test_benchmark_mixed_loop_runs() {
        let result = benchmark_mixed_loop(Duration::from_millis(100));
        assert!(result.instructions_per_second > 0.0);
    }
}
