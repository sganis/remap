use std::{ops, process};
use std::io::{Read, Write};
use std::process::Command;
use std::net::{TcpStream};
use std::sync::{Mutex};
use std::sync::mpsc::{Sender, Receiver};
use lazy_static::lazy_static;
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use minifb::{Key, MouseButton, MouseMode, ScaleMode, Window, WindowOptions};

//use vnc::{PixelFormat, Rect, VncConnector, VncEvent, X11Event};

use remap::{util, Result, Rect, ClientEvent, ServerEvent, Message};

//     main_window.connect_key_press_event(move |_, e| {
//         let name = e.keyval().name().unwrap().as_str().to_string();
//         let modifiers = e.state();
//         let key = *e.keyval();
//         println!("Key: {:?}, code: {}, state: {:?}, unicode: {:?}, name: {:?}, modifiers: {}", 
//             e.keyval(), 
//             *e.keyval(),
//             e.state(), 
//             e.keyval().to_unicode(), 
//             name, modifiers);

//         let mut app = &APP.lock().unwrap()[0];
//         let mut stream = &app.stream;
//         let message = ClientEvent::KeyEvent { down: true, key  };
//         message.write_to(&mut stream).unwrap(); 

//         if name == "F5" {
//             let width = 1684;
//             let height = 874;
//             let message = ClientEvent::FramebufferUpdateRequest { 
//                 incremental: false, x_position: 0, y_position: 0, width, height };
//             message.write_to(&mut stream).unwrap(); 
//             let reply = match ServerEvent::read_from(&mut stream) {
//                 Err(e) => {
//                     println!("Server disconnected: {:?}", e);
//                     return Inhibit(true);
//                 },
//                 Ok(o) => o,
//             };
//             match reply {
//                 ServerEvent::FramebufferUpdate { count } => {
//                     let nbytes = width as usize * height as usize * 4 as usize;
//                     let mut bytes = vec![0; nbytes as usize];
//                     stream.read_exact(&mut bytes).unwrap();
//                     println!("update reply");

//                     // let mut video = app.video;

//                     // for i in 0..bytes.len() {
//                     //     app.video[i] = bytes[i];
//                     // }
                    
                    


//                 },
//                 _ => {
//                     println!("other reply");
//                 }
//             }
//         }

//         Inhibit(true)
//     });

//     main_window.connect_key_release_event(move |_, e| {
//         let key = *e.keyval();
//         let message = ClientEvent::KeyEvent { down: false, key  };
//         let mut stream = &APP.lock().unwrap()[0].stream;
//         message.write_to(&mut stream).unwrap(); 
//         Inhibit(true)
//     });



struct App {
    window: Window,
    buffer: Vec<u32>,
    width: u32,
    height: u32,
    client_tx: Sender<ClientEvent>,
    server_rx: Receiver<ServerEvent>,
}

