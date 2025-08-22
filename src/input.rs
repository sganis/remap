#![cfg(target_os = "linux")]

use std::collections::HashMap;

use log::debug;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{
    ConnectionExt as _,
    GetKeyboardMappingReply,
    Window,
    KEY_PRESS_EVENT,
    KEY_RELEASE_EVENT,
    BUTTON_PRESS_EVENT,
    BUTTON_RELEASE_EVENT,
    MOTION_NOTIFY_EVENT,
};
use x11rb::protocol::xtest::ConnectionExt as _;
use x11rb::rust_connection::RustConnection;

// --- KeySym constants we care about ---
const XK_BackSpace:  u32 = 0xFF08;
const XK_Tab:        u32 = 0xFF09;
const XK_Return:     u32 = 0xFF0D;
const XK_Escape:     u32 = 0xFF1B;
const XK_Delete:     u32 = 0xFFFF;
const XK_KP_Enter:   u32 = 0xFF8D;

const XK_Shift_L:    u32 = 0xFFE1;
const XK_Shift_R:    u32 = 0xFFE2;
const XK_Control_L:  u32 = 0xFFE3;
const XK_Control_R:  u32 = 0xFFE4;

/// Simple XTEST-based input injector (no enigo).
pub struct Input {
    conn: RustConnection,
    root: Window,
    /// map Keysym (u32) -> keycode (u8)
    keysym_to_code: HashMap<u32, u8>,
    /// server keycode range (inclusive)
    min_code: u8,
    max_code: u8,
    /// convenient modifier keycodes (left-preferred)
    shift_code: Option<u8>,
    ctrl_code:  Option<u8>,
}

impl Input {
    pub fn new() -> Self {
        // DISPLAY must be set by the server launcher (Xvfb/Xorg etc.)
        let (conn, screen_num) = x11rb::connect(None).expect("X11 connect failed");
        let setup = conn.setup();
        let screen = &setup.roots[screen_num];
        let root = screen.root;

        // Make sure XTEST exists (most servers have it enabled)
        let _ = conn.xtest_get_version(2, 2).unwrap().reply().unwrap();

        let min_code = setup.min_keycode.into();
        let max_code = setup.max_keycode.into();
        debug!("input: server keycode range = [{min_code}, {max_code}]");

        // Build a keysym -> keycode map from the server's keyboard mapping
        let mapping = fetch_keyboard_mapping(&conn);
        let keysym_to_code = invert_keyboard_mapping(&mapping, min_code);

        // Modifiers
        let shift_code = keysym_to_code
            .get(&XK_Shift_L).copied()
            .or_else(|| keysym_to_code.get(&XK_Shift_R).copied());
        let ctrl_code = keysym_to_code
            .get(&XK_Control_L).copied()
            .or_else(|| keysym_to_code.get(&XK_Control_R).copied());

        debug!(
            "input: Return→{:?}, KP_Enter→{:?}, 'e'→{:?}, Shift→{:?}, Ctrl→{:?}",
            keysym_to_code.get(&XK_Return),
            keysym_to_code.get(&XK_KP_Enter),
            keysym_to_code.get(&(b'e' as u32)),
            shift_code,
            ctrl_code,
        );

        Self { conn, root, keysym_to_code, min_code, max_code, shift_code, ctrl_code }
    }

    pub fn set_window(&mut self, _xid: i32) {}
    pub fn focus(&mut self) {}
    pub fn set_server_geometry(&mut self, _geom: crate::Geometry) {}

    /// Move mouse to absolute screen coordinates.
    pub fn mouse_move(&mut self, x: i32, y: i32, _mods: u32) {
        let (x16, y16) = (x as i16, y as i16);
        self.conn
            .xtest_fake_input(MOTION_NOTIFY_EVENT, 0, 0, self.root, x16, y16, 0)
            .unwrap();
        self.conn.flush().unwrap();
    }

