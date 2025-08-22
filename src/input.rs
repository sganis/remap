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

use crate::{MOD_SHIFT, MOD_CTRL, MOD_ALT, MOD_META};

// --- KeySym constants we care about ---
const XK_BackSpace:  u32 = 0xFF08;
const XK_Tab:        u32 = 0xFF09;
const XK_Return:     u32 = 0xFF0D;
const XK_Escape:     u32 = 0xFF1B;
const XK_Delete:     u32 = 0xFFFF;
const XK_KP_Enter:   u32 = 0xFF8D;

const XK_Home:       u32 = 0xFF50;
const XK_Left:       u32 = 0xFF51;
const XK_Up:         u32 = 0xFF52;
const XK_Right:      u32 = 0xFF53;
const XK_Down:       u32 = 0xFF54;
const XK_Page_Up:    u32 = 0xFF55;
const XK_Page_Down:  u32 = 0xFF56;
const XK_End:        u32 = 0xFF57;
const XK_Insert:     u32 = 0xFF63;

// Modifiers (left-preferred)
const XK_Shift_L:    u32 = 0xFFE1;
const XK_Shift_R:    u32 = 0xFFE2;
const XK_Control_L:  u32 = 0xFFE3;
const XK_Control_R:  u32 = 0xFFE4;
const XK_Alt_L:      u32 = 0xFFE9;
const XK_Alt_R:      u32 = 0xFFEA;
const XK_Meta_L:     u32 = 0xFFE7;
const XK_Meta_R:     u32 = 0xFFE8;
const XK_Super_L:    u32 = 0xFFEB;
const XK_Super_R:    u32 = 0xFFEC;

// ---- Virtual key bytes (MUST match client) ----
const VK_HOME:   u8 = 0xE0;
const VK_END:    u8 = 0xE1;
const VK_INSERT: u8 = 0xE2;
const VK_PGUP:   u8 = 0xE3;
const VK_PGDN:   u8 = 0xE4;
const VK_LEFT:   u8 = 0xE5;
const VK_RIGHT:  u8 = 0xE6;
const VK_UP:     u8 = 0xE7;
const VK_DOWN:   u8 = 0xE8;

/// Simple XTEST-based input injector.
pub struct Input {
    conn: RustConnection,
    root: Window,
    keysym_to_code: HashMap<u32, u8>,
    min_code: u8,
    max_code: u8,
    shift_code: Option<u8>,
    ctrl_code:  Option<u8>,
    alt_code:   Option<u8>,
    meta_code:  Option<u8>,
    /// Which modifiers we currently have pressed via XTEST
    mods_down: u16,
}

impl Input {
    pub fn new() -> Self {
        let (conn, screen_num) = x11rb::connect(None).expect("X11 connect failed");
        let setup = conn.setup();
        let screen = &setup.roots[screen_num];
        let root = screen.root;

        let _ = conn.xtest_get_version(2, 2).unwrap().reply().unwrap();

        let min_code = setup.min_keycode.into();
        let max_code = setup.max_keycode.into();
        debug!("input: server keycode range = [{min_code}, {max_code}]");

        let mapping = fetch_keyboard_mapping(&conn);
        let keysym_to_code = invert_keyboard_mapping(&mapping, min_code);

        let shift_code = pick_first(&keysym_to_code, &[XK_Shift_L, XK_Shift_R]);
        let ctrl_code  = pick_first(&keysym_to_code, &[XK_Control_L, XK_Control_R]);
        let alt_code   = pick_first(&keysym_to_code, &[XK_Alt_L, XK_Alt_R]);
        let meta_code  = pick_first(&keysym_to_code, &[XK_Meta_L, XK_Meta_R, XK_Super_L, XK_Super_R]);

        Self {
            conn, root, keysym_to_code, min_code, max_code,
            shift_code, ctrl_code, alt_code, meta_code,
            mods_down: 0,
        }
    }

    pub fn set_window(&mut self, _xid: i32) {}
    pub fn focus(&mut self) {}
    pub fn set_server_geometry(&mut self, _geom: crate::Geometry) {}

    /// Move pointer to absolute coordinates (already in your code)
    pub fn mouse_move(&mut self, x: i32, y: i32, _mods: u32) {
        let (x16, y16) = (x as i16, y as i16);
        self.conn
            .xtest_fake_input(MOTION_NOTIFY_EVENT, 0, 0, self.root, x16, y16, 0)
            .unwrap();
        self.conn.flush().unwrap();
    }