impl App {
    fn new(client_tx: Sender<ClientEvent>, server_rx: Receiver<ServerEvent>) -> Result<Self> {
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

    fn resize(&mut self, width: u32, height: u32) -> Result<(u32,u32)> {
        let mut window = Window::new(
            "Remap", width as usize, height as usize, 
            WindowOptions {
                resize: true,
                scale_mode: ScaleMode::UpperLeft,
                ..WindowOptions::default()
            })
            .expect("Unable to create window");
        window.limit_update_rate(Some(std::time::Duration::from_micros(33_000)));
        self.window = window;
        self.width = width;
        self.height = height;
        self.buffer.resize(height as usize * width as usize, 0);
        Ok((self.width, self.height))
    }
    fn is_open(&self) -> bool {
        self.window.is_open()
    }
    fn draw(&mut self, rect: Rect, data: Vec<u8>) -> Result<()> {
        // since we set the PixelFormat as bgra
        // the pixels must be sent in [blue, green, red, alpha] in the network order
        let mut s_idx = 0;
        for y in rect.y..rect.y + rect.height {
            let mut d_idx = y as usize * self.width as usize + rect.x as usize;
            for _ in rect.x..rect.x + rect.width {
                self.buffer[d_idx] =
                    u32::from_le_bytes(data[s_idx..s_idx + 4].try_into().unwrap()) & 0x00_ff_ff_ff;
                s_idx += 4;
                d_idx += 1;
            }
        }
        Ok(())
    }

    fn update(&mut self) -> Result<()> {
        self.window
            .update_with_buffer(&self.buffer, self.width as usize, self.height as usize)
            .expect("Unable to update screen buffer");
        Ok(())
    }

    fn copy(&mut self, dst: Rect, src: Rect) -> Result<()> {
        let mut tmp = vec![0; src.width as usize * src.height as usize];
        let mut tmp_idx = 0;
        for y in 0..src.height as usize {
            let mut s_idx = (src.y as usize + y) * self.width as usize + src.x as usize;
            for _ in 0..src.width {
                tmp[tmp_idx] = self.buffer[s_idx];
                tmp_idx += 1;
                s_idx += 1;
            }
        }
        tmp_idx = 0;
        for y in 0..src.height as usize {
            let mut d_idx = (dst.y as usize + y) * self.width as usize + dst.x as usize;
            for _ in 0..src.width {
                self.buffer[d_idx] = tmp[tmp_idx];
                tmp_idx += 1;
                d_idx += 1;
            }
        }
        Ok(())
    }

    fn close(&self) {}

    fn handle_input(&mut self) -> Result<()> {
        if let Some((x, y)) = self.window.get_mouse_pos(MouseMode::Discard) {            
            if self.window.get_mouse_down(MouseButton::Left) {
                println!("Mouse down left ({},{})", x,y);
            }
            if self.window.get_mouse_down(MouseButton::Right) {
                println!("Mouse down right ({},{})", x,y);
            }
            if let Some(scroll) = self.window.get_scroll_wheel() {
                println!("Scrolling {} - {}", scroll.0, scroll.1);
            }
            self.window.get_keys().iter().for_each(|key| match key {
                Key::W => println!("holding w!"),
                Key::T => println!("holding t!"),
                _ => (),
            });
            self.window.get_keys_released().iter().for_each(|key| match key {
                Key::W => println!("released w!"),
                Key::T => println!("released t!"),
                _ => (),
            });
        }
        Ok(())
    }

    fn handle_server_events(&mut self) -> Result<()> {
        if let Ok(reply) = self.server_rx.try_recv() {
             match reply {
                ServerEvent::FramebufferUpdate { count, bytes } => {
                    if bytes.len() > 0 {
                        let rect = Rect { 
                            x: 0, y: 0, 
                            width: self.width as u16, 
                            height: self.height as u16 
                        };
                        self.draw(rect, bytes)?;                        
                        //println!("updated");
                    } else {
                       // println!("not changed");
                    }        
                },
                m => println!("messge from server: {:?}", m)
            }
        } else {
            //println!("server busy");
        }
    
        
        Ok(())
    }

    fn request_update(&mut self) -> Result<()> {
        self.client_tx.send(ClientEvent::FramebufferUpdateRequest { 
            incremental:true, x_position: 0, y_position: 0, 
            width: self.width as u16, height: self.height as u16}
        ).unwrap();
        //println!("update request...");
        Ok(())
    }
}


pub fn main() -> Result<()> {
    
    let user = "san";
    //let host = "ecclin.chaintrust.com";
    //let host = "ecclap.chaintrust.com";
    let host = "192.168.100.202";
    let port: u16 = 10100;

    // make ssh connection
    let (tx,rx) = std::sync::mpsc::channel();

    // Spawn ssh tunnel thread
    std::thread::spawn(move|| {
        if util::port_is_listening(port) {
            println!("Tunnel exists, reusing...");            
            tx.send(()).expect("Could not send signal on channel.");
        } else {
            println!("Connecting...");
            let _handle = Command::new("ssh")
                .args(["-oStrictHostkeyChecking=no","-N","-L", 
                    &format!("{port}:127.0.0.1:{port}"),
                    &format!("{user}@{host}")])
                .spawn().unwrap();
            while !util::port_is_listening(port) {
                std::thread::sleep(std::time::Duration::from_millis(200));
            }
            tx.send(()).expect("Could not send signal on channel.");
        }
    });
    
    // wait for signal
    rx.recv().expect("Could not receive from channel.");
    println!("Tunnel Ok.");
    
    //  connection
    let mut stream = TcpStream::connect(&format!("127.0.0.1:{port}"))?;
    let stream2 = stream.try_clone()?;
    println!("Connected");
    let width = stream.read_u16::<BigEndian>()?;
    let height = stream.read_u16::<BigEndian>()?;
    println!("Geometry: {}x{}", width, height);

    let (server_tx, server_rx) = std::sync::mpsc::channel();
    let (client_tx, client_rx) = std::sync::mpsc::channel();

    std::thread::spawn(move || {        
        let mut stream = stream2;
        loop {
            let message: ClientEvent = client_rx.recv().unwrap();
            message.write_to(&mut stream).unwrap(); 
            let reply = match ServerEvent::read_from(&mut stream) {
                Err(e) => {
                    println!("Server disconnected: {:?}", e);
                    break;
                },
                Ok(o) => o,
            };   
            server_tx.send(reply).unwrap();        
        }
    });
 
    let mut app = App::new(client_tx, server_rx)?;
    app.resize(width as u32, height as u32)?;

    // loop at update rate
    while app.is_open() {
        app.handle_input()?;
        app.handle_server_events()?;
        app.update()?;
        app.request_update()?;
    }

    app.close();
    Ok(())

}
