/// A platform-neutral key event passed into the input core.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyEvent {
    key: Key,
    modifiers: KeyModifiers,
    phase: KeyPhase,
}

impl KeyEvent {
    pub const fn new(key: Key, modifiers: KeyModifiers, phase: KeyPhase) -> Self {
        Self {
            key,
            modifiers,
            phase,
        }
    }

    pub const fn press(key: Key) -> Self {
        Self::new(key, KeyModifiers::empty(), KeyPhase::Press)
    }

    pub const fn press_char(ch: char) -> Self {
        Self::press(Key::Char(ch))
    }

    pub const fn key(&self) -> Key {
        self.key
    }

    pub const fn modifiers(&self) -> KeyModifiers {
        self.modifiers
    }

    pub const fn phase(&self) -> KeyPhase {
        self.phase
    }
}

/// The key identity after a platform shell has normalized native key data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Char(char),
    Named(NamedKey),
}

/// Non-text keys commonly needed by input methods.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NamedKey {
    Space,
    Enter,
    Backspace,
    Escape,
    Tab,
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    PageUp,
    PageDown,
    Shift,
    Control,
    Alt,
    Meta,
    Unknown,
}

/// Keyboard modifier state attached to a key event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct KeyModifiers {
    shift: bool,
    control: bool,
    alt: bool,
    meta: bool,
}

impl KeyModifiers {
    pub const fn new(shift: bool, control: bool, alt: bool, meta: bool) -> Self {
        Self {
            shift,
            control,
            alt,
            meta,
        }
    }

    pub const fn empty() -> Self {
        Self::new(false, false, false, false)
    }

    pub const fn shift(&self) -> bool {
        self.shift
    }

    pub const fn control(&self) -> bool {
        self.control
    }

    pub const fn alt(&self) -> bool {
        self.alt
    }

    pub const fn meta(&self) -> bool {
        self.meta
    }
}

/// Whether the key event represents a press or a release.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyPhase {
    Press,
    Release,
}
