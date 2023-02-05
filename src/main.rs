use std::{ops, process};
use std::io::{Read, Write};
use std::process::Command;
use std::net::{TcpStream};
use gdk::{prelude::*, gdk_pixbuf};
use gtk::prelude::*;
use std::sync::{Mutex};
use lazy_static::lazy_static;
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use remap::{util, ClientEvent, ServerEvent, Message};

struct App {
    stream : TcpStream,
    width: u16,
    height : u16,
}

lazy_static! {
    static ref APP: Mutex<Vec<App>> = Mutex::new(Vec::new());
}

struct AppWindow {
    main_window: gtk::Window,
    timeout_id: Option<glib::SourceId>,
}

impl ops::Deref for AppWindow {
    type Target = gtk::Window;

    fn deref(&self) -> &gtk::Window {
        &self.main_window
    }
}

impl Drop for AppWindow {
    fn drop(&mut self) {
        if let Some(source_id) = self.timeout_id.take() {
            source_id.remove();
        }
    }
}

fn create_ui() -> AppWindow {

    let main_window = gtk::Window::new(gtk::WindowType::Toplevel);
    main_window.set_events(
        gdk::EventMask::PROPERTY_CHANGE_MASK
    );


    let video_window = gtk::DrawingArea::new();
    video_window.set_events(
        gdk::EventMask::BUTTON_PRESS_MASK |
        gdk::EventMask::BUTTON_RELEASE_MASK |
        gdk::EventMask::SCROLL_MASK |
        gdk::EventMask::PROPERTY_CHANGE_MASK |
        gdk::EventMask::POINTER_MOTION_MASK
    );

    video_window.connect_draw(move |_, context| {
        println!("window redraw called.");
        
        //context.set_source_pixbuf(&pixbuf, 0, 0);
        //context.paint();
        return Inhibit(false);
    });


    main_window.connect_key_press_event(move |_, e| {
        let name = e.keyval().name().unwrap().as_str().to_string();
        let modifiers = e.state();
        let key = *e.keyval();
        println!("Key: {:?}, code: {}, state: {:?}, unicode: {:?}, name: {:?}, modifiers: {}", 
            e.keyval(), 
            *e.keyval(),
            e.state(), 
            e.keyval().to_unicode(), 
            name, modifiers);

        let mut stream = &APP.lock().unwrap()[0].stream;
        let message = ClientEvent::KeyEvent { down: true, key  };
        message.write_to(&mut stream).unwrap(); 

        if name == "F5" {
            let width = 1684;
            let height = 874;
            let message = ClientEvent::FramebufferUpdateRequest { 
                incremental: false, x_position: 0, y_position: 0, width, height };
            message.write_to(&mut stream).unwrap(); 
            let reply = match ServerEvent::read_from(&mut stream) {
                Err(e) => {
                    println!("Server disconnected: {:?}", e);
                    return Inhibit(true);
                },
                Ok(o) => o,
            };
            match reply {
                ServerEvent::FramebufferUpdate { count } => {
                    let nbytes = width as usize * height as usize * 4 as usize;
                    let mut bytes = vec![0; nbytes as usize];
                    stream.read_exact(&mut bytes).unwrap();
                    println!("update reply");

                    // render image
                    image::save_buffer("image.jpg",
                        &bytes, width as u32, height as u32, image::ColorType::Rgba8).unwrap();
                    


                },
                _ => {
                    println!("other reply");
                }
            }
        }

        Inhibit(true)
    });

    main_window.connect_key_release_event(move |_, e| {
        let key = *e.keyval();
        let message = ClientEvent::KeyEvent { down: false, key  };
        let mut stream = &APP.lock().unwrap()[0].stream;
        message.write_to(&mut stream).unwrap(); 
        Inhibit(true)
    });

    video_window.connect_button_press_event(|_, e| {
        println!("{:?}, state: {:?}, button: {}", e.position(), e.state(), e.button());
       
        let buttons = match e.button() {
            1 => 128,
            2 => 64,
            3 => 32,
            _ => 0,
        };

        let message = ClientEvent::PointerEvent { 
            button_mask: buttons, 
            x_position: e.position().0 as u16,
            y_position: e.position().1 as u16, 
        };
        
        let mut stream = &APP.lock().unwrap()[0].stream;
        message.write_to(&mut stream).unwrap(); 
        
        Inhibit(true)
    });

    video_window.connect_button_release_event(|_, e| {
        println!("{:?}, state: {:?}, button: {}", e.position(), e.state(), e.button());
        let message = ClientEvent::PointerEvent { 
            button_mask: 0, 
            x_position: e.position().0 as u16,
            y_position: e.position().1 as u16, 
        };      
        let mut stream = &APP.lock().unwrap()[0].stream;
        message.write_to(&mut stream).unwrap(); 
        Inhibit(true)
    });

    video_window.connect_scroll_event(move |_, e| {
        println!("{:?}, state: {:?}, dir: {:?}", e.position(), e.state(), e.direction());
 
        // let b4: u8 = util::bits_to_number(&vec![0, 0, 0, 1, 0, 0, 0, 0]);
        // let b5: u8 = util::bits_to_number(&vec![0, 0, 0, 0, 1, 0, 0, 0]);
            
        let buttons = if e.direction() == gdk::ScrollDirection::Up { 16 } else { 8 };
        let message = ClientEvent::PointerEvent { 
            button_mask: buttons, 
            x_position: e.position().0 as u16,
            y_position: e.position().1 as u16, 
        };        
        
        let mut stream = &APP.lock().unwrap()[0].stream;
        message.write_to(&mut stream).unwrap(); 
        let message = ClientEvent::PointerEvent { 
            button_mask: 0, 
            x_position: e.position().0 as u16,
            y_position: e.position().1 as u16, 
        };        
        message.write_to(&mut stream).unwrap(); 

        Inhibit(true)
    });
    
    main_window.connect_delete_event(|_, _| {
        gtk::main_quit();
        Inhibit(false)
    });
    
    video_window.connect_motion_notify_event(|_, e| {
        
        Inhibit(true)
    });
    
    let timeout_id = glib::timeout_add_local(std::time::Duration::from_millis(500), || {

        Continue(true)
    });
    
    let vbox = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    vbox.pack_start(&video_window, true, true, 0);
    main_window.add(&vbox);
    let width = APP.lock().unwrap()[0].width;
    let height = APP.lock().unwrap()[0].height;
    main_window.set_default_size(width as i32, height as i32);
    main_window.show_all();

    AppWindow {
        main_window,
        timeout_id: Some(timeout_id),
    }
}