    /// Press a mouse button (1=Left, 2=Middle, 3=Right, 4=WheelUp, 5=WheelDown)
    pub fn mouse_press(&mut self, button: u8) {
        self.conn
            .xtest_fake_input(BUTTON_PRESS_EVENT, button, 0, self.root, 0, 0, 0)
            .unwrap();
        self.conn.flush().unwrap();
    }

    /// Release a mouse button
    pub fn mouse_release(&mut self, button: u8) {
        self.conn
            .xtest_fake_input(BUTTON_RELEASE_EVENT, button, 0, self.root, 0, 0, 0)
            .unwrap();
        self.conn.flush().unwrap();
    }

    /// Click (press + release) without moving the pointer
    pub fn mouse_click_button(&mut self, button: u8) {
        self.mouse_press(button);
        self.mouse_release(button);
    }

    /// Key down (protocol: base byte + modifiers bitmask).
    pub fn key_down(&mut self, key: u8, mods: u16) {
        let (ks, code, extra_shift) = self.resolve_from_byte(key);
        let mut desired = mods;
        if extra_shift { desired |= MOD_SHIFT; } // temporary shift needed for this key only

        debug!(
            "key_down: byte={} ks={:#06x?} code={:?} mods=0x{:x} desired=0x{:x}",
            key, ks.unwrap_or(0), code, mods, desired
        );

        self.sync_modifiers(desired);

        if let Some(code) = code {
            self.press_code(code);
            self.conn.flush().unwrap();
        }
    }

