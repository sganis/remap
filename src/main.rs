use std::{ops, process};
use std::io::{Read, Write};
use std::process::Command;
use std::net::{TcpStream};
use std::sync::{Mutex};
use lazy_static::lazy_static;
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use minifb::{Key, MouseButton, MouseMode, ScaleMode, Window, WindowOptions};

//use vnc::{PixelFormat, Rect, VncConnector, VncEvent, X11Event};

use remap::{util, Result, Rect, ClientEvent, ServerEvent, Message};

struct App {
    stream : TcpStream,
    video: Vec<u8>,
    width: u16,
    height : u16,
}

lazy_static! {
    static ref APP: Mutex<Vec<App>> = Mutex::new(Vec::new());
}

// struct AppWindow {
//     main_window: gtk::Window,
//     timeout_id: Option<glib::SourceId>,
// }

// impl ops::Deref for AppWindow {
//     type Target = gtk::Window;

//     fn deref(&self) -> &gtk::Window {
//         &self.main_window
//     }
// }

// impl Drop for AppWindow {
//     fn drop(&mut self) {
//         if let Some(source_id) = self.timeout_id.take() {
//             source_id.remove();
//         }
//     }
// }

// fn create_ui() -> AppWindow {

//     let main_window = gtk::Window::new(gtk::WindowType::Toplevel);
//     main_window.set_events(
//         gdk::EventMask::PROPERTY_CHANGE_MASK
//     );


//     let video_window = gtk::DrawingArea::new();
//     video_window.set_events(
//         gdk::EventMask::BUTTON_PRESS_MASK |
//         gdk::EventMask::BUTTON_RELEASE_MASK |
//         gdk::EventMask::SCROLL_MASK |
//         gdk::EventMask::PROPERTY_CHANGE_MASK |
//         gdk::EventMask::POINTER_MOTION_MASK
//     );

//     video_window.connect_draw(move |_, context| {
//         println!("window redraw called.");
        
//         // render image
//         let app = &APP.lock().unwrap()[0];
//         let bytes = &app.video;
//         let width = app.width;
//         let height = app.height;

//         if bytes.len() > 0 {
//             image::save_buffer("image.jpg",
//                 bytes, width as u32, height as u32, image::ColorType::Rgba8).unwrap();
//             println!("image saved.")
//             //context.set_source_pixbuf(&pixbuf, 0, 0);
//             //context.paint();
//         }

//         return Inhibit(false);
//     });


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

//     video_window.connect_button_press_event(|_, e| {
//         println!("{:?}, state: {:?}, button: {}", e.position(), e.state(), e.button());
       
//         let buttons = match e.button() {
//             1 => 128,
//             2 => 64,
//             3 => 32,
//             _ => 0,
//         };

//         let message = ClientEvent::PointerEvent { 
//             button_mask: buttons, 
//             x_position: e.position().0 as u16,
//             y_position: e.position().1 as u16, 
//         };
        
//         let mut stream = &APP.lock().unwrap()[0].stream;
//         message.write_to(&mut stream).unwrap(); 
        
//         Inhibit(true)
//     });

//     video_window.connect_button_release_event(|_, e| {
//         println!("{:?}, state: {:?}, button: {}", e.position(), e.state(), e.button());
//         let message = ClientEvent::PointerEvent { 
//             button_mask: 0, 
//             x_position: e.position().0 as u16,
//             y_position: e.position().1 as u16, 
//         };      
//         let mut stream = &APP.lock().unwrap()[0].stream;
//         message.write_to(&mut stream).unwrap(); 
//         Inhibit(true)
//     });

//     video_window.connect_scroll_event(move |_, e| {
//         println!("{:?}, state: {:?}, dir: {:?}", e.position(), e.state(), e.direction());
 
//         // let b4: u8 = util::bits_to_number(&vec![0, 0, 0, 1, 0, 0, 0, 0]);
//         // let b5: u8 = util::bits_to_number(&vec![0, 0, 0, 0, 1, 0, 0, 0]);
            
//         let buttons = if e.direction() == gdk::ScrollDirection::Up { 16 } else { 8 };
//         let message = ClientEvent::PointerEvent { 
//             button_mask: buttons, 
//             x_position: e.position().0 as u16,
//             y_position: e.position().1 as u16, 
//         };        
        
//         let mut stream = &APP.lock().unwrap()[0].stream;
//         message.write_to(&mut stream).unwrap(); 
//         let message = ClientEvent::PointerEvent { 
//             button_mask: 0, 
//             x_position: e.position().0 as u16,
//             y_position: e.position().1 as u16, 
//         };        
//         message.write_to(&mut stream).unwrap(); 

