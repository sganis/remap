use flume::{Receiver, Sender};
use anyhow::Result;
use log::debug;
use minifb::{MouseButton, MouseMode, ScaleMode, Window, WindowOptions};
use crate::{Rec, ClientEvent, ServerEvent};

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

        // Keys
        self.window.get_keys_pressed(minifb::KeyRepeat::No).iter().for_each(|key| {
            let _ = self.client_tx.send(ClientEvent::KeyEvent { down: true, key: *key as u8 });
        });
        self.window.get_keys_released().iter().for_each(|key| {
            let _ = self.client_tx.send(ClientEvent::KeyEvent { down: false, key: *key as u8 });
        });

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
