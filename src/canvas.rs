use flume::{Receiver, Sender};
use anyhow::Result;
use log::debug;
use minifb::{MouseButton, MouseMode, ScaleMode, Window, WindowOptions, Key};
use crate::{Rec, ClientEvent, ServerEvent, MOD_SHIFT, MOD_CTRL, MOD_ALT, MOD_META};

// pointer bit masks
const BTN_LEFT:       u8 = 0x01;
const BTN_MIDDLE:     u8 = 0x02;
const BTN_RIGHT:      u8 = 0x04;
const BTN_WHEEL_UP:   u8 = 0x08;
const BTN_WHEEL_DOWN: u8 = 0x10;

// ---- Virtual key bytes for non-ASCII keys (shared idea with server) ----
const VK_HOME:      u8 = 0xE0;
const VK_END:       u8 = 0xE1;
const VK_INSERT:    u8 = 0xE2;
const VK_PGUP:      u8 = 0xE3;
const VK_PGDN:      u8 = 0xE4;
const VK_LEFT:      u8 = 0xE5;
const VK_RIGHT:     u8 = 0xE6;
const VK_UP:        u8 = 0xE7;
const VK_DOWN:      u8 = 0xE8;
// (Delete remains 0x7F == 127)

pub struct Canvas {
    window: Window,
    buffer: Vec<u32>,
    width: u32,
    height: u32,
    client_tx: Sender<ClientEvent>,
    client_rx: Receiver<ServerEvent>,
    buttons: u8,
    need_update: bool,
    last_mouse: Option<(u16,u16)>,
}

impl Canvas {
    pub fn new(client_tx: Sender<ClientEvent>, client_rx: Receiver<ServerEvent>) -> Result<Self> {
        Ok(Self {
            window: Window::new("Remap", 800, 600, WindowOptions::default())
                .expect("Unable to create window"),
            buffer: vec![0; 800 * 600],
            width: 800,
            height: 600,
            client_tx,
            client_rx,
            buttons: 0,
            need_update: false,
            last_mouse: None,
        })
    }

    pub fn resize(&mut self, width: u32, height: u32) -> Result<(u32,u32)> {
        let mut window = Window::new(
            "Remap",
            width as usize,
            height as usize,
            WindowOptions { resize: true, scale_mode: ScaleMode::AspectRatioStretch, ..WindowOptions::default() },
        ).expect("Unable to create window");
        window.set_target_fps(60);
        self.window = window;
        self.width = width;
        self.height = height;
        self.buffer.resize((width * height) as usize, 0);
        Ok((width, height))
    }

    pub fn is_open(&self) -> bool { self.window.is_open() }

    pub fn draw(&mut self, rec: &Rec) -> Result<()> {
        if self.buffer.is_empty() || rec.width == 0 || rec.height == 0 { return Ok(()); }
        let fb_w = self.width as i32; let fb_h = self.height as i32;
        let rx = rec.x as i32; let ry = rec.y as i32; let rw = rec.width as i32; let rh = rec.height as i32;

        let x0 = rx.max(0).min(fb_w); let y0 = ry.max(0).min(fb_h);
        let x1 = (rx + rw).max(0).min(fb_w); let y1 = (ry + rh).max(0).min(fb_h);
        let cw = (x1 - x0).max(0) as usize; let ch = (y1 - y0).max(0) as usize;
        if cw == 0 || ch == 0 { return Ok(()); }

        let src_stride = rec.width as usize * 4;
        let src_x_off = (x0 - rx).max(0) as usize;
        let src_y_off = (y0 - ry).max(0) as usize;
        let dst_stride = self.width as usize;

        for row in 0..ch {
            let src_row_start = (src_y_off + row) * src_stride + src_x_off * 4;
            let dst_row_start = (y0 as usize + row) * dst_stride + x0 as usize;

            let src = &rec.bytes[src_row_start .. src_row_start + cw * 4];
            let dst = &mut self.buffer[dst_row_start .. dst_row_start + cw];

            let mut s = 0;
            for px in dst.iter_mut() {
                let b = src[s] as u32; let g = src[s+1] as u32; let r = src[s+2] as u32;
                *px = (r) | (g << 8) | (b << 16);
                s += 4;
            }
        }
        Ok(())
    }

    pub fn update(&mut self) -> Result<()> {
        if self.need_update {
            self.window.update_with_buffer(&self.buffer, self.width as usize, self.height as usize)
                .expect("Unable to update screen buffer");
            self.need_update = false;
        } else {
            self.window.update();
        }
        Ok(())
    }

