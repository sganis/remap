use flume::{Sender, Receiver};
use anyhow::Result;
use log::{debug};
use minifb::{Key, MouseButton, MouseMode, ScaleMode, Window, WindowOptions};
use crate::{Rec, ClientEvent, ServerEvent};

pub struct Canvas {
    window: Window,
    buffer: Vec<u32>,
    width: u32,
    height: u32,
    client_tx: Sender<ClientEvent>,
    client_rx: Receiver<ServerEvent>,
    buttons: u8,
    need_update: bool,
}

// enum Pointer {
//     Left = 0x01,
//     Middle = 0x02,
//     Right = 0x04,
//     WheelUp = 0x08,
//     WheelDown = 0x16, 
// }

impl Canvas {
    pub fn new(client_tx: Sender<ClientEvent>, client_rx: Receiver<ServerEvent>) -> Result<Self> {
        Ok(Self {
            window: Window::new("Remap", 800_usize, 600_usize, WindowOptions::default())
                .expect("Unable to create window"),
            buffer: vec![],
            width: 800,
            height: 600,
            client_tx,
            client_rx,
            buttons: 0u8,
            need_update: false,
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
        window.limit_update_rate(Some(std::time::Duration::from_micros(17_000)));
        self.window = window;
        self.width = width;
        self.height = height;
        self.buffer.resize(height as usize * width as usize, 0);
        Ok((self.width, self.height))
    }
    pub fn is_open(&self) -> bool {
        self.window.is_open()
    }
    pub fn draw(&mut self, rec: &Rec) -> Result<()> {
        let mut s_idx = 0;
        for y in rec.y..rec.y + rec.height {
            let mut d_idx = y as usize * self.width as usize + rec.x as usize;
            for _ in rec.x..rec.x + rec.width {
                self.buffer[d_idx] =
                    u32::from_le_bytes(rec.bytes[s_idx..s_idx + 4].try_into()?) & 0x00_ff_ff_ff;
                s_idx += 4;
                d_idx += 1;
            }
        }
        Ok(())
    }

    pub fn update(&mut self) -> Result<()> {
        self.window
            .update_with_buffer(&self.buffer, self.width as usize, self.height as usize)
            .expect("Unable to update screen buffer");
        Ok(())
    }

    pub fn close(&self) {}

    pub fn handle_input(&mut self) -> Result<()> {
        if let Some((x, y)) = self.window.get_mouse_pos(MouseMode::Discard) {            
            let mut event = ClientEvent::PointerEvent { buttons: 0, x: 0, y: 0 };
            if self.window.get_mouse_down(MouseButton::Left) {
                if self.buttons & 0x01 != 0x01 {
                    self.buttons |= 0x01;
                    debug!("Mouse left down ({},{})", x,y);
                    let event = ClientEvent::PointerEvent { 
                        buttons: self.buttons, x: x as u16, y: y as u16 };                
                    self.client_tx.send(event)?; 
                }
            } else {
                if self.buttons & 0x01 == 0x01 {
                    debug!("Mouse left up ({},{})", x,y);
                    self.buttons &= !0x01;
                    let event = ClientEvent::PointerEvent { 
                        buttons: self.buttons, x: x as u16, y: y as u16 };                
                    self.client_tx.send(event)?; 
                }
            }
            if self.window.get_mouse_down(MouseButton::Middle) {
                if self.buttons & 0x02 != 0x02 {
                    self.buttons |= 0x02;
                    debug!("Mouse middle down ({},{})", x,y);
                    let event = ClientEvent::PointerEvent { 
                        buttons: self.buttons, x: x as u16, y: y as u16 };                
                    self.client_tx.send(event)?; 
                }
            } else {
                if self.buttons & 0x02 == 0x02 {
                    debug!("Mouse middle up ({},{})", x,y);
                    self.buttons &= !0x02;
                    debug!("buttons: {}", self.buttons);
                    let event = ClientEvent::PointerEvent { 
                        buttons: self.buttons, x: x as u16, y: y as u16 };                
                    self.client_tx.send(event)?; 
                }
            }
            if self.window.get_mouse_down(MouseButton::Right) {
                if self.buttons & 0x04 != 0x04 {
                    self.buttons |= 0x04;
                    debug!("Mouse right down ({},{})", x,y);
                    let event = ClientEvent::PointerEvent { 
                        buttons: self.buttons, x: x as u16, y: y as u16 };                
                    self.client_tx.send(event)?; 
                }
            } else {
                if self.buttons & 0x04 == 0x04 {
                    debug!("Mouse right up ({},{})", x,y);
                    self.buttons &= !0x04;
                    let event = ClientEvent::PointerEvent { 
                        buttons: self.buttons, x: x as u16, y: y as u16 };                
                    self.client_tx.send(event)?; 
                }
            }
            if let Some(scroll) = self.window.get_scroll_wheel() {
                let y_direction = scroll.1 as i32;
                if y_direction > 0 {
                    debug!("Scrolling up");
                    let event = ClientEvent::PointerEvent { 
                        buttons: 0x08, x: x as u16, y: y as u16 };
                    self.client_tx.send(event)?; 
                } else {
                    debug!("Scrolling down");
                    let event = ClientEvent::PointerEvent { 
                        buttons: 0x16, x: x as u16, y: y as u16 };
                    self.client_tx.send(event)?; 
                }
                let event = ClientEvent::PointerEvent { 
                    buttons: 0, x: x as u16, y: y as u16 };
                self.client_tx.send(event)?; 
            }
        }
        self.window.get_keys_pressed(minifb::KeyRepeat::No)
            .iter().for_each(|key| {
            debug!("key down: {:?}", key);
            let event = ClientEvent::KeyEvent { down: true, key: *key as u8 };
            self.client_tx.send(event).unwrap();                        
        });
        self.window.get_keys_released().iter().for_each(|key| {
            debug!("key up: {:?}", key);
            let event = ClientEvent::KeyEvent { down: false, key: *key as u8 };
            self.client_tx.send(event).unwrap();
        });
        
        Ok(())
    }

    pub fn handle_server_events(&mut self) -> Result<()> {
        if let Ok(reply) = self.client_rx.try_recv() {
             match reply {
                ServerEvent::FramebufferUpdate { count, rectangles } => {
                    debug!("Rectangles recieved: {}", count);
                    if count > 0 {
                        for rec in rectangles.iter() {
                            self.draw(rec)?;    
                        }
                        self.need_update = true;
                    }        
                },
                m => debug!("messge from server: {:?}", m)
            }
        } else {
            //debug!("server busy");
        }
        
        Ok(())
    }

    pub fn request_update(&mut self) -> Result<()> {
        let event = ClientEvent::FramebufferUpdateRequest { 
            incremental:true, x: 0, y: 0, 
            width: self.width as u16, height: self.height as u16
        };
        if let Err(e) = self.client_tx.send(event) {
            anyhow::bail!("exiting after server disonnection")
        };
        Ok(())
    }
}