//         Inhibit(true)
//     });
    
//     main_window.connect_delete_event(|_, _| {
//         gtk::main_quit();
//         Inhibit(false)
//     });
    
//     video_window.connect_motion_notify_event(|_, e| {
        
//         Inhibit(true)
//     });
    
//     let timeout_id = glib::timeout_add_local(std::time::Duration::from_millis(500), || {

//         Continue(true)
//     });
    
//     let vbox = gtk::Box::new(gtk::Orientation::Horizontal, 0);
//     vbox.pack_start(&video_window, true, true, 0);
//     main_window.add(&vbox);
//     let width = APP.lock().unwrap()[0].width;
//     let height = APP.lock().unwrap()[0].height;
//     main_window.set_default_size(width as i32, height as i32);
//     main_window.show_all();

//     AppWindow {
//         main_window,
//         timeout_id: Some(timeout_id),
//     }
// }



struct CanvasUtils {
    window: Window,
    video: Vec<u32>,
    width: u32,
    height: u32,
}

impl CanvasUtils {
    fn new() -> Result<Self> {
        Ok(Self {
            window: Window::new(
                "mstsc-rs Remote Desktop in Rust",
                800_usize,
                600_usize,
                WindowOptions::default(),
            ).expect("Unable to create window"),
            video: vec![],
            width: 800,
            height: 600,
        })
    }

    fn init(&mut self, width: u32, height: u32) -> Result<()> {
        let mut window = Window::new(
            "mstsc-rs Remote Desktop in Rust",
            width as usize,
            height as usize,
            WindowOptions::default(),
        ).expect("Unable to create window");
        window.limit_update_rate(Some(std::time::Duration::from_micros(500_000)));
        self.window = window;
        self.width = width;
        self.height = height;
        self.video.resize(height as usize * width as usize, 0);
        Ok(())
    }

    fn draw(&mut self, rect: Rect, data: Vec<u8>) -> Result<()> {
        // since we set the PixelFormat as bgra
        // the pixels must be sent in [blue, green, red, alpha] in the network order

        let mut s_idx = 0;
        for y in rect.y..rect.y + rect.height {
            let mut d_idx = y as usize * self.width as usize + rect.x as usize;

            for _ in rect.x..rect.x + rect.width {
                self.video[d_idx] =
                    u32::from_le_bytes(data[s_idx..s_idx + 4].try_into().unwrap()) & 0x00_ff_ff_ff;
                s_idx += 4;
                d_idx += 1;
            }
        }
        Ok(())
    }

    fn flush(&mut self) -> Result<()> {
        self.window
            .update_with_buffer(&self.video, self.width as usize, self.height as usize)
            .expect("Unable to update screen buffer");
        Ok(())
    }

    fn copy(&mut self, dst: Rect, src: Rect) -> Result<()> {
        println!("Copy");
        let mut tmp = vec![0; src.width as usize * src.height as usize];
        let mut tmp_idx = 0;
        for y in 0..src.height as usize {
            let mut s_idx = (src.y as usize + y) * self.width as usize + src.x as usize;
            for _ in 0..src.width {
                tmp[tmp_idx] = self.video[s_idx];
                tmp_idx += 1;
                s_idx += 1;
            }
        }
        tmp_idx = 0;
        for y in 0..src.height as usize {
            let mut d_idx = (dst.y as usize + y) * self.width as usize + dst.x as usize;
            for _ in 0..src.width {
                self.video[d_idx] = tmp[tmp_idx];
                tmp_idx += 1;
                d_idx += 1;
            }
        }
        Ok(())
    }

    fn close(&self) {}

