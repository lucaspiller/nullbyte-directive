/// Number of architecturally visible general-purpose registers (`R0..R7`).
pub const GENERAL_REGISTER_COUNT: usize = 8;
/// `FLAGS` bit for zero result.
pub const FLAGS_Z: u16 = 1 << 0;
/// `FLAGS` bit for negative result.
pub const FLAGS_N: u16 = 1 << 1;
/// `FLAGS` bit for carry/borrow.
pub const FLAGS_C: u16 = 1 << 2;
/// `FLAGS` bit for signed overflow.
pub const FLAGS_V: u16 = 1 << 3;
/// `FLAGS` bit for event enable.
pub const FLAGS_I: u16 = 1 << 4;
/// `FLAGS` bit for fault latched.
pub const FLAGS_F: u16 = 1 << 5;
/// Mask of architecturally active `FLAGS` bits (`Z/N/C/V/I/F`).
pub const FLAGS_ACTIVE_MASK: u16 = FLAGS_Z | FLAGS_N | FLAGS_C | FLAGS_V | FLAGS_I | FLAGS_F;
/// Authority-profile default capability mask (`CAP[0..3] = 1`).
pub const CAP_AUTHORITY_DEFAULT_MASK: u16 = 0x000F;

/// Architecturally visible general-purpose register identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
#[allow(missing_docs)]
pub enum GeneralRegister {
    R0 = 0,
    R1 = 1,
    R2 = 2,
    R3 = 3,
    R4 = 4,
    R5 = 5,
    R6 = 6,
    R7 = 7,
}

impl GeneralRegister {
    /// Ordered list of all architectural general-purpose registers.
    pub const ALL: [Self; GENERAL_REGISTER_COUNT] = [
        Self::R0,
        Self::R1,
        Self::R2,
        Self::R3,
        Self::R4,
        Self::R5,
        Self::R6,
        Self::R7,
    ];

    /// Returns the array index for this register (`0..=7`).
    #[must_use]
    pub const fn index(self) -> usize {
        self as usize
    }

    /// Decodes a 3-bit register field into an architectural register.
    #[must_use]
    pub const fn from_u3(bits: u8) -> Option<Self> {
        match bits {
            0 => Some(Self::R0),
            1 => Some(Self::R1),
            2 => Some(Self::R2),
            3 => Some(Self::R3),
            4 => Some(Self::R4),
            5 => Some(Self::R5),
            6 => Some(Self::R6),
            7 => Some(Self::R7),
            _ => None,
        }
    }
}

/// Full architectural register state for the Nullbyte One core.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct ArchitecturalState {
    gpr: [u16; GENERAL_REGISTER_COUNT],
    pc: u16,
    sp: u16,
    flags: u16,
    tick: u16,
    cap: u16,
    cause: u16,
    evp: u16,
}

impl Default for ArchitecturalState {
    fn default() -> Self {
        Self {
            gpr: [0; GENERAL_REGISTER_COUNT],
            pc: 0,
            sp: 0,
            flags: 0,
            tick: 0,
            cap: CAP_AUTHORITY_DEFAULT_MASK,
            cause: 0,
            evp: 0,
        }
    }
}

impl ArchitecturalState {
    /// Reads a general-purpose register.
    #[must_use]
    pub const fn gpr(&self, reg: GeneralRegister) -> u16 {
        self.gpr[reg.index()]
    }

    /// Writes a general-purpose register.
    pub const fn set_gpr(&mut self, reg: GeneralRegister, value: u16) {
        self.gpr[reg.index()] = value;
    }

    /// Reads the `PC` register.
    #[must_use]
    pub const fn pc(&self) -> u16 {
        self.pc
    }

    /// Writes the `PC` register.
    pub const fn set_pc(&mut self, value: u16) {
        self.pc = value;
    }

    /// Reads the `SP` register.
    #[must_use]
    pub const fn sp(&self) -> u16 {
        self.sp
    }

    /// Writes the `SP` register.
    pub const fn set_sp(&mut self, value: u16) {
        self.sp = value;
    }

    /// Reads the `FLAGS` register.
    #[must_use]
    pub const fn flags(&self) -> u16 {
        self.flags
    }

    /// Writes the `FLAGS` register.
    pub const fn set_flags(&mut self, value: u16) {
        self.flags = value & FLAGS_ACTIVE_MASK;
    }

    /// Returns `true` when a specific `FLAGS` bit is set.
    #[must_use]
    pub const fn flag_is_set(&self, flag: u16) -> bool {
        (self.flags & flag) != 0
    }

    /// Sets or clears a specific active `FLAGS` bit.
    pub const fn set_flag(&mut self, flag: u16, enabled: bool) {
        if enabled {
            self.flags |= flag & FLAGS_ACTIVE_MASK;
        } else {
            self.flags &= !(flag & FLAGS_ACTIVE_MASK);
        }
    }

    /// Reads the `TICK` register.
    #[must_use]
    pub const fn tick(&self) -> u16 {
        self.tick
    }