    pub fn handle_input(&mut self) -> Result<()> {
        if let Some((xf, yf)) = self.window.get_mouse_pos(MouseMode::Discard) {
            let (x, y) = (xf as u16, yf as u16);

            if self.last_mouse.map(|p| p != (x, y)).unwrap_or(true) {
                self.client_tx.send(ClientEvent::PointerEvent { buttons: self.buttons, x, y })?;
                self.last_mouse = Some((x, y));
            }

            // Buttons
            if self.window.get_mouse_down(MouseButton::Left) {
                if self.buttons & BTN_LEFT == 0 { self.buttons |= BTN_LEFT; self.client_tx.send(ClientEvent::PointerEvent { buttons: self.buttons, x, y })?; }
            } else if self.buttons & BTN_LEFT != 0 { self.buttons &= !BTN_LEFT; self.client_tx.send(ClientEvent::PointerEvent { buttons: self.buttons, x, y })?; }

            if self.window.get_mouse_down(MouseButton::Middle) {
                if self.buttons & BTN_MIDDLE == 0 { self.buttons |= BTN_MIDDLE; self.client_tx.send(ClientEvent::PointerEvent { buttons: self.buttons, x, y })?; }
            } else if self.buttons & BTN_MIDDLE != 0 { self.buttons &= !BTN_MIDDLE; self.client_tx.send(ClientEvent::PointerEvent { buttons: self.buttons, x, y })?; }

            if self.window.get_mouse_down(MouseButton::Right) {
                if self.buttons & BTN_RIGHT == 0 { self.buttons |= BTN_RIGHT; self.client_tx.send(ClientEvent::PointerEvent { buttons: self.buttons, x, y })?; }
            } else if self.buttons & BTN_RIGHT != 0 { self.buttons &= !BTN_RIGHT; self.client_tx.send(ClientEvent::PointerEvent { buttons: self.buttons, x, y })?; }

            // Scroll (emit multiple pulses for large wheel deltas)
            if let Some((_sx, sy)) = self.window.get_scroll_wheel() {
                let mut pulses = sy;
                while pulses >= 1.0 { self.client_tx.send(ClientEvent::PointerEvent { buttons: BTN_WHEEL_UP, x, y })?; pulses -= 1.0; }
                while pulses <= -1.0 { self.client_tx.send(ClientEvent::PointerEvent { buttons: BTN_WHEEL_DOWN, x, y })?; pulses += 1.0; }
                if sy != 0.0 {
                    self.client_tx.send(ClientEvent::PointerEvent { buttons: self.buttons, x, y })?;
                }
            }
        }

        // ---- Keyboard ----
        let mods = current_mods(&self.window);

        for key in self.window.get_keys_pressed(minifb::KeyRepeat::No) {
            if let Some(byte) = map_key_to_byte(key) {
                debug!("key down: {:?} byte={} mods=0x{:x}", key, byte, mods);
                let _ = self.client_tx.send(ClientEvent::KeyEvent { down: true,  key: byte, mods });
            }
        }
        for key in self.window.get_keys_released() {
            if let Some(byte) = map_key_to_byte(key) {
                let _ = self.client_tx.send(ClientEvent::KeyEvent { down: false, key: byte, mods });
            }
        }

        Ok(())
    }

    pub fn handle_server_events(&mut self) -> Result<()> {
        let mut any = false;
        while let Ok(reply) = self.client_rx.try_recv() {
            match reply {
                ServerEvent::FramebufferUpdate { count, rectangles } => {
                    if count > 0 {
                        for rec in &rectangles { self.draw(rec)?; }
                        any = true;
                    }
                }
                m => debug!("server event: {:?}", m),
            }
        }
        if any {
            self.need_update = true;
            self.request_update(true)?; // next incremental
        }
        Ok(())
    }

    pub fn request_update(&mut self, incremental: bool) -> Result<()> {
        self.client_tx.send(ClientEvent::FramebufferUpdateRequest {
            incremental, x: 0, y: 0, width: self.width as u16, height: self.height as u16
        })?;
        Ok(())
    }
}

// ---- helpers ----

fn current_mods(window: &Window) -> u16 {
    let mut m = 0;
    if window.is_key_down(Key::LeftShift)  || window.is_key_down(Key::RightShift)  { m |= MOD_SHIFT; }
    if window.is_key_down(Key::LeftCtrl)   || window.is_key_down(Key::RightCtrl)   { m |= MOD_CTRL;  }
    if window.is_key_down(Key::LeftAlt)    || window.is_key_down(Key::RightAlt)    { m |= MOD_ALT;   }
    if window.is_key_down(Key::LeftSuper)  || window.is_key_down(Key::RightSuper)  { m |= MOD_META;  }
    m
}

/// Map a minifb::Key to a single byte for the protocol.
/// Letters/digits/punctuation use their unshifted ASCII byte.
/// Navigation and arrows use VK_* in 0xE0..0xE8.
/// Modifiers are *not* sent as standalone events; they’re in the mods bitmask.
fn map_key_to_byte(key: Key) -> Option<u8> {
    use Key::*;
    Some(match key {
        // letters -> lowercase ASCII base
        A=>b'a', B=>b'b', C=>b'c', D=>b'd', E=>b'e', F=>b'f', G=>b'g', H=>b'h', I=>b'i',
        J=>b'j', K=>b'k', L=>b'l', M=>b'm', N=>b'n', O=>b'o', P=>b'p', Q=>b'q', R=>b'r',
        S=>b's', T=>b't', U=>b'u', V=>b'v', W=>b'w', X=>b'x', Y=>b'y', Z=>b'z',

        // digits (unshifted)
        Key0=>b'0', Key1=>b'1', Key2=>b'2', Key3=>b'3', Key4=>b'4',
        Key5=>b'5', Key6=>b'6', Key7=>b'7', Key8=>b'8', Key9=>b'9',

        // punctuation (unshifted ASCII base)
        Minus=>b'-', Equal=>b'=', LeftBracket=>b'[', RightBracket=>b']', Backslash=>b'\\',
        Semicolon=>b';', Apostrophe=>b'\'', Comma=>b',', Period=>b'.', Slash=>b'/',

        // controls / whitespace
        Enter=>13, Tab=>9, Escape=>27, Space=>32, Backspace=>8, Delete=>127,

        // navigation + arrows -> VK_* range
        Home=>VK_HOME, End=>VK_END, Insert=>VK_INSERT, PageUp=>VK_PGUP, PageDown=>VK_PGDN,
        Left=>VK_LEFT, Right=>VK_RIGHT, Up=>VK_UP, Down=>VK_DOWN,

        // ignore pure modifiers and unsupported keys (F-keys etc. not encoded in u8)
        LeftShift | RightShift | LeftCtrl | RightCtrl | LeftAlt | RightAlt | LeftSuper | RightSuper
        | F1 | F2 | F3 | F4 | F5 | F6 | F7 | F8 | F9 | F10 | F11 | F12
        => return None,

        _ => return None,
    })
}
