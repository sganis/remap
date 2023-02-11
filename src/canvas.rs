use std::sync::mpsc::{Sender, Receiver};
use minifb::{Key, MouseButton, MouseMode, ScaleMode, Window, WindowOptions};
use crate::{Result, Rec, ClientEvent, ServerEvent};

pub struct Canvas {
    window: Window,
    buffer: Vec<u32>,
    width: u32,
    height: u32,
    client_tx: Sender<ClientEvent>,
    server_rx: Receiver<ServerEvent>,
}

impl Canvas {
    pub fn new(client_tx: Sender<ClientEvent>, server_rx: Receiver<ServerEvent>) -> Result<Self> {
        Ok(Self {
            window: Window::new("Remap", 800_usize, 600_usize, WindowOptions::default())
                .expect("Unable to create window"),
            buffer: vec![],
            width: 800,
            height: 600,
            client_tx,
            server_rx,
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
        window.limit_update_rate(Some(std::time::Duration::from_micros(30_000)));
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
        // since we set the PixelFormat as bgra
        // the pixels must be sent in [blue, green, red, alpha] in the network order
        let mut s_idx = 0;
        for y in rec.y..rec.y + rec.height {
            let mut d_idx = y as usize * self.width as usize + rec.x as usize;
            for _ in rec.x..rec.x + rec.width {
                self.buffer[d_idx] =
                    u32::from_le_bytes(rec.bytes[s_idx..s_idx + 4].try_into().unwrap()) & 0x00_ff_ff_ff;
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

    // pub fn copy(&mut self, dst: Rec, src: Rec) -> Result<()> {
    //     let mut tmp = vec![0; src.width as usize * src.height as usize];
    //     let mut tmp_idx = 0;
    //     for y in 0..src.height as usize {
    //         let mut s_idx = (src.y as usize + y) * self.width as usize + src.x as usize;
    //         for _ in 0..src.width {
    //             tmp[tmp_idx] = self.buffer[s_idx];
    //             tmp_idx += 1;
    //             s_idx += 1;
    //         }
    //     }
    //     tmp_idx = 0;
    //     for y in 0..src.height as usize {
    //         let mut d_idx = (dst.y as usize + y) * self.width as usize + dst.x as usize;
    //         for _ in 0..src.width {
    //             self.buffer[d_idx] = tmp[tmp_idx];
    //             tmp_idx += 1;
    //             d_idx += 1;
    //         }
    //     }
    //     Ok(())
    // }

    pub fn close(&self) {}

    pub fn handle_input(&mut self) -> Result<()> {
        if let Some((x, y)) = self.window.get_mouse_pos(MouseMode::Discard) {            
            if self.window.get_mouse_down(MouseButton::Left) {
                println!("Mouse down left ({},{})", x,y);
                let event = ClientEvent::PointerEvent { 
                    button_mask: 1, x_position: x as u16, y_position: y as u16 };
                self.client_tx.send(event).unwrap(); 
            }
            if self.window.get_mouse_down(MouseButton::Right) {
                println!("Mouse down right ({},{})", x,y);
                let event = ClientEvent::PointerEvent { 
                    button_mask: 1, x_position: x as u16, y_position: y as u16 };
                self.client_tx.send(event).unwrap(); 
            }
            if let Some(scroll) = self.window.get_scroll_wheel() {
                println!("Scrolling {} - {}", scroll.0, scroll.1);
                let event = ClientEvent::PointerEvent { 
                    button_mask: 1, x_position: x as u16, y_position: y as u16 };
                self.client_tx.send(event).unwrap(); 
            }
        }
        self.window.get_keys_pressed(minifb::KeyRepeat::No).iter().for_each(|key| {
            println!("key down: {:?}", key);
            let event = ClientEvent::KeyEvent { down: true, key: *key as u8 };
            self.client_tx.send(event).unwrap();                        
        });
        self.window.get_keys_released().iter().for_each(|key| {
            println!("key up: {:?}", key);
            let event = ClientEvent::KeyEvent { down: false, key: *key as u8 };
            self.client_tx.send(event).unwrap();
        });
        
        Ok(())
    }

    pub fn handle_server_events(&mut self) -> Result<()> {
        if let Ok(reply) = self.server_rx.try_recv() {
             match reply {
                ServerEvent::FramebufferUpdate { count, rectangles } => {
                    println!("Rectangles recieved: {}", count);
                    if count > 0 {
                        for rec in rectangles.iter() {
                            self.draw(rec)?;    
                        }
                    }        
                },
                m => println!("messge from server: {:?}", m)
            }
        } else {
            //println!("server busy");
        }
        
        Ok(())
    }

    pub fn request_update(&mut self) -> Result<()> {
        let event = ClientEvent::FramebufferUpdateRequest { 
            incremental:true, x_position: 0, y_position: 0, 
            width: self.width as u16, height: self.height as u16
        };
        self.client_tx.send(event).unwrap();
        //println!("update request...");
        Ok(())
    }
}
