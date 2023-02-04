use std::{ops, os::raw::c_void, process};
use std::io::{Read, Write};
use std::process::Command;
use std::net::{TcpStream};
use gdk::{prelude::*, gdk_pixbuf, ModifierType};
use gtk::prelude::*;

use std::sync::{Mutex};
use lazy_static::lazy_static;
use remap::{Event, EventAction, Modifier, util, ClientEvent, Message};


lazy_static! {
    static ref TCP: Mutex<Vec<TcpStream>> = Mutex::new(Vec::new());
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
        //let modifiers = e.state().bits();     
        // let a = ModifierType::CONTROL_MASK;
        // let m = e.state().contains(a);
        // println!(" contains control: {}", m);

        let mut stream = &TCP.lock().unwrap()[0];
        let message = ClientEvent::KeyEvent { down: true, key  };
        message.write_to(&mut stream).unwrap(); 

            // let mut data = [0; 2]; // using 2 byte buffer
            // match stream.read(&mut data) {
            //     Ok(_) => {
            //         let c = String::from_utf8_lossy(&data[..]);
            //         println!("Response: {}", c);                            
            //     },
            //     Err(e) => {
            //         println!("Failed to receive data: {}", e);
            //     }
            // }

            // send update request

            // let nbytes: usize = width * height * 4;
            // let mut event = Event {
            //     action: EventAction::FramebufferUpdateRequest {
            //         incremental: false, x: 0, y: 0, 
            //         width: width as u16, 
            //         height: height as u16 },
            //     modifiers,
            // };
            // stream.write(&event.as_bytes()).unwrap();
            // let mut data = vec![0; nbytes as usize];
            // match stream.read(&mut data) {
            //     Ok(_) => {

            //         // image::save_buffer("image.jpg",
            //         //     &data, width as u32, height as u32, 
            //         //     image::ColorType::Rgba8).unwrap();
            //         println!("image saved");                            
            //     },
            //     Err(e) => {
            //         println!("Failed to receive image: {}", e);
            //     }
            // }


        // } else if name == "BackSpace" || name == "Delete" ||
        //         name == "Page_Down" || name == "Page_Up" ||
        //         name == "Up" || name == "Down" ||
        //         name == "Left" || name == "Right" ||
        //         name == "Home" || name == "End" ||
        //         name == "Tab" || name == "Escape" {
        //     let mut event = Event {
        //         action: EventAction::KeyPress { key: name },
        //         modifiers,
        //     };
        //     stream.write(&event.as_bytes()).unwrap();
        //     //stream.flush().unwrap();
        // } else {
        //     match e.keyval().to_unicode() {
        //         Some(k) => {
        //             let mut event = Event {
        //                 action: EventAction::KeyPress { key: k.to_string() },
        //                 modifiers,
        //             };
        //             stream.write(&event.as_bytes()).unwrap();
        //             //stream.flush().unwrap();
        //             println!("key sent: {k}");
                            
        //         },
        //         None => {
        //             println!("key not supported: {name}");
        //         }
        //     }                   
        // }                
        Inhibit(true)
    });

    main_window.connect_key_release_event(move |_, e| {
        let key = *e.keyval();
        let message = ClientEvent::KeyEvent { down: false, key  };
        let mut stream = &TCP.lock().unwrap()[0];
        message.write_to(&mut stream).unwrap(); 
        Inhibit(true)
    });

    video_window.connect_button_press_event(|_, e| {
        //println!("{:?}", e);    
        println!("{:?}, state: {:?}, button: {}", e.position(), e.state(), e.button());
        let button = e.button();
        let b1: u8 = (button == 1).into();
        let b2: u8 = (button == 2).into();
        let b3: u8 = (button == 3).into();
        let bits: Vec<u8> = vec![b1, b2, b3, 0, 0, 0, 0, 0];
        let buttons: u8 = util::bits_to_number(&bits);
        println!("bits: {:?}, number: {}", bits, buttons);
        
        let message = ClientEvent::PointerEvent { 
            button_mask: buttons, 
            x_position: e.position().0 as u16,
            y_position: e.position().1 as u16, 
        };
        
        let mut stream = &TCP.lock().unwrap()[0];
        message.write_to(&mut stream).unwrap(); 
        
        // let mut event = Event {
        //     action: EventAction::Click {
        //         x: e.position().0 as i32,
        //         y: e.position().1 as i32,
        //         button,
        //     },
        //     modifiers,
        // };
        
        // stream.write(&event.as_bytes()).expect("Could not send mouse event");

        // if button == 1 {
        //     let mut data = [0; 2]; 
        //     match stream.read(&mut data) {
        //         Ok(_) => {
        //             let c = String::from_utf8_lossy(&data[..]);
        //             println!("Response: {}", c);                            
        //         },
        //         Err(e) => {
        //             println!("Failed to receive data: {}", e);
        //         }
        //     }
        // }
        Inhibit(true)
    });

    video_window.connect_button_release_event(|_, e| {
        println!("{:?}, state: {:?}, button: {}", e.position(), e.state(), e.button());
        let message = ClientEvent::PointerEvent { 
            button_mask: 0, 
            x_position: e.position().0 as u16,
            y_position: e.position().1 as u16, 
        };      
        let mut stream = &TCP.lock().unwrap()[0];
        message.write_to(&mut stream).unwrap(); 
        Inhibit(true)
    });

    video_window.connect_scroll_event(move |_, e| {
        println!("{:?}", e);    
        println!("{:?}, state: {:?}, dir: {:?}", e.position(), e.state(), e.direction());
        Inhibit(true)
    });
    
    main_window.connect_delete_event(|_, _| {
        gtk::main_quit();
        Inhibit(false)
    });
    
    // video_window.connect_resize(|_, width, height| {
    //     let mut stream = &TCP.lock().unwrap()[0];
    //     let mut event = Event {
    //         action: EventAction::Resize {width, height}, 
    //         modifiers: 0
    //     };   
    //     stream.write(&event.as_bytes()).expect("Could not send resize event");
    //     stream.flush().unwrap();
    //     let mut data = [0; 2]; 
    //     stream.read(&mut data).expect("Failed to recieved mouse move");
    // });

    video_window.connect_motion_notify_event(|_, e| {
        return Inhibit(true);
        //println!("{:?}, state: {:?}", e.position(), e.state());
        let modifiers = e.state().bits();
        let mut stream = &TCP.lock().unwrap()[0];
        let mut event = Event {
            action: EventAction::MouseMove {
                x: e.position().0 as i32,
                y: e.position().1 as i32,
            },
            modifiers,
        };        
        stream.write(&event.as_bytes()).expect("Could not send mouse move event");
        stream.flush().unwrap();
        let mut data = [0; 2]; 
        stream.read(&mut data).expect("Failed to recieved mouse move");
        Inhibit(true)
    });
    
    
    let vbox = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    vbox.pack_start(&video_window, true, true, 0);
    main_window.add(&vbox);
    main_window.set_default_size(1684, 874);
    main_window.show_all();

    AppWindow {
        main_window,
        timeout_id: None // Some(timeout_id),
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
    rx.recv()
        .expect("Could not receive from channel.");
    println!("Tunnel Ok.");
    
    //  connection
    let stream = TcpStream::connect(&format!("127.0.0.1:{port}"))
        .expect("Cannot connect to input port");
    println!("Event connection Ok.");
    {
        let mut guard = TCP.lock().unwrap();
        guard.push(stream);
    }

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
