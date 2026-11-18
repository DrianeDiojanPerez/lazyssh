//! A program that turns the mouse on (nvim, htop, less --mouse) expects the
//! terminal to type each click back in as an escape sequence. lazyssh owns the
//! real mouse, so it writes those sequences itself for the session it shows.

use crossterm::event::{KeyModifiers, MouseButton, MouseEventKind};
use vt100::{MouseProtocolEncoding, MouseProtocolMode};

const RELEASE: u8 = 3;
const MOTION: u8 = 32;
const WHEEL: u8 = 64;

/// The bytes that report `kind` at `column` and `row` (both from zero, inside
/// the session) the way the program asked, or nothing if it did not ask for
/// this kind of event.
pub fn encode(
    mode: MouseProtocolMode,
    encoding: MouseProtocolEncoding,
    kind: MouseEventKind,
    modifiers: KeyModifiers,
    column: u16,
    row: u16,
) -> Option<Vec<u8>> {
    let (button, released) = match kind {
        MouseEventKind::Down(button) => (code(button), false),
        MouseEventKind::Up(button) if mode != MouseProtocolMode::Press => (code(button), true),
        MouseEventKind::Drag(button)
            if matches!(mode, MouseProtocolMode::ButtonMotion | MouseProtocolMode::AnyMotion) =>
        {
            (code(button) + MOTION, false)
        }
        MouseEventKind::Moved if mode == MouseProtocolMode::AnyMotion => (RELEASE + MOTION, false),
        MouseEventKind::ScrollUp => (WHEEL, false),
        MouseEventKind::ScrollDown => (WHEEL + 1, false),
        MouseEventKind::ScrollLeft => (WHEEL + 2, false),
        MouseEventKind::ScrollRight => (WHEEL + 3, false),
        _ => return None,
    };

    if mode == MouseProtocolMode::None {
        return None;
    }

    let button = button + held(modifiers);
    let (x, y) = (u32::from(column) + 1, u32::from(row) + 1);

    match encoding {
        MouseProtocolEncoding::Sgr => {
            let end = if released { 'm' } else { 'M' };
            Some(format!("\x1b[<{};{};{}{}", button, x, y, end).into_bytes())
        }
        // INFO: the older encodings have no way to say which button let go,
        // so every release is reported as button 3
        MouseProtocolEncoding::Default => {
            let button = if released { RELEASE + held(modifiers) } else { button };
            let (x, y) = (u8::try_from(32 + x).ok()?, u8::try_from(32 + y).ok()?);
            Some(vec![0x1b, b'[', b'M', 32 + button, x, y])
        }
        MouseProtocolEncoding::Utf8 => {
            let button = if released { RELEASE + held(modifiers) } else { button };
            let mut bytes = vec![0x1b, b'[', b'M', 32 + button];
            for value in [32 + x, 32 + y] {
                let mut buffer = [0; 4];
                bytes.extend_from_slice(char::from_u32(value)?.encode_utf8(&mut buffer).as_bytes());
            }
            Some(bytes)
        }
    }
}

fn code(button: MouseButton) -> u8 {
    match button {
        MouseButton::Left => 0,
        MouseButton::Middle => 1,
        MouseButton::Right => 2,
    }
}

fn held(modifiers: KeyModifiers) -> u8 {
    let mut bits = 0;
    if modifiers.contains(KeyModifiers::SHIFT) {
        bits += 4;
    }
    if modifiers.contains(KeyModifiers::ALT) {
        bits += 8;
    }
    if modifiers.contains(KeyModifiers::CONTROL) {
        bits += 16;
    }
    bits
}

#[cfg(test)]
mod tests {
    use super::*;

    const SGR: MouseProtocolEncoding = MouseProtocolEncoding::Sgr;
    const NONE: KeyModifiers = KeyModifiers::NONE;

    fn sgr(mode: MouseProtocolMode, kind: MouseEventKind, column: u16, row: u16) -> Option<String> {
        encode(mode, SGR, kind, NONE, column, row).map(|bytes| String::from_utf8(bytes).unwrap())
    }