    /// Writes the `TICK` register.
    pub const fn set_tick(&mut self, value: u16) {
        self.tick = value;
    }

    /// Reads the `CAP` register.
    #[must_use]
    pub const fn cap(&self) -> u16 {
        self.cap
    }

    /// Architecturally-visible writes to `CAP` are ignored.
    pub const fn set_cap(&mut self, _value: u16) {}

    /// Reads the `CAUSE` register.
    #[must_use]
    pub const fn cause(&self) -> u16 {
        self.cause
    }

    /// Writes the `CAUSE` register.
    pub const fn set_cause(&mut self, value: u16) {
        self.cause = value;
    }

    /// Reads the `EVP` register.
    #[must_use]
    pub const fn evp(&self) -> u16 {
        self.evp
    }

    /// Architecturally-visible writes to `EVP` are ignored.
    pub const fn set_evp(&mut self, _value: u16) {}

    /// Core-owned update path for `EVP` (event-pending bitmap ownership model).
    pub const fn set_evp_core_owned(&mut self, value: u16) {
        self.evp = value;
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ArchitecturalState, GeneralRegister, CAP_AUTHORITY_DEFAULT_MASK, FLAGS_ACTIVE_MASK,
        FLAGS_C, FLAGS_F, FLAGS_I, FLAGS_N, FLAGS_V, FLAGS_Z, GENERAL_REGISTER_COUNT,
    };

    #[test]
    fn register_count_and_decode_match_architecture() {
        assert_eq!(GENERAL_REGISTER_COUNT, 8);

        for bits in 0_u8..=7 {
            let reg = GeneralRegister::from_u3(bits).expect("valid 3-bit register encoding");
            assert_eq!(reg.index(), usize::from(bits));
        }

        assert!(GeneralRegister::from_u3(8).is_none());
    }

    #[test]
    fn general_register_file_tracks_each_register_independently() {
        let mut state = ArchitecturalState::default();

        for (offset, reg) in (0_u16..).zip(GeneralRegister::ALL.iter().copied()) {
            state.set_gpr(reg, 0x1000 + offset);
        }

        for (offset, reg) in (0_u16..).zip(GeneralRegister::ALL.iter().copied()) {
            assert_eq!(state.gpr(reg), 0x1000 + offset);
        }
    }

    #[test]
    fn special_registers_are_present_and_readable() {
        let mut state = ArchitecturalState::default();

        state.set_pc(0x0102);
        state.set_sp(0xA0B0);
        state.set_flags(0x001F);
        state.set_tick(123);
        state.set_cap(0xA5A5);
        state.set_cause(0x00AA);
        state.set_evp_core_owned(0x00C3);

        assert_eq!(state.pc(), 0x0102);
        assert_eq!(state.sp(), 0xA0B0);
        assert_eq!(state.flags(), 0x001F);
        assert_eq!(state.tick(), 123);
        assert_eq!(state.cap(), CAP_AUTHORITY_DEFAULT_MASK);
        assert_eq!(state.cause(), 0x00AA);
        assert_eq!(state.evp(), 0x00C3);
    }

    #[test]
    fn cap_defaults_to_authority_mask_and_ignores_architectural_writes() {
        let mut state = ArchitecturalState::default();
        assert_eq!(state.cap(), CAP_AUTHORITY_DEFAULT_MASK);

        state.set_cap(0);
        assert_eq!(state.cap(), CAP_AUTHORITY_DEFAULT_MASK);

        state.set_cap(u16::MAX);
        assert_eq!(state.cap(), CAP_AUTHORITY_DEFAULT_MASK);
    }

    #[test]
    fn evp_defaults_to_zero_and_ignores_architectural_writes() {
        let mut state = ArchitecturalState::default();
        assert_eq!(state.evp(), 0);

        state.set_evp(0x1234);
        assert_eq!(state.evp(), 0);
    }

    #[test]
    fn evp_core_owned_updates_are_preserved_against_architectural_writes() {
        let mut state = ArchitecturalState::default();

        state.set_evp_core_owned(0x00C3);
        assert_eq!(state.evp(), 0x00C3);

        state.set_evp(0xFFFF);
        assert_eq!(state.evp(), 0x00C3);
    }

    #[test]
    fn flags_only_store_active_architectural_bits() {
        let mut state = ArchitecturalState::default();
        state.set_flags(u16::MAX);

        assert_eq!(state.flags(), FLAGS_ACTIVE_MASK);
    }

    #[test]
    fn flags_individual_bits_can_be_set_and_cleared() {
        let mut state = ArchitecturalState::default();

        for flag in [FLAGS_Z, FLAGS_N, FLAGS_C, FLAGS_V, FLAGS_I, FLAGS_F] {
            state.set_flag(flag, true);
            assert!(state.flag_is_set(flag));
        }

        for flag in [FLAGS_Z, FLAGS_N, FLAGS_C, FLAGS_V, FLAGS_I, FLAGS_F] {
            state.set_flag(flag, false);
            assert!(!state.flag_is_set(flag));
        }

        assert_eq!(state.flags(), 0);
    }
}
