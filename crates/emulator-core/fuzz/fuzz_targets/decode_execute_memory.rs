#![no_main]

use emulator_core::{
    validate_fetch_access, validate_mmio_alignment, validate_mmio_width, validate_word_alignment,
    CoreConfig, CoreState, Decoder, MmioBus, MmioError, MmioWriteResult,
};
use libfuzzer_sys::fuzz_target;

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

fuzz_target!(|data: &[u8]| {
    if data.len() < 5 {
        return;
    }

    let word = u16::from_be_bytes([data[0], data[1]]);
    let addr = u16::from_be_bytes([data[2], data[3]]);
    let width = data[4];

    let _ = Decoder::decode(word);

    let mut state = CoreState::default();
    state.memory[0] = data[0];
    state.memory[1] = data[1];
    let mut mmio = NoopMmio;
    let config = CoreConfig::default();
    let _ = emulator_core::step_one(&mut state, &mut mmio, &config);

    let _ = validate_fetch_access(addr);
    let _ = validate_word_alignment(addr);
    let _ = validate_mmio_alignment(addr);
    let _ = validate_mmio_width(width);
});
