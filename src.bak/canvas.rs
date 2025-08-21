use flume::{Sender, Receiver};
use anyhow::Result;
use log::debug;
use minifb::{Key, MouseButton, MouseMode, ScaleMode, Window, WindowOptions};
use crate::{Rec, ClientEvent, ServerEvent};

// pointer bit masks
const BTN_LEFT:      u8 = 0x01;
const BTN_MIDDLE:    u8 = 0x02;
const BTN_RIGHT:     u8 = 0x04;
const BTN_WHEEL_UP:  u8 = 0x08;
const BTN_WHEEL_DOWN:u8 = 0x10;

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
            "Remap", width as usize, height as usize,
            WindowOptions {
                resize: true,
                scale_mode: ScaleMode::AspectRatioStretch,
                ..WindowOptions::default()
            })
            .expect("Unable to create window");
        window.limit_update_rate(Some(std::time::Duration::from_micros(16_667))); // ~60Hz
        self.window = window;
        self.width = width;
        self.height = height;
        self.buffer.resize((width * height) as usize, 0);
        Ok((width, height))
    }

    pub fn is_open(&self) -> bool {
        self.window.is_open()
    }

    pub fn draw(&mut self, rec: &Rec) -> Result<()> {
        let sw = self.width as usize;
        let rw = rec.width as usize;
        let rh = rec.height as usize;
        let mut s = 0;
        for row in 0..rh {
            let dst_row = (rec.y as usize + row) * sw + rec.x as usize;
            let dst = &mut self.buffer[dst_row .. dst_row + rw];
            for px in dst {
                let b = rec.bytes[s] as u32;
                let g = rec.bytes[s+1] as u32;
                let r = rec.bytes[s+2] as u32;
                *px = (r) | (g << 8) | (b << 16);
                s += 4;
            }
        }
        Ok(())
    }

    pub fn update(&mut self) -> Result<()> {
        if self.need_update {
            self.window
                .update_with_buffer(&self.buffer, self.width as usize, self.height as usize)
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

            if self.last_mouse.map(|p| p != (x,y)).unwrap_or(true) {
                self.client_tx.send(ClientEvent::PointerEvent { buttons: self.buttons, x, y })?;
                self.last_mouse = Some((x,y));
            }

            // Left
            if self.window.get_mouse_down(MouseButton::Left) {
                if self.buttons & BTN_LEFT == 0 {
                    self.buttons |= BTN_LEFT;
                    self.client_tx.send(ClientEvent::PointerEvent { buttons: self.buttons, x, y })?;
                }
            } else if self.buttons & BTN_LEFT != 0 {
                self.buttons &= !BTN_LEFT;
                self.client_tx.send(ClientEvent::PointerEvent { buttons: self.buttons, x, y })?;
            }

            // Middle
            if self.window.get_mouse_down(MouseButton::Middle) {
                if self.buttons & BTN_MIDDLE == 0 {
                    self.buttons |= BTN_MIDDLE;
                    self.client_tx.send(ClientEvent::PointerEvent { buttons: self.buttons, x, y })?;
                }
            } else if self.buttons & BTN_MIDDLE != 0 {
                self.buttons &= !BTN_MIDDLE;
                self.client_tx.send(ClientEvent::PointerEvent { buttons: self.buttons, x, y })?;
            }

            // Right
            if self.window.get_mouse_down(MouseButton::Right) {
                if self.buttons & BTN_RIGHT == 0 {
                    self.buttons |= BTN_RIGHT;
                    self.client_tx.send(ClientEvent::PointerEvent { buttons: self.buttons, x, y })?;
                }
            } else if self.buttons & BTN_RIGHT != 0 {
                self.buttons &= !BTN_RIGHT;
                self.client_tx.send(ClientEvent::PointerEvent { buttons: self.buttons, x, y })?;
            }

            // Scroll
            if let Some(scroll) = self.window.get_scroll_wheel() {
                let dy = scroll.1 as i32;
                let pulse = if dy > 0 { BTN_WHEEL_UP } else if dy < 0 { BTN_WHEEL_DOWN } else { 0 };
                if pulse != 0 {
                    self.client_tx.send(ClientEvent::PointerEvent { buttons: pulse, x, y })?;
                    self.client_tx.send(ClientEvent::PointerEvent { buttons: self.buttons, x, y })?;
                }
            }
        }

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
            self.request_update(true)?; // ask for next incremental
        }
        Ok(())
    }

    pub fn request_update(&mut self, incremental: bool) -> Result<()> {
        let event = ClientEvent::FramebufferUpdateRequest {
            incremental,
            x: 0, y: 0,
            width: self.width as u16,
            height: self.height as u16,
        };
        self.client_tx.send(event)?;
        Ok(())
    }
}
