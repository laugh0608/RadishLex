use radishlex_ime_core::{Key, KeyEvent as CoreKeyEvent, KeyModifiers, KeyPhase, NamedKey};

use crate::error::FfiError;

pub const RADISHLEX_KEY_KIND_CHAR: u32 = 1;
pub const RADISHLEX_KEY_KIND_NAMED: u32 = 2;

pub const RADISHLEX_NAMED_KEY_SPACE: u32 = 1;
pub const RADISHLEX_NAMED_KEY_ENTER: u32 = 2;
pub const RADISHLEX_NAMED_KEY_BACKSPACE: u32 = 3;
pub const RADISHLEX_NAMED_KEY_ESCAPE: u32 = 4;
pub const RADISHLEX_NAMED_KEY_TAB: u32 = 5;
pub const RADISHLEX_NAMED_KEY_ARROW_UP: u32 = 6;
pub const RADISHLEX_NAMED_KEY_ARROW_DOWN: u32 = 7;
pub const RADISHLEX_NAMED_KEY_ARROW_LEFT: u32 = 8;
pub const RADISHLEX_NAMED_KEY_ARROW_RIGHT: u32 = 9;
pub const RADISHLEX_NAMED_KEY_PAGE_UP: u32 = 10;
pub const RADISHLEX_NAMED_KEY_PAGE_DOWN: u32 = 11;
pub const RADISHLEX_NAMED_KEY_SHIFT: u32 = 12;
pub const RADISHLEX_NAMED_KEY_CONTROL: u32 = 13;
pub const RADISHLEX_NAMED_KEY_ALT: u32 = 14;
pub const RADISHLEX_NAMED_KEY_META: u32 = 15;
pub const RADISHLEX_NAMED_KEY_UNKNOWN: u32 = 255;

pub const RADISHLEX_KEY_MOD_SHIFT: u32 = 1 << 0;
pub const RADISHLEX_KEY_MOD_CONTROL: u32 = 1 << 1;
pub const RADISHLEX_KEY_MOD_ALT: u32 = 1 << 2;
pub const RADISHLEX_KEY_MOD_META: u32 = 1 << 3;

pub const RADISHLEX_KEY_PHASE_PRESS: u32 = 1;
pub const RADISHLEX_KEY_PHASE_RELEASE: u32 = 2;

const RADISHLEX_KEY_MOD_ALL: u32 = RADISHLEX_KEY_MOD_SHIFT
    | RADISHLEX_KEY_MOD_CONTROL
    | RADISHLEX_KEY_MOD_ALT
    | RADISHLEX_KEY_MOD_META;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RadishLexKeyEvent {
    pub key_kind: u32,
    pub codepoint: u32,
    pub named_key: u32,
    pub modifiers: u32,
    pub phase: u32,
}

impl RadishLexKeyEvent {
    pub const fn press_char(ch: char) -> Self {
        Self {
            key_kind: RADISHLEX_KEY_KIND_CHAR,
            codepoint: ch as u32,
            named_key: 0,
            modifiers: 0,
            phase: RADISHLEX_KEY_PHASE_PRESS,
        }
    }

    pub const fn press_named(named_key: u32) -> Self {
        Self {
            key_kind: RADISHLEX_KEY_KIND_NAMED,
            codepoint: 0,
            named_key,
            modifiers: 0,
            phase: RADISHLEX_KEY_PHASE_PRESS,
        }
    }
}

impl TryFrom<RadishLexKeyEvent> for CoreKeyEvent {
    type Error = FfiError;

    fn try_from(event: RadishLexKeyEvent) -> Result<Self, Self::Error> {
        let key = match event.key_kind {
            RADISHLEX_KEY_KIND_CHAR => {
                let ch = char::from_u32(event.codepoint).ok_or_else(|| {
                    FfiError::invalid_argument(
                        "key event codepoint is not a valid Unicode scalar value",
                    )
                })?;
                Key::Char(ch)
            }
            RADISHLEX_KEY_KIND_NAMED => Key::Named(named_key_from_code(event.named_key)?),
            _ => {
                return Err(FfiError::invalid_argument(format!(
                    "unknown key event kind {}",
                    event.key_kind
                )));
            }
        };

        Ok(Self::new(
            key,
            modifiers_from_bits(event.modifiers)?,
            phase_from_code(event.phase)?,
        ))
    }
}

fn named_key_from_code(code: u32) -> Result<NamedKey, FfiError> {
    match code {
        RADISHLEX_NAMED_KEY_SPACE => Ok(NamedKey::Space),
        RADISHLEX_NAMED_KEY_ENTER => Ok(NamedKey::Enter),
        RADISHLEX_NAMED_KEY_BACKSPACE => Ok(NamedKey::Backspace),
        RADISHLEX_NAMED_KEY_ESCAPE => Ok(NamedKey::Escape),
        RADISHLEX_NAMED_KEY_TAB => Ok(NamedKey::Tab),
        RADISHLEX_NAMED_KEY_ARROW_UP => Ok(NamedKey::ArrowUp),
        RADISHLEX_NAMED_KEY_ARROW_DOWN => Ok(NamedKey::ArrowDown),
        RADISHLEX_NAMED_KEY_ARROW_LEFT => Ok(NamedKey::ArrowLeft),
        RADISHLEX_NAMED_KEY_ARROW_RIGHT => Ok(NamedKey::ArrowRight),
        RADISHLEX_NAMED_KEY_PAGE_UP => Ok(NamedKey::PageUp),
        RADISHLEX_NAMED_KEY_PAGE_DOWN => Ok(NamedKey::PageDown),
        RADISHLEX_NAMED_KEY_SHIFT => Ok(NamedKey::Shift),
        RADISHLEX_NAMED_KEY_CONTROL => Ok(NamedKey::Control),
        RADISHLEX_NAMED_KEY_ALT => Ok(NamedKey::Alt),
        RADISHLEX_NAMED_KEY_META => Ok(NamedKey::Meta),
        RADISHLEX_NAMED_KEY_UNKNOWN => Ok(NamedKey::Unknown),
        _ => Err(FfiError::invalid_argument(format!(
            "unknown named key code {code}"
        ))),
    }
}

fn modifiers_from_bits(bits: u32) -> Result<KeyModifiers, FfiError> {
    let unknown_bits = bits & !RADISHLEX_KEY_MOD_ALL;
    if unknown_bits != 0 {
        return Err(FfiError::invalid_argument(format!(
            "unknown key modifier bits 0x{unknown_bits:x}"
        )));
    }

    Ok(KeyModifiers::new(
        bits & RADISHLEX_KEY_MOD_SHIFT != 0,
        bits & RADISHLEX_KEY_MOD_CONTROL != 0,
        bits & RADISHLEX_KEY_MOD_ALT != 0,
        bits & RADISHLEX_KEY_MOD_META != 0,
    ))
}

fn phase_from_code(code: u32) -> Result<KeyPhase, FfiError> {
    match code {
        RADISHLEX_KEY_PHASE_PRESS => Ok(KeyPhase::Press),
        RADISHLEX_KEY_PHASE_RELEASE => Ok(KeyPhase::Release),
        _ => Err(FfiError::invalid_argument(format!(
            "unknown key phase code {code}"
        ))),
    }
}
