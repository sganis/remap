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

        // Build a fresh mods bitmask each frame
        let mods = current_mods(&self.window);
        //debug!("mods=0x{:x}", mods);

        // Non-modifier keys with current modifiers
        for key in self.window.get_keys_pressed(minifb::KeyRepeat::No) {
            if is_modifier(key) { continue; }
            if let Some(byte) = map_key_to_byte(key) {
                debug!("key down: {:?} byte={} mods=0x{:x}", key, byte, mods);
                let _ = self.client_tx.send(ClientEvent::KeyEvent { down: true,  key: byte, mods });
            } else {
                debug!("key down: {:?} (unmapped)", key);
            }
        }
        for key in self.window.get_keys_released() {
            if is_modifier(key) { continue; }
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

fn is_modifier(key: Key) -> bool {
    use Key::*;
    matches!(key,
        LeftShift | RightShift | LeftCtrl | RightCtrl |
        LeftAlt   | RightAlt   | LeftSuper| RightSuper
    )
}

/// Map a non-modifier `minifb::Key` to a single ASCII-like byte (unshifted base).
/// Shift/Ctrl/Alt/Meta are carried separately via `mods`.
/// NOTE: Navigation (Home/End/Arrows/PageUp/Down), Insert, and F-keys cannot be
/// represented in this 1-byte scheme. To support them, evolve the protocol to
/// send a keysym (u32) instead of `u8`.
fn map_key_to_byte(key: Key) -> Option<u8> {
    use Key::*;
    match key {
        // letters -> lowercase ASCII base
        A=>Some(b'a'), B=>Some(b'b'), C=>Some(b'c'), D=>Some(b'd'), E=>Some(b'e'),
        F=>Some(b'f'), G=>Some(b'g'), H=>Some(b'h'), I=>Some(b'i'), J=>Some(b'j'),
        K=>Some(b'k'), L=>Some(b'l'), M=>Some(b'm'), N=>Some(b'n'), O=>Some(b'o'),
        P=>Some(b'p'), Q=>Some(b'q'), R=>Some(b'r'), S=>Some(b's'), T=>Some(b't'),
        U=>Some(b'u'), V=>Some(b'v'), W=>Some(b'w'), X=>Some(b'x'), Y=>Some(b'y'),
        Z=>Some(b'z'),

        // digits (unshifted)
        Key0=>Some(b'0'), Key1=>Some(b'1'), Key2=>Some(b'2'), Key3=>Some(b'3'),
        Key4=>Some(b'4'), Key5=>Some(b'5'), Key6=>Some(b'6'), Key7=>Some(b'7'),
        Key8=>Some(b'8'), Key9=>Some(b'9'),

        // punctuation (unshifted ASCII base)
        Minus        => Some(b'-'),
        Equal        => Some(b'='),
        LeftBracket  => Some(b'['),
        RightBracket => Some(b']'),
        Backslash    => Some(b'\\'),
        Semicolon    => Some(b';'),
        Apostrophe   => Some(b'\''),
        Comma        => Some(b','),
        Period       => Some(b'.'),
        Slash        => Some(b'/'),
        // If present in your minifb: GraveAccent => Some(b'`'),

        // controls / whitespace
        Enter     => Some(13),   // CR
        Tab       => Some(9),
        Escape    => Some(27),
        Space     => Some(32),
        Backspace => Some(8),
        Delete    => Some(127),

        // everything else (arrows, Home/End, etc.) is not representable
        _ => None,
    }
}