    fn hande_event(&mut self, event: ServerEvent) -> Result<()> {
        // match event {
        //     VncEvent::SetResolution(screen) => {
        //         tracing::info!("Resize {:?}", screen);
        //         self.init(screen.width as u32, screen.height as u32)?
        //     }
        //     VncEvent::RawImage(rect, data) => {
        //         self.draw(rect, data)?;
        //     }
        //     VncEvent::Bell => {
        //         tracing::warn!("Bell event got, but ignore it");
        //     }
        //     VncEvent::SetPixelFormat(_) => unreachable!(),
        //     VncEvent::Copy(dst, src) => {
        //         self.copy(dst, src)?;
        //     }
        //     VncEvent::JpegImage(_rect, _data) => {
        //         tracing::warn!("Jpeg event got, but ignore it");
        //     }
        //     VncEvent::SetCursor(rect, data) => {
        //         if rect.width != 0 {
        //             self.draw(rect, data)?;
        //         }
        //     }
        //     VncEvent::Text(string) => {
        //         tracing::info!("Got clipboard message {}", string);
        //     }
        //     _ => unreachable!(),
        // }
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
    let mut stream = TcpStream::connect(&format!("127.0.0.1:{port}"))
        .expect("Cannot connect to input port");
    let stream2 = stream.try_clone()
        .expect("Cannot connect to input port");
    println!("Connected");
    let width = stream.read_u16::<BigEndian>().unwrap();
    let height = stream.read_u16::<BigEndian>().unwrap();
    //let size = width as usize * height as usize;


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
            // match reply {
            //     ServerEvent::FramebufferUpdate { count, bytes } => {
            //         // let nbytes = width as usize * height as usize * 4 as usize;
            //         // let mut bytes = vec![0; nbytes as usize];
            //         // stream.read_exact(&mut bytes).unwrap();
            //         println!("update reply");
            //         server_tx.send(reply);
            //         println!("there 1");
            //     },
            //     e => {
            //         println!("event: {:?}", e);
            //         server_tx.send(ServerEvent::Bell);
            //         println!("there 2");
            //     }
            // }
        }
    });



    //let mut canvas = CanvasUtils::new()?;

    client_tx.send(ClientEvent::FramebufferUpdateRequest { 
        incremental: true, 
        x_position: 0, y_position: 0, width, height 
    }).unwrap();
   
    let width = width as usize;
    let height = height as usize;
    let mut buffer: Vec<u32> = vec![0; width * height];

    let mut window = Window::new(
        "Remap",
        width,
        height,
        WindowOptions {
            resize: true,
            scale_mode: ScaleMode::UpperLeft,
            ..WindowOptions::default()
        },
    ).unwrap();
    window.limit_update_rate(Some(std::time::Duration::from_micros(30_000)));
        

    let mut size = (width, height);

    // loop at update rate
    while window.is_open() {

        let new_size = (window.get_size().0, window.get_size().1);
        if new_size != size {
            size = new_size;
            buffer.resize(size.0 * size.1, 0);
        }
        
        if let Some((x, y)) = window.get_mouse_pos(MouseMode::Discard) {            
            if window.get_mouse_down(MouseButton::Left) {
                println!("Mouse down left ({},{})", x,y);
            }
            if window.get_mouse_down(MouseButton::Right) {
                println!("Mouse down right ({},{})", x,y);
            }
        }

        if let Some(scroll) = window.get_scroll_wheel() {
            println!("Scrolling {} - {}", scroll.0, scroll.1);
        }
        window.get_keys().iter().for_each(|key| match key {
            Key::W => println!("holding w!"),
            Key::T => println!("holding t!"),
            _ => (),
        });

        window.get_keys_released().iter().for_each(|key| match key {
            Key::W => println!("released w!"),
            Key::T => println!("released t!"),
            _ => (),
        });
        // if let Ok(reply) = server_rx.try_recv() {
        //     match reply {
        match server_rx.recv().unwrap() {
            ServerEvent::FramebufferUpdate { count, bytes } => {
                let mut s_idx = 0;
                for y in 0..height {
                    let mut d_idx = y as usize * width as usize;
                    for _ in 0..width {
                        buffer[d_idx] = u32::from_le_bytes(
                            bytes[s_idx..s_idx + 4].try_into().unwrap()) & 0x00_ff_ff_ff;
                        s_idx += 4;
                        d_idx += 1;
                    }
                }
                println!("updated");        
            },
            _ => ()
        }
    

        // We unwrap here as we want this code to exit if it fails
        window.update_with_buffer(&buffer, new_size.0, new_size.1).unwrap();
        
        client_tx.send(ClientEvent::FramebufferUpdateRequest { 
            incremental:true, x_position: 0, y_position: 0, 
            width: new_size.0 as u16, height: new_size.1 as u16}).unwrap();
        println!("update request...");
    }




    // while let Ok(event) = server_rx.recv() {
    //     println!("here");
    //     canvas.hande_event(event)?;
    //     while let Ok(e) = server_rx.try_recv() {
    //         canvas.hande_event(e)?;
    //         println!("here 2");
    //     }
    //     //canvas.flush()?;
    //     client_tx.send(ClientEvent::FramebufferUpdateRequest { 
    //         incremental: true, 
    //         x_position: 0, y_position: 0, width, height 
    //     }).unwrap();
    //     println!("here 3");
    // }
    
    
    //canvas.close();
    Ok(())

}
