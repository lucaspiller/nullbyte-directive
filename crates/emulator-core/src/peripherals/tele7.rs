//! TELE-7 Textual Display Device peripheral implementation.
//!
//! Provides MMIO interface for the TELE-7 40x25 character display.

use crate::api::{MmioBus, MmioError, MmioWriteResult};

/// TELE-7 MMIO register base address.
pub const TELE7_BASE: u16 = 0xE120;

/// TELE-7 MMIO register end address.
pub const TELE7_END: u16 = 0xE12F;

/// TELE-7 device identification constant.
pub const TELE7_ID: u16 = 0x0745;

/// TELE-7 device version.
pub const TELE7_VERSION: u16 = 0x0001;

const PAGE_SIZE_WORDS: usize = 500;
#[allow(clippy::cast_possible_truncation)]
const PAGE_SIZE_BYTES: u16 = PAGE_SIZE_WORDS as u16 * 2;

#[allow(dead_code)]
const COLS: usize = 40;

#[allow(dead_code)]
const ROWS: usize = 25;

const DEFAULT_BLINK_DIV: u16 = 50;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Configuration for the TELE-7 peripheral.
pub struct Tele7Config {
    /// Default page buffer base address.
    pub page_base: u16,
}

impl Default for Tele7Config {
    fn default() -> Self {
        Self { page_base: 0x4000 }
    }
}

#[derive(Debug, Clone)]
/// Runtime state for the TELE-7 peripheral.
pub struct Tele7State {
    ctrl: u16,
    #[allow(dead_code)]
    status: u16,
    page_base: u16,
    border: u16,
    origin: u16,
    blink_div: u16,
    fault: bool,
    tick_count: u32,
}

impl Default for Tele7State {
    fn default() -> Self {
        Self {
            ctrl: 0,
            status: 0,
            page_base: 0x4000,
            border: 0,
            origin: 0,
            blink_div: DEFAULT_BLINK_DIV,
            fault: false,
            tick_count: 0,
        }
    }
}

impl Tele7State {
    /// Advances the tick counter for blink timing.
    #[allow(clippy::missing_const_for_fn)]
    pub fn tick(&mut self) {
        self.tick_count = self.tick_count.wrapping_add(1);
    }

    /// Returns true if the display is enabled.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    pub fn is_enabled(&self) -> bool {
        self.ctrl & 0x01 != 0
    }

    /// Returns true if live-read mode is enabled.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    pub fn is_live_read(&self) -> bool {
        self.ctrl & 0x02 != 0
    }

    /// Returns the current blink phase.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    pub fn blink_phase(&self) -> bool {
        if self.blink_div == 0 {
            return false;
        }
        let div = u32::from(self.blink_div);
        let count = self.tick_count / div;
        !count.is_multiple_of(2)
    }

    /// Returns true if the page buffer is currently mapped and valid.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    pub fn page_mapped(&self) -> bool {
        if self.fault {
            return false;
        }
        let base = self.page_base;
        let end = base.wrapping_add(PAGE_SIZE_BYTES);
        base < 0xE000 && end <= 0xDFFF && base.is_multiple_of(2)
    }

    /// Returns the current STATUS register bits.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    pub fn status_bits(&self) -> u16 {
        let mut status = 0;
        if self.is_enabled() {
            status |= 0x01;
        }
        if self.page_mapped() {
            status |= 0x02;
        }
        if self.fault {
            status |= 0x04;
        }
        if self.blink_phase() {
            status |= 0x08;
        }
        status
    }

    /// Returns the current origin (scroll position).
    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    pub fn origin(&self) -> u16 {
        self.origin % 25
    }

    /// Returns the border color (0-7).
    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    pub fn border_color(&self) -> u8 {
        (self.border & 0x07) as u8
    }
}

/// TELE-7 Textual Display Device peripheral.
///
/// Implements the `MmioBus` trait for integration with the emulator core.
#[derive(Debug)]
pub struct Tele7Peripheral {
    #[allow(dead_code)]
    config: Tele7Config,
    state: Tele7State,
}

impl Default for Tele7Peripheral {
    fn default() -> Self {
        Self::new(Tele7Config::default())
    }
}