    /// Click mouse button: 1=Left, 2=Middle, 3=Right.
    pub fn mouse_click(&mut self, x: i32, y: i32, button: u32, _mods: u32) {
        let (x16, y16) = (x as i16, y as i16);
        let detail = match button {
            1 | 2 | 3 => button as u8,
            _ => 1,
        };
        self.conn
            .xtest_fake_input(BUTTON_PRESS_EVENT, detail, 0, self.root, x16, y16, 0)
            .unwrap();
        self.conn
            .xtest_fake_input(BUTTON_RELEASE_EVENT, detail, 0, self.root, x16, y16, 0)
            .unwrap();
        self.conn.flush().unwrap();
    }

    /// Key down.
    pub fn key_down(&mut self, key: u8) {
        let (ks, code, need_shift, need_ctrl) = self.resolve_keycode_and_mods(key);
        debug!(
            "key_down: byte={key} ks={:#06x?} code={code:?} shift={need_shift} ctrl={need_ctrl}",
            ks.unwrap_or(0)
        );
        if let Some(code) = code {
            // Press required modifiers then the key
            if need_ctrl {
                if let Some(c) = self.ctrl_code { self.press_code(c); }
            }
            if need_shift {
                if let Some(c) = self.shift_code { self.press_code(c); }
            }
            self.press_code(code);
            self.conn.flush().unwrap();
        }
    }

    /// Key up.
    pub fn key_up(&mut self, key: u8) {
        let (ks, code, need_shift, need_ctrl) = self.resolve_keycode_and_mods(key);
        debug!(
            "key_up:   byte={key} ks={:#06x?} code={code:?} shift={need_shift} ctrl={need_ctrl}",
            ks.unwrap_or(0)
        );
        if let Some(code) = code {
            // Release key then release modifiers we synthesized
            self.release_code(code);
            if need_shift {
                if let Some(c) = self.shift_code { self.release_code(c); }
            }
            if need_ctrl {
                if let Some(c) = self.ctrl_code { self.release_code(c); }
            }
            self.conn.flush().unwrap();
        }
    }

    // --- Low-level helpers ---
    fn press_code(&mut self, code: u8) {
        let _ = self.conn.xtest_fake_input(KEY_PRESS_EVENT, code, 0, self.root, 0, 0, 0);
    }
    fn release_code(&mut self, code: u8) {
        let _ = self.conn.xtest_fake_input(KEY_RELEASE_EVENT, code, 0, self.root, 0, 0, 0);
    }

    /// Resolve the incoming byte to (keysym, keycode, need_shift, need_ctrl).
    /// Strategy:
    ///  1) Control bytes 0x01..0x1A  → Ctrl + (A..Z).
    ///  2) Special controls/backspace/tab/enter/esc/del.
    ///  3) Printable ASCII:
    ///     a) If directly bound → use it.
    ///     b) If uppercase or shifted symbol → try base char + Shift.
    ///  4) If byte is in [min_code,max_code], treat it as a raw X11 keycode.
    fn resolve_keycode_and_mods(&self, key: u8) -> (Option<u32>, Option<u8>, bool, bool) {
        // 1) CTRL-A..CTRL-Z
        if (1..=26).contains(&key) {
            let base = b'a' + (key - 1); // 'a'..'z'
            let (code, ks) = self.find_letter_code_any_case(base);
            return (Some(ks), code, false, true);
        }

        // 2) Common controls
        match key {
            8   => return (Some(XK_BackSpace), self.keysym_to_code.get(&XK_BackSpace).copied(), false, false),
            9   => return (Some(XK_Tab),       self.keysym_to_code.get(&XK_Tab).copied(),       false, false),
            10 | 13 =>
                // Return or KP_Enter
                return if let Some(c) = self.keysym_to_code.get(&XK_Return).copied() {
                    (Some(XK_Return), Some(c), false, false)
                } else if let Some(c) = self.keysym_to_code.get(&XK_KP_Enter).copied() {
                    (Some(XK_KP_Enter), Some(c), false, false)
                } else {
                    (Some(XK_Return), None, false, false)
                },
            27  => return (Some(XK_Escape),    self.keysym_to_code.get(&XK_Escape).copied(),    false, false),
            127 => return (Some(XK_Delete),    self.keysym_to_code.get(&XK_Delete).copied(),    false, false),
            _ => {}
        }

        // 3) Printable ASCII
        if (32..=126).contains(&key) {
            let ks = key as u32;

            // 3a) Direct binding?
            if let Some(&code) = self.keysym_to_code.get(&ks) {
                return (Some(ks), Some(code), false, false);
            }

            // 3b) If uppercase letter, try lowercase + Shift
            if key.is_ascii_uppercase() {
                let base = key.to_ascii_lowercase() as u32;
                if let Some(&code) = self.keysym_to_code.get(&base) {
                    return (Some(ks), Some(code), true, false);
                }
            }

            // 3c) If shifted symbol, try base char + Shift (US layout)
            if let Some(base_char) = shifted_symbol_base(key) {
                let base_ks = base_char as u32;
                if let Some(&code) = self.keysym_to_code.get(&base_ks) {
                    return (Some(ks), Some(code), true, false);
                }
            }

            // Unshifted punctuation/digits should have been covered by 3a; if not bound, fall through.
            return (Some(ks), None, false, false);
        }

        // 4) Raw keycode fallback (transport might have sent an X11 keycode)
        if key >= self.min_code && key <= self.max_code {
            return (None, Some(key), false, false);
        }

        (None, None, false, false)
    }