    #[test]
    fn a_click_is_counted_from_one() {
        let click = sgr(MouseProtocolMode::PressRelease, MouseEventKind::Down(MouseButton::Left), 0, 0);

        assert_eq!(click.as_deref(), Some("\x1b[<0;1;1M"));
    }

    #[test]
    fn a_release_ends_in_a_small_m_and_keeps_its_button() {
        let release = sgr(MouseProtocolMode::PressRelease, MouseEventKind::Up(MouseButton::Right), 9, 4);

        assert_eq!(release.as_deref(), Some("\x1b[<2;10;5m"));
    }

    #[test]
    fn the_wheel_is_reported_as_buttons_64_and_65() {
        let mode = MouseProtocolMode::PressRelease;

        assert_eq!(sgr(mode, MouseEventKind::ScrollUp, 2, 3).as_deref(), Some("\x1b[<64;3;4M"));
        assert_eq!(sgr(mode, MouseEventKind::ScrollDown, 2, 3).as_deref(), Some("\x1b[<65;3;4M"));
    }

    #[test]
    fn nothing_is_sent_when_the_program_did_not_ask() {
        let mode = MouseProtocolMode::None;

        assert_eq!(sgr(mode, MouseEventKind::Down(MouseButton::Left), 1, 1), None);
        assert_eq!(sgr(mode, MouseEventKind::ScrollUp, 1, 1), None);
    }

    #[test]
    fn drags_are_sent_only_when_motion_was_asked_for() {
        let drag = MouseEventKind::Drag(MouseButton::Left);

        assert_eq!(sgr(MouseProtocolMode::PressRelease, drag, 4, 4), None);
        assert_eq!(sgr(MouseProtocolMode::ButtonMotion, drag, 4, 4).as_deref(), Some("\x1b[<32;5;5M"));
    }

    #[test]
    fn plain_moves_are_sent_only_in_any_motion_mode() {
        assert_eq!(sgr(MouseProtocolMode::ButtonMotion, MouseEventKind::Moved, 4, 4), None);
        assert_eq!(
            sgr(MouseProtocolMode::AnyMotion, MouseEventKind::Moved, 4, 4).as_deref(),
            Some("\x1b[<35;5;5M")
        );
    }

    #[test]
    fn press_only_mode_drops_releases() {
        assert_eq!(sgr(MouseProtocolMode::Press, MouseEventKind::Up(MouseButton::Left), 0, 0), None);
    }

    #[test]
    fn held_keys_are_added_to_the_button() {
        let click = encode(
            MouseProtocolMode::PressRelease,
            SGR,
            MouseEventKind::Down(MouseButton::Left),
            KeyModifiers::CONTROL | KeyModifiers::ALT,
            0,
            0,
        );

        assert_eq!(click, Some(b"\x1b[<24;1;1M".to_vec()));
    }

    #[test]
    fn the_old_encoding_offsets_everything_by_32() {
        let click = encode(
            MouseProtocolMode::PressRelease,
            MouseProtocolEncoding::Default,
            MouseEventKind::Down(MouseButton::Left),
            NONE,
            9,
            4,
        );

        assert_eq!(click, Some(vec![0x1b, b'[', b'M', 32, 42, 37]));
    }

    #[test]
    fn the_old_encoding_gives_up_past_column_223() {
        let far = encode(
            MouseProtocolMode::PressRelease,
            MouseProtocolEncoding::Default,
            MouseEventKind::Down(MouseButton::Left),
            NONE,
            230,
            0,
        );

        assert_eq!(far, None);
    }

    #[test]
    fn the_utf8_encoding_reaches_past_column_223() {
        let far = encode(
            MouseProtocolMode::PressRelease,
            MouseProtocolEncoding::Utf8,
            MouseEventKind::Down(MouseButton::Left),
            NONE,
            230,
            0,
        );

        let mut expected = vec![0x1b, b'[', b'M', 32];
        expected.extend_from_slice("\u{107}".as_bytes());
        expected.push(33);
        assert_eq!(far, Some(expected));
    }
}