impl Tele7Peripheral {
    /// Creates a new TELE-7 peripheral with the given configuration.
    #[must_use]
    pub fn new(config: Tele7Config) -> Self {
        Self {
            config,
            state: Tele7State::default(),
        }
    }

    /// Returns a reference to the current state.
    #[must_use]
    pub const fn state(&self) -> &Tele7State {
        &self.state
    }

    /// Returns a mutable reference to the current state.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    pub fn state_mut(&mut self) -> &mut Tele7State {
        &mut self.state
    }

    /// Resets the peripheral to default state.
    pub fn reset(&mut self) {
        self.state = Tele7State::default();
    }

    #[allow(clippy::missing_const_for_fn)]
    fn validate_page_base(&mut self, addr: u16) {
        let end = addr.wrapping_add(PAGE_SIZE_BYTES);
        self.state.fault =
            !addr.is_multiple_of(2) || addr < 0x4000 || end > 0xDFFF || addr >= 0xE000;
        self.state.page_base = addr;
    }

    /// Reads a byte from the page buffer at the given index.
    #[must_use]
    pub fn read_page_byte(&self, memory: &[u8], byte_idx: usize) -> u8 {
        if !self.state.page_mapped() {
            return 0;
        }
        let addr = self
            .state
            .page_base
            .wrapping_add(u16::try_from(byte_idx).unwrap_or(0));
        memory.get(addr as usize).map_or(0, |val| *val)
    }

    /// Gets the complete display buffer from page memory.
    ///
    /// Returns a vector of word pairs (high byte, low byte) representing
    /// the 40x25 character grid.
    #[must_use]
    pub fn get_display_buffer(&self, memory: &[u8]) -> Vec<[u8; 2]> {
        let mut buffer = Vec::with_capacity(PAGE_SIZE_WORDS);
        for word_idx in 0..PAGE_SIZE_WORDS {
            let byte_idx = word_idx * 2;
            buffer.push([
                self.read_page_byte(memory, byte_idx),
                self.read_page_byte(memory, byte_idx + 1),
            ]);
        }
        buffer
    }
}

impl MmioBus for Tele7Peripheral {
    fn read16(&mut self, addr: u16) -> Result<u16, MmioError> {
        match addr {
            0xE120 => Ok(TELE7_ID),
            0xE121 => Ok(TELE7_VERSION),
            0xE122 => Ok(self.state.ctrl & 0x07),
            0xE123 => Ok(self.state.status_bits()),
            0xE124 => Ok(self.state.page_base),
            0xE125 => Ok(self.state.border),
            0xE126 => Ok(self.state.origin),
            0xE127 => Ok(self.state.blink_div),
            _ => Ok(0),
        }
    }

    fn write16(&mut self, addr: u16, value: u16) -> Result<MmioWriteResult, MmioError> {
        match addr {
            0xE122 => {
                self.state.ctrl = value & 0x07;
            }
            0xE124 => {
                self.validate_page_base(value);
            }
            0xE125 => {
                self.state.border = value & 0x07;
            }
            0xE126 => {
                self.state.origin = value % 25;
            }
            0xE127 => {
                self.state.blink_div = if value == 0 { DEFAULT_BLINK_DIV } else { value };
            }
            _ => {}
        }
        Ok(MmioWriteResult::Applied)
    }
}

/// Composite MMIO bus supporting multiple peripheral devices.
pub struct CompositeMmio {
    tele7: Option<Tele7Peripheral>,
}

impl Default for CompositeMmio {
    fn default() -> Self {
        Self::new()
    }
}

impl CompositeMmio {
    /// Creates a new empty composite MMIO bus.
    #[must_use]
    pub const fn new() -> Self {
        Self { tele7: None }
    }

    /// Adds a TELE-7 peripheral to the bus.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    pub fn with_tele7(mut self, tele7: Tele7Peripheral) -> Self {
        self.tele7 = Some(tele7);
        self
    }

    /// Returns a reference to the TELE-7 peripheral, if present.
    #[must_use]
    pub const fn tele7(&self) -> Option<&Tele7Peripheral> {
        self.tele7.as_ref()
    }

    /// Returns a mutable reference to the TELE-7 peripheral, if present.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    pub fn tele7_mut(&mut self) -> Option<&mut Tele7Peripheral> {
        self.tele7.as_mut()
    }

    /// Advances tick counter for all peripherals.
    pub fn tick(&mut self) {
        if let Some(t7) = self.tele7.as_mut() {
            t7.state_mut().tick();
        }
    }
}