    /// Find a letter keycode for either case; returns (code, chosen_keysym).
    fn find_letter_code_any_case(&self, lower: u8) -> (Option<u8>, u32) {
        let lower_ks = (lower as u32);
        if let Some(&c) = self.keysym_to_code.get(&lower_ks) {
            return (Some(c), lower_ks);
        }
        let upper_ks = (lower.to_ascii_uppercase() as u32);
        if let Some(&c) = self.keysym_to_code.get(&upper_ks) {
            return (Some(c), upper_ks);
        }
        (None, lower_ks)
    }
}

/// Map shifted symbols (US layout) back to their unshifted base.
/// Example: '!' → '1', '_' → '-', '{' → '[', etc.
fn shifted_symbol_base(ch: u8) -> Option<u8> {
    match ch {
        b'!' => Some(b'1'),
        b'@' => Some(b'2'),
        b'#' => Some(b'3'),
        b'$' => Some(b'4'),
        b'%' => Some(b'5'),
        b'^' => Some(b'6'),
        b'&' => Some(b'7'),
        b'*' => Some(b'8'),
        b'(' => Some(b'9'),
        b')' => Some(b'0'),
        b'_' => Some(b'-'),
        b'+' => Some(b'='),
        b'{' => Some(b'['),
        b'}' => Some(b']'),
        b'|' => Some(b'\\'),
        b':' => Some(b';'),
        b'"' => Some(b'\''),
        b'<' => Some(b','),
        b'>' => Some(b'.'),
        b'?' => Some(b'/'),
        b'~' => Some(b'`'),
        _ => None,
    }
}

/// Query the keyboard mapping from the server.
fn fetch_keyboard_mapping(conn: &RustConnection) -> GetKeyboardMappingReply {
    let setup = conn.setup();
    let min = setup.min_keycode;
    let max = setup.max_keycode;
    let keycode_count = u8::from(max).saturating_sub(u8::from(min)) + 1;
    conn.get_keyboard_mapping(min, keycode_count)
        .unwrap()
        .reply()
        .unwrap()
}

/// Invert the keyboard mapping: Keysym -> keycode (first match wins).
/// IMPORTANT: base keycode is the server's min_keycode (do NOT assume 8).
fn invert_keyboard_mapping(map: &GetKeyboardMappingReply, min_code: u8) -> HashMap<u32, u8> {
    let mut h = HashMap::new();
    let keysyms_per_keycode = map.keysyms_per_keycode as usize;
    for (i, chunk) in map.keysyms.chunks(keysyms_per_keycode).enumerate() {
        let keycode = min_code.saturating_add(i as u8);
        for &ks in chunk {
            if ks != 0 && !h.contains_key(&ks) {
                h.insert(ks, keycode);
            }
        }
    }
    h
}