    /// Key up (release key first, then re-sync modifiers to the client state).
    pub fn key_up(&mut self, key: u8, mods: u16) {
        let (ks, code, _extra_shift) = self.resolve_from_byte(key);

        debug!(
            "key_up:   byte={} ks={:#06x?} code={:?} mods=0x{:x}",
            key, ks.unwrap_or(0), code, mods
        );

        if let Some(code) = code {
            self.release_code(code);
            self.sync_modifiers(mods); // drop any temporary shift we added for this key only
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

    /// Press/release modifiers so that our XTEST-held modifiers match `desired`.
    /// This avoids borrowing `self` immutably while mutating it by first taking
    /// a local snapshot of the (flag, keycode) pairs.
    fn sync_modifiers(&mut self, desired: u16) {
        // Snapshot of modifier mapping (copies only Option<u8> / u16)
        let mods_spec: [(u16, Option<u8>); 4] = [
            (MOD_CTRL,  self.ctrl_code),
            (MOD_ALT,   self.alt_code),
            (MOD_META,  self.meta_code),
            (MOD_SHIFT, self.shift_code),
        ];

        // Release any we shouldn't hold
        for (flag, code_opt) in mods_spec.iter().copied() {
            if (self.mods_down & flag) != 0 && (desired & flag) == 0 {
                if let Some(code) = code_opt { self.release_code(code); }
                self.mods_down &= !flag;
            }
        }
        // Press any we need but don't hold
        for (flag, code_opt) in mods_spec.iter().copied() {
            if (self.mods_down & flag) == 0 && (desired & flag) != 0 {
                if let Some(code) = code_opt { self.press_code(code); }
                self.mods_down |= flag;
            }
        }
    }

    /// Convert protocol byte into (keysym, keycode, extra_shift_needed).
    fn resolve_from_byte(&self, key: u8) -> (Option<u32>, Option<u8>, bool) {
        // Controls
        match key {
            8   => return (Some(XK_BackSpace), self.keysym_to_code.get(&XK_BackSpace).copied(), false),
            9   => return (Some(XK_Tab),       self.keysym_to_code.get(&XK_Tab).copied(),       false),
            10 | 13 =>
                return if let Some(c) = self.keysym_to_code.get(&XK_Return).copied() {
                    (Some(XK_Return), Some(c), false)
                } else if let Some(c) = self.keysym_to_code.get(&XK_KP_Enter).copied() {
                    (Some(XK_KP_Enter), Some(c), false)
                } else {
                    (Some(XK_Return), None, false)
                },
            27  => return (Some(XK_Escape),    self.keysym_to_code.get(&XK_Escape).copied(),    false),
            127 => return (Some(XK_Delete),    self.keysym_to_code.get(&XK_Delete).copied(),    false),
            _ => {}
        }

        // Navigation + arrows via virtual key bytes
        match key {
            VK_HOME  => return (Some(XK_Home),      self.keysym_to_code.get(&XK_Home).copied(),      false),
            VK_END   => return (Some(XK_End),       self.keysym_to_code.get(&XK_End).copied(),       false),
            VK_INSERT=> return (Some(XK_Insert),    self.keysym_to_code.get(&XK_Insert).copied(),    false),
            VK_PGUP  => return (Some(XK_Page_Up),   self.keysym_to_code.get(&XK_Page_Up).copied(),   false),
            VK_PGDN  => return (Some(XK_Page_Down), self.keysym_to_code.get(&XK_Page_Down).copied(), false),
            VK_LEFT  => return (Some(XK_Left),      self.keysym_to_code.get(&XK_Left).copied(),      false),
            VK_RIGHT => return (Some(XK_Right),     self.keysym_to_code.get(&XK_Right).copied(),     false),
            VK_UP    => return (Some(XK_Up),        self.keysym_to_code.get(&XK_Up).copied(),        false),
            VK_DOWN  => return (Some(XK_Down),      self.keysym_to_code.get(&XK_Down).copied(),      false),
            _ => {}
        }

        // Printable ASCII (32..=126): direct, or lowercase+shift for upper/shifted.
        if (32..=126).contains(&key) {
            let ks = key as u32;

            if let Some(&code) = self.keysym_to_code.get(&ks) {
                return (Some(ks), Some(code), false);
            }

            if key.is_ascii_uppercase() {
                let base = key.to_ascii_lowercase() as u32;
                if let Some(&code) = self.keysym_to_code.get(&base) {
                    return (Some(ks), Some(code), true);
                }
            }

            if let Some(base_char) = shifted_symbol_base(key) {
                let base_ks = base_char as u32;
                if let Some(&code) = self.keysym_to_code.get(&base_ks) {
                    return (Some(ks), Some(code), true);
                }
            }

            return (Some(ks), None, false);
        }

        // Optional raw-keycode path (disabled by default)
        if key >= self.min_code && key <= self.max_code {
            return (None, Some(key), false);
        }

        (None, None, false)
    }
}

/// Map shifted symbols (US layout) back to their unshifted base.
/// Example: '!' → '1', '_' → '-', '{' → '[', etc.
fn shifted_symbol_base(ch: u8) -> Option<u8> {
    match ch {
        b'!' => Some(b'1'), b'@' => Some(b'2'), b'#' => Some(b'3'), b'$' => Some(b'4'),
        b'%' => Some(b'5'), b'^' => Some(b'6'), b'&' => Some(b'7'), b'*' => Some(b'8'),
        b'(' => Some(b'9'), b')' => Some(b'0'), b'_' => Some(b'-'), b'+' => Some(b'='),
        b'{' => Some(b'['), b'}' => Some(b']'), b'|' => Some(b'\\'), b':' => Some(b';'),
        b'"' => Some(b'\''), b'<' => Some(b','), b'>' => Some(b'.'), b'?' => Some(b'/'),
        b'~' => Some(b'`'),
        _ => None,
    }
}

fn pick_first(map: &HashMap<u32, u8>, candidates: &[u32]) -> Option<u8> {
    for ks in candidates {
        if let Some(&c) = map.get(ks) { return Some(c); }
    }
    None
}

fn fetch_keyboard_mapping(conn: &RustConnection) -> GetKeyboardMappingReply {
    let setup = conn.setup();
    let min = setup.min_keycode;
    let max = setup.max_keycode;
    let keycode_count = u8::from(max).saturating_sub(u8::from(min)) + 1;
    conn.get_keyboard_mapping(min, keycode_count).unwrap().reply().unwrap()
}

fn invert_keyboard_mapping(map: &GetKeyboardMappingReply, min_code: u8) -> HashMap<u32, u8> {
    let mut h = HashMap::new();
    let keysyms_per_keycode = map.keysyms_per_keycode as usize;
    for (i, chunk) in map.keysyms.chunks(keysyms_per_keycode).enumerate() {
        let keycode = min_code.saturating_add(i as u8);
        for &ks in chunk {
            if ks != 0 && !h.contains_key(&ks) { h.insert(ks, keycode); }
        }
    }
    h
}
