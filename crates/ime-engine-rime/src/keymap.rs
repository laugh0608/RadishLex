use std::ffi::c_int;

use radishlex_ime_core::{Key, KeyEvent, KeyPhase, NamedKey};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RimeKeyInput {
    Character(char),
    Named(RimeNamedKey),
    Ignored,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RimeNamedKey {
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
}

pub fn classify_key_event(event: KeyEvent) -> RimeKeyInput {
    if event.phase() != KeyPhase::Press {
        return RimeKeyInput::Ignored;
    }

    match event.key() {
        Key::Char(ch) if ch.is_ascii_alphanumeric() || ch == '\'' => {
            RimeKeyInput::Character(ch.to_ascii_lowercase())
        }
        Key::Char(_) => RimeKeyInput::Ignored,
        Key::Named(NamedKey::Space) => RimeKeyInput::Named(RimeNamedKey::Space),
        Key::Named(NamedKey::Enter) => RimeKeyInput::Named(RimeNamedKey::Enter),
        Key::Named(NamedKey::Backspace) => RimeKeyInput::Named(RimeNamedKey::Backspace),
        Key::Named(NamedKey::Escape) => RimeKeyInput::Named(RimeNamedKey::Escape),
        Key::Named(NamedKey::Tab) => RimeKeyInput::Named(RimeNamedKey::Tab),
        Key::Named(NamedKey::ArrowUp) => RimeKeyInput::Named(RimeNamedKey::ArrowUp),
        Key::Named(NamedKey::ArrowDown) => RimeKeyInput::Named(RimeNamedKey::ArrowDown),
        Key::Named(NamedKey::ArrowLeft) => RimeKeyInput::Named(RimeNamedKey::ArrowLeft),
        Key::Named(NamedKey::ArrowRight) => RimeKeyInput::Named(RimeNamedKey::ArrowRight),
        Key::Named(NamedKey::PageUp) => RimeKeyInput::Named(RimeNamedKey::PageUp),
        Key::Named(NamedKey::PageDown) => RimeKeyInput::Named(RimeNamedKey::PageDown),
        Key::Named(
            NamedKey::Shift
            | NamedKey::Control
            | NamedKey::Alt
            | NamedKey::Meta
            | NamedKey::Unknown,
        ) => RimeKeyInput::Ignored,
    }
}

#[cfg_attr(not(feature = "native-rime"), allow(dead_code))]
pub fn rime_keycode(input: RimeKeyInput) -> Option<c_int> {
    match input {
        RimeKeyInput::Character(ch) if ch.is_ascii() => Some(ch as c_int),
        RimeKeyInput::Character(_) | RimeKeyInput::Ignored => None,
        RimeKeyInput::Named(named) => Some(match named {
            RimeNamedKey::Space => 0x20,
            RimeNamedKey::Enter => 0xff0d,
            RimeNamedKey::Backspace => 0xff08,
            RimeNamedKey::Escape => 0xff1b,
            RimeNamedKey::Tab => 0xff09,
            RimeNamedKey::ArrowLeft => 0xff51,
            RimeNamedKey::ArrowUp => 0xff52,
            RimeNamedKey::ArrowRight => 0xff53,
            RimeNamedKey::ArrowDown => 0xff54,
            RimeNamedKey::PageUp => 0xff55,
            RimeNamedKey::PageDown => 0xff56,
        }),
    }
}

#[cfg(test)]
mod tests {
    use radishlex_ime_core::{Key, KeyEvent, KeyModifiers, KeyPhase, NamedKey};

    use super::{classify_key_event, rime_keycode, RimeKeyInput, RimeNamedKey};

    #[test]
    fn classifies_ascii_input_without_native_keycodes() {
        let input = classify_key_event(KeyEvent::press_char('L'));
        assert_eq!(input, RimeKeyInput::Character('l'));
    }

    #[test]
    fn classifies_navigation_keys() {
        let input = classify_key_event(KeyEvent::press(Key::Named(NamedKey::PageDown)));
        assert_eq!(input, RimeKeyInput::Named(RimeNamedKey::PageDown));
    }

    #[test]
    fn ignores_key_releases() {
        let input = classify_key_event(KeyEvent::new(
            Key::Char('a'),
            KeyModifiers::empty(),
            KeyPhase::Release,
        ));
        assert_eq!(input, RimeKeyInput::Ignored);
    }

    #[test]
    fn maps_named_keys_to_rime_keycodes() {
        assert_eq!(
            rime_keycode(RimeKeyInput::Named(RimeNamedKey::Backspace)),
            Some(0xff08)
        );
        assert_eq!(
            rime_keycode(RimeKeyInput::Named(RimeNamedKey::Space)),
            Some(0x20)
        );
    }
}