impl MmioBus for CompositeMmio {
    fn read16(&mut self, addr: u16) -> Result<u16, MmioError> {
        if let Some(ref mut t7) = self.tele7 {
            if (0xE120..=0xE12F).contains(&addr) {
                return t7.read16(addr);
            }
        }
        Ok(0)
    }

    fn write16(&mut self, addr: u16, value: u16) -> Result<MmioWriteResult, MmioError> {
        if let Some(ref mut t7) = self.tele7 {
            if (0xE120..=0xE12F).contains(&addr) {
                return t7.write16(addr, value);
            }
        }
        Ok(MmioWriteResult::Applied)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tele7_constants() {
        assert_eq!(TELE7_BASE, 0xE120);
        assert_eq!(TELE7_END, 0xE12F);
        assert_eq!(TELE7_ID, 0x0745);
        assert_eq!(TELE7_VERSION, 0x0001);
    }

    #[test]
    fn tele7_default_state() {
        let t7 = Tele7Peripheral::default();
        let state = t7.state();
        assert!(!state.is_enabled());
        // Default page_base (0x4000) maps to valid RAM, so page_mapped is true
        assert!(state.page_mapped());
        assert!(!state.blink_phase());
        assert_eq!(state.origin(), 0);
        assert_eq!(state.border_color(), 0);
    }

    #[test]
    fn tele7_ctrl_register() {
        let mut t7 = Tele7Peripheral::default();

        // Write to CTRL register (0xE122)
        t7.write16(0xE122, 0x01).unwrap();
        assert!(t7.state().is_enabled());

        t7.write16(0xE122, 0x00).unwrap();
        assert!(!t7.state().is_enabled());
    }

    #[test]
    fn tele7_page_base_validation() {
        let mut t7 = Tele7Peripheral::default();

        // Valid page base
        t7.write16(0xE124, 0x4000).unwrap();
        assert!(t7.state().page_mapped());

        // Invalid - odd address
        t7.write16(0xE124, 0x4001).unwrap();
        assert!(!t7.state().page_mapped());

        // Invalid - in MMIO region
        t7.write16(0xE124, 0xE000).unwrap();
        assert!(!t7.state().page_mapped());
    }

    #[test]
    fn tele7_read_id_version() {
        let mut t7 = Tele7Peripheral::default();

        assert_eq!(t7.read16(0xE120).unwrap(), TELE7_ID);
        assert_eq!(t7.read16(0xE121).unwrap(), TELE7_VERSION);
    }

    #[test]
    fn tele7_status_bits() {
        let mut t7 = Tele7Peripheral::default();

        // Initially disabled, but page is mapped (default 0x4000)
        let status = t7.read16(0xE123).unwrap();
        assert_eq!(status & 0x01, 0); // Not enabled
        assert!(status & 0x02 != 0); // PAGE_MAPPED

        // Enable and set valid page
        t7.write16(0xE122, 0x01).unwrap();
        t7.write16(0xE124, 0x4000).unwrap();

        let status = t7.read16(0xE123).unwrap();
        assert!(status & 0x01 != 0); // ENABLED
        assert!(status & 0x02 != 0); // PAGE_MAPPED
    }

    #[test]
    fn tele7_blink_timing() {
        let mut t7 = Tele7Peripheral::default();

        // Default blink div is 50
        assert_eq!(t7.state().blink_div, DEFAULT_BLINK_DIV);

        // Tick advances the counter
        t7.state_mut().tick();
        t7.state_mut().tick();

        // Blink phase depends on tick count
        let _ = t7.state().blink_phase();
    }

    #[test]
    fn tele7_border_color() {
        let mut t7 = Tele7Peripheral::default();

        t7.write16(0xE125, 0x03).unwrap();
        assert_eq!(t7.state().border_color(), 3);

        // Only low 3 bits
        t7.write16(0xE125, 0xFF).unwrap();
        assert_eq!(t7.state().border_color(), 7);
    }

    #[test]
    fn tele7_origin_scroll() {
        let mut t7 = Tele7Peripheral::default();

        t7.write16(0xE126, 10).unwrap();
        assert_eq!(t7.state().origin(), 10);

        // Wraps at 25
        t7.write16(0xE126, 30).unwrap();
        assert_eq!(t7.state().origin(), 5);
    }

    #[test]
    fn tele7_display_buffer() {
        let t7 = Tele7Peripheral::default();

        // Create test memory with some data at page base
        let mut memory = vec![0u8; 65536];
        memory[0x4000] = b'H';
        memory[0x4001] = b'e';

        let buffer = t7.get_display_buffer(&memory);
        assert_eq!(buffer[0][0], b'H');
        assert_eq!(buffer[0][1], b'e');
    }

    #[test]
    fn composite_mmio_with_tele7() {
        let mmio = CompositeMmio::new().with_tele7(Tele7Peripheral::new(Tele7Config::default()));

        assert!(mmio.tele7().is_some());
    }

    #[test]
    fn composite_mmio_delegates_to_tele7() {
        let mut mmio =
            CompositeMmio::new().with_tele7(Tele7Peripheral::new(Tele7Config::default()));

        // Read TELE7 ID
        let id = mmio.read16(0xE120).unwrap();
        assert_eq!(id, TELE7_ID);

        // Write to CTRL
        mmio.write16(0xE122, 0x01).unwrap();

        // Verify through tele7 accessor
        assert!(mmio.tele7().unwrap().state().is_enabled());
    }

    #[test]
    fn composite_mmio_tick() {
        let mut mmio =
            CompositeMmio::new().with_tele7(Tele7Peripheral::new(Tele7Config::default()));

        mmio.tick();
        // Should not panic
    }

    #[test]
    fn tele7_execution_flow() {
        use crate::{step_one, CoreConfig, CoreState, GeneralRegister, StepOutcome};

        let mut state = CoreState::default();
        let config = CoreConfig::default();
        let mut mmio =
            CompositeMmio::new().with_tele7(Tele7Peripheral::new(Tele7Config::default()));

        // MOV R1, #0x4100
        state.memory[0] = 0x12;
        state.memory[1] = 0x05;
        state.memory[2] = 0x41;
        state.memory[3] = 0x00;
        // STORE R1, #0xE124 (Immediate mode: primary=0x3205, ext=0xE124)
        state.memory[4] = 0x32;
        state.memory[5] = 0x05;
        state.memory[6] = 0xE1;
        state.memory[7] = 0x24;
        // MOV R1, #0x0001
        state.memory[8] = 0x12;
        state.memory[9] = 0x05;
        state.memory[10] = 0x00;
        state.memory[11] = 0x01;
        // STORE R1, #0xE122 (Immediate mode: primary=0x3205, ext=0xE122)
        state.memory[12] = 0x32;
        state.memory[13] = 0x05;
        state.memory[14] = 0xE1;
        state.memory[15] = 0x22;
        // HALT
        state.memory[16] = 0x00;
        state.memory[17] = 0x10;

        // Execute MOV R1, #0x4100
        let outcome = step_one(&mut state, &mut mmio, &config);
        assert!(matches!(outcome, StepOutcome::Retired { .. }));
        assert_eq!(state.arch.gpr(GeneralRegister::R1), 0x4100);

        // Execute STORE R1, #0xE124
        let outcome = step_one(&mut state, &mut mmio, &config);
        assert!(matches!(outcome, StepOutcome::Retired { .. }));

        // Verify PAGE_BASE was written
        let t7 = mmio.tele7().unwrap();
        assert_eq!(t7.state().page_base, 0x4100);

        // Execute MOV R1, #0x0001
        let outcome = step_one(&mut state, &mut mmio, &config);
        assert!(matches!(outcome, StepOutcome::Retired { .. }));
        assert_eq!(state.arch.gpr(GeneralRegister::R1), 0x0001);

        // Execute STORE R1, #0xE122
        let outcome = step_one(&mut state, &mut mmio, &config);
        assert!(matches!(outcome, StepOutcome::Retired { .. }));

        // Verify CTRL was written - TELE-7 should now be enabled
        let t7 = mmio.tele7().unwrap();
        assert!(
            t7.state().is_enabled(),
            "TELE-7 should be enabled after writing CTRL=1"
        );

        // Execute HALT
        let outcome = step_one(&mut state, &mut mmio, &config);
        assert!(matches!(outcome, StepOutcome::HaltedForTick));
    }
}
