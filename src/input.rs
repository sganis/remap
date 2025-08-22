#![cfg(target_os = "linux")]

use std::collections::HashMap;

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

/// Simple XTEST-based input injector (no enigo).
pub struct Input {
    conn: RustConnection,
    root: Window,
    /// map Keysym (u32) -> keycode (u8)
    keysym_to_code: HashMap<u32, u8>,
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

        // Build a keysym -> keycode map from the server's keyboard mapping
        let mapping = fetch_keyboard_mapping(&conn);
        let keysym_to_code = invert_keyboard_mapping(&mapping);

        Self { conn, root, keysym_to_code }
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
        if let Some(code) = self.lookup_keycode(key) {
            self.conn
                .xtest_fake_input(KEY_PRESS_EVENT, code, 0, self.root, 0, 0, 0)
                .unwrap();
            self.conn.flush().unwrap();
        }
    }

    /// Key up.
    pub fn key_up(&mut self, key: u8) {
        if let Some(code) = self.lookup_keycode(key) {
            self.conn
                .xtest_fake_input(KEY_RELEASE_EVENT, code, 0, self.root, 0, 0, 0)
                .unwrap();
            self.conn.flush().unwrap();
        }
    }

    /// Map your client’s key byte (ASCII-like) to a keycode via keysym.
    fn lookup_keycode(&self, key: u8) -> Option<u8> {
        let ks = ascii_like_to_keysym(key)?;
        self.keysym_to_code.get(&ks).copied()
    }
}

/// Convert a few common ASCII-ish codes to X11 KeySym values.
/// Extend as needed to cover F-keys, arrows, modifiers, etc.
fn ascii_like_to_keysym(key: u8) -> Option<u32> {
    match key {
        b'0'..=b'9' => Some(key as u32),
        b'a'..=b'z' => Some(key as u32),
        b'A'..=b'Z' => Some(key as u32),
        27 => Some(0xff1b), // Escape
        9  => Some(0xff09), // Tab
        13 => Some(0xff0d), // Return
        32 => Some(0x0020), // Space
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
fn invert_keyboard_mapping(map: &GetKeyboardMappingReply) -> HashMap<u32, u8> {
    let mut h = HashMap::new();
    let keysyms_per_keycode = map.keysyms_per_keycode as usize;
    for (i, chunk) in map.keysyms.chunks(keysyms_per_keycode).enumerate() {
        let keycode = i as u8 + 8; // X11 keycodes typically start at 8
        for &ks in chunk {
            if ks != 0 && !h.contains_key(&ks) {
                h.insert(ks, keycode);
            }
        }
    }
    h
}
