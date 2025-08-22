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
use x11rb::protocol::xtest::ConnectionExt as _; // for xtest_fake_input
use x11rb::rust_connection::RustConnection;

// Import shared modifier bits from lib.rs
use crate::{MOD_SHIFT, MOD_CTRL, MOD_ALT, MOD_META};

// --- KeySym constants we care about ---
const XK_BackSpace:  u32 = 0xFF08;
const XK_Tab:        u32 = 0xFF09;
const XK_Return:     u32 = 0xFF0D;
const XK_Escape:     u32 = 0xFF1B;
const XK_Delete:     u32 = 0xFFFF;
const XK_KP_Enter:   u32 = 0xFF8D;

// Modifiers: we’ll try these in this order (left preferred)
const XK_Shift_L:    u32 = 0xFFE1;
const XK_Shift_R:    u32 = 0xFFE2;
const XK_Control_L:  u32 = 0xFFE3;
const XK_Control_R:  u32 = 0xFFE4;
const XK_Alt_L:      u32 = 0xFFE9;
const XK_Alt_R:      u32 = 0xFFEA;
const XK_Meta_L:     u32 = 0xFFE7; // sometimes "Super" is used instead of "Meta"
const XK_Meta_R:     u32 = 0xFFE8;
const XK_Super_L:    u32 = 0xFFEB; // fallback when Meta is not present
const XK_Super_R:    u32 = 0xFFEC;

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
    alt_code:   Option<u8>,
    meta_code:  Option<u8>, // Meta or Super
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

        // Modifiers (left-preferred)
        let shift_code = pick_first(&keysym_to_code, &[XK_Shift_L, XK_Shift_R]);
        let ctrl_code  = pick_first(&keysym_to_code, &[XK_Control_L, XK_Control_R]);
        let alt_code   = pick_first(&keysym_to_code, &[XK_Alt_L, XK_Alt_R]);

        // Meta can be mapped as Meta or Super depending on the server
        let meta_code  = pick_first(&keysym_to_code, &[XK_Meta_L, XK_Meta_R, XK_Super_L, XK_Super_R]);

        debug!(
            "input: Return→{:?}, KP_Enter→{:?}, 'e'→{:?}, Shift→{:?}, Ctrl→{:?}, Alt→{:?}, Meta→{:?}",
            keysym_to_code.get(&XK_Return),
            keysym_to_code.get(&XK_KP_Enter),
            keysym_to_code.get(&(b'e' as u32)),
            shift_code, ctrl_code, alt_code, meta_code,
        );

        Self {
            conn, root, keysym_to_code, min_code, max_code,
            shift_code, ctrl_code, alt_code, meta_code
        }
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

    /// Key down (protocol: base byte + modifiers bitmask).
    pub fn key_down(&mut self, key: u8, mods: u16) {
        let (ks, code, extra_shift) = self.resolve_from_byte(key);
        let need_shift = extra_shift || (mods & MOD_SHIFT) != 0;
        let need_ctrl  = (mods & MOD_CTRL)  != 0;
        let need_alt   = (mods & MOD_ALT)   != 0;
        let need_meta  = (mods & MOD_META)  != 0;

        debug!(
            "key_down: byte={} ks={:#06x?} code={:?} mods=0x{:x} [shift={}, ctrl={}, alt={}, meta={}]",
            key, ks.unwrap_or(0), code, mods, need_shift, need_ctrl, need_alt, need_meta
        );

        if let Some(code) = code {
            // Press required modifiers, then the key
            if need_ctrl { if let Some(c) = self.ctrl_code { self.press_code(c); } }
            if need_alt  { if let Some(c) = self.alt_code  { self.press_code(c); } }
            if need_meta { if let Some(c) = self.meta_code { self.press_code(c); } }
            if need_shift{ if let Some(c) = self.shift_code{ self.press_code(c); } }

            self.press_code(code);
            self.conn.flush().unwrap();
        }
    }

    /// Key up (release key first, then modifiers we synthesized).
    pub fn key_up(&mut self, key: u8, mods: u16) {
        let (ks, code, extra_shift) = self.resolve_from_byte(key);
        let need_shift = extra_shift || (mods & MOD_SHIFT) != 0;
        let need_ctrl  = (mods & MOD_CTRL)  != 0;
        let need_alt   = (mods & MOD_ALT)   != 0;
        let need_meta  = (mods & MOD_META)  != 0;

        debug!(
            "key_up:   byte={} ks={:#06x?} code={:?} mods=0x{:x} [shift={}, ctrl={}, alt={}, meta={}]",
            key, ks.unwrap_or(0), code, mods, need_shift, need_ctrl, need_alt, need_meta
        );

        if let Some(code) = code {
            self.release_code(code);

            if need_shift{ if let Some(c) = self.shift_code{ self.release_code(c); } }
            if need_meta { if let Some(c) = self.meta_code { self.release_code(c); } }
            if need_alt  { if let Some(c) = self.alt_code  { self.release_code(c); } }
            if need_ctrl { if let Some(c) = self.ctrl_code { self.release_code(c); } }

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

    /// Convert a single byte (ASCII-like) into (keysym, keycode, extra_shift_needed).
    /// We accept:
    /// - 8,9,10/13,27,127 for Backspace/Tab/Return/Escape/Delete
    /// - 32..=126 printable ASCII:
    ///     * If directly bound, use it.
    ///     * If uppercase letter, try lowercase + extra Shift.
    ///     * If shifted punctuation ('_', '+', etc.), map to base ('-', '=') + extra Shift.
    fn resolve_from_byte(&self, key: u8) -> (Option<u32>, Option<u8>, bool) {
        // Controls
        match key {
            8   => { let c = self.keysym_to_code.get(&XK_BackSpace).copied();
                     return (Some(XK_BackSpace), c, false); }
            9   => { let c = self.keysym_to_code.get(&XK_Tab).copied();
                     return (Some(XK_Tab), c, false); }
            10 | 13 => {
                if let Some(c) = self.keysym_to_code.get(&XK_Return).copied() {
                    return (Some(XK_Return), Some(c), false);
                }
                if let Some(c) = self.keysym_to_code.get(&XK_KP_Enter).copied() {
                    return (Some(XK_KP_Enter), Some(c), false);
                }
                return (Some(XK_Return), None, false);
            }
            27  => { let c = self.keysym_to_code.get(&XK_Escape).copied();
                     return (Some(XK_Escape), c, false); }
            127 => { let c = self.keysym_to_code.get(&XK_Delete).copied();
                     return (Some(XK_Delete), c, false); }
            _ => {}
        }

        // Printable ASCII
        if (32..=126).contains(&key) {
            let ks = key as u32;

            // Direct binding?
            if let Some(&code) = self.keysym_to_code.get(&ks) {
                return (Some(ks), Some(code), false);
            }

            // Uppercase letter? try lowercase + shift
            if key.is_ascii_uppercase() {
                let base = key.to_ascii_lowercase() as u32;
                if let Some(&code) = self.keysym_to_code.get(&base) {
                    return (Some(ks), Some(code), true);
                }
            }

            // Shifted symbol? map back to base + shift (US layout)
            if let Some(base_char) = shifted_symbol_base(key) {
                let base_ks = base_char as u32;
                if let Some(&code) = self.keysym_to_code.get(&base_ks) {
                    return (Some(ks), Some(code), true);
                }
            }

            // Unshifted punctuation/digits should have been covered by "direct binding".
            return (Some(ks), None, false);
        }

        // We *could* treat values inside [min_code,max_code] as a raw X11 keycode,
        // but our protocol sends ASCII-ish bytes, so ignore that path by default.
        (None, None, false)
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

/// Pick the first available keycode for a list of possible modifier keysyms.
fn pick_first(map: &HashMap<u32, u8>, candidates: &[u32]) -> Option<u8> {
    for ks in candidates {
        if let Some(&c) = map.get(ks) {
            return Some(c);
        }
    }
    None
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