pub fn main() {
    
    let user = "san";
    //let host = "ecclin.chaintrust.com";
    //let host = "ecclap.chaintrust.com";
    let host = "192.168.100.202";
    let port: u16 = 10100;
    
    // video overlay does not work with this env var
    std::env::set_var("GTK_CSD","0");
    std::env::set_var("GDK_WIN32_LAYERED","0");
    
    // make ssh connection
    let (tx,rx) = std::sync::mpsc::channel();
    // read/write socket
    //let (stream_tx,stream_rx) = std::sync::mpsc::channel();

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
    println!("Connected");
    let width = stream.read_u16::<BigEndian>().unwrap();
    let height = stream.read_u16::<BigEndian>().unwrap();

    let app = App {
        stream,
        width,
        height,
    };
    APP.lock().unwrap().push(app);
    

    // spwan a thread to read socket
    // std::thread::spawn(move|| {
    //     loop {
    //         let mut stream = &TCP.lock().unwrap()[0];
    //         let reply = match ServerEvent::read_from(&mut stream) {
    //             Err(e) => {
    //                 println!("Server disconnected: {:?}", e);
    //                 break;
    //             },
    //             Ok(o) => o,            
    //         };
    //         println!("Reply from server");
    //         //stream_tx.send(reply).expect("Could not send signal on channel.");
    //     }
    // });

    // Initialize GTK
    if let Err(err) = gtk::init() {
        eprintln!("Failed to initialize GTK: {}", err);
        return;
    }

    let window = create_ui();
    //gdk::set_show_events(true);

    gtk::main();

    window.hide();
}
