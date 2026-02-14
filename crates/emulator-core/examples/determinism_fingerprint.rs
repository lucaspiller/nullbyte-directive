//! Deterministic replay fingerprint generator used by CI cross-host comparison.

use emulator_core::{
    replay_from_snapshot, CoreConfig, CoreSnapshot, CoreState, MmioBus, MmioError, MmioWriteResult,
    ReplayEventStream, RunBoundary, SnapshotVersion,
};
use proptest as _;
use rstest as _;
#[cfg(feature = "serde")]
use serde as _;
use thiserror as _;

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

const fn encode(op: u8, rd: u8, ra: u8, sub: u8, am: u8) -> u16 {
    ((op as u16) << 12) | ((rd as u16) << 9) | ((ra as u16) << 6) | ((sub as u16) << 3) | am as u16
}

fn load_word(state: &mut CoreState, addr: u16, word: u16) {
    let [hi, lo] = word.to_be_bytes();
    state.memory[usize::from(addr)] = hi;
    state.memory[usize::from(addr.wrapping_add(1))] = lo;
}

fn hash_bytes(hash: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        *hash ^= u64::from(*byte);
        *hash = hash.wrapping_mul(0x1000_0000_01B3);
    }
}

fn fingerprint() -> String {
    let mut initial = CoreState::default();
    load_word(&mut initial, 0x0000, encode(0x0, 0, 0, 0x0, 0));
    load_word(&mut initial, 0x0002, encode(0x0, 0, 0, 0x2, 0));

    let snapshot = CoreSnapshot::from_core_state(SnapshotVersion::V1, &initial);
    let mut stream = ReplayEventStream::new();
    stream.add_event(0x11);
    stream.add_event(0x22);

    let config = CoreConfig::default();
    let mut mmio = NoopMmio;
    let replay = replay_from_snapshot(snapshot, &stream, &mut mmio, &config, RunBoundary::Halted)
        .expect("replay should succeed");

    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    hash_bytes(&mut hash, &replay.steps.to_le_bytes());

    match replay.final_outcome {
        emulator_core::StepOutcome::Retired { cycles } => {
            hash_bytes(&mut hash, &[0x10]);
            hash_bytes(&mut hash, &cycles.to_le_bytes());
        }
        emulator_core::StepOutcome::HaltedForTick => hash_bytes(&mut hash, &[0x11]),
        emulator_core::StepOutcome::TrapDispatch { cause } => {
            hash_bytes(&mut hash, &[0x12]);
            hash_bytes(&mut hash, &cause.to_le_bytes());
        }
        emulator_core::StepOutcome::EventDispatch { event_id } => {
            hash_bytes(&mut hash, &[0x13, event_id]);
        }
        emulator_core::StepOutcome::Fault { cause } => {
            hash_bytes(&mut hash, &[0x14, cause.as_u8()]);
        }
    }

    hash_bytes(&mut hash, &replay.final_state.arch.pc().to_le_bytes());
    hash_bytes(&mut hash, &replay.final_state.arch.sp().to_le_bytes());
    hash_bytes(&mut hash, &replay.final_state.arch.tick().to_le_bytes());
    hash_bytes(&mut hash, &replay.final_state.arch.flags().to_le_bytes());
    hash_bytes(&mut hash, &replay.final_state.arch.cause().to_le_bytes());
    hash_bytes(&mut hash, &replay.final_state.arch.cap().to_le_bytes());
    hash_bytes(&mut hash, &replay.final_state.arch.evp().to_le_bytes());
    hash_bytes(&mut hash, &replay.final_state.memory);

    format!("{hash:016x}")
}

fn main() {
    println!("{}", fingerprint());
}
