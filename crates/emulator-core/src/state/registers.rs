/// Number of architecturally visible general-purpose registers (`R0..R7`).
pub const GENERAL_REGISTER_COUNT: usize = 8;

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
#[derive(Debug, Clone, PartialEq, Eq, Default)]
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
        self.flags = value;
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

    /// Writes the `CAP` register.
    pub const fn set_cap(&mut self, value: u16) {
        self.cap = value;
    }

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

    /// Writes the `EVP` register.
    pub const fn set_evp(&mut self, value: u16) {
        self.evp = value;
    }
}

#[cfg(test)]
mod tests {
    use super::{ArchitecturalState, GeneralRegister, GENERAL_REGISTER_COUNT};

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
        state.set_cap(0x000F);
        state.set_cause(0x00AA);
        state.set_evp(0x00C3);

        assert_eq!(state.pc(), 0x0102);
        assert_eq!(state.sp(), 0xA0B0);
        assert_eq!(state.flags(), 0x001F);
        assert_eq!(state.tick(), 123);
        assert_eq!(state.cap(), 0x000F);
        assert_eq!(state.cause(), 0x00AA);
        assert_eq!(state.evp(), 0x00C3);
    }
}
