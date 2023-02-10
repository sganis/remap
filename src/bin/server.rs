#![allow(unused)]
use std::error::Error;
use std::io::{Read,Write};
use std::net::TcpListener;
use std::process::{Command, Stdio};
use std::str::FromStr;
use std::cell::RefCell;
use std::rc::Rc;
use std::{time::Instant};
use xcb::x::{Window, Drawable, GetImage, ImageFormat, GetGeometry};
use xcb::{XidNew};
use clap::Parser;
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use remap::{util, Rec, Geometry, ClientEvent, ServerEvent, Message};
use remap::capture::Capture;
use remap::input::Input;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// The display to use (default: :100)
    #[arg(short, long)]
    display: Option<u32>,

    /// The app to run (default: xterm)
    #[arg(short, long)]
    app: Option<String>,

    /// The port to use (default: 10100)
    #[arg(short, long)]
    port: Option<u16>,

    /// Verbosity level
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,
}

fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();
    let display = cli.display.unwrap_or(100);
    let app = cli.app.unwrap_or(
        String::from("xterm -fa 'Monospace' -fs 14 -geometry 110x24"));
    let args: Vec<&str> = app.split_whitespace().collect();
    let app = args[0];
    let args = &args[1..];       
    let desktop = app == "desktop";
    let port = cli.port.unwrap_or(10100);
    let input_addr = format!("127.0.0.1:{port}");
    let mut display_proc = None;
    let mut app_proc = None;
    let mut xid = 0;
    let mut geometry = Geometry::default();

    println!("Display: :{}", display);
    println!("App: {}", app);
    println!("Args: {:?}", args);
    println!("Port: {}", port);
    println!("Verbosity: {}", cli.verbose);

    if !desktop {
        std::env::set_var("DISPLAY",&format!(":{display}"));
    }

    if !desktop {
        // run display_server
        // default resolution: 1280x1024x8
        let p = Command::new("Xvfb")
            .args(["+extension","GLX","+extension","Composite",
                "-screen","0","2048x1024x24+32",
                //"-auth","/run/user/1000/gdm/Xauthority",
                "-nolisten","tcp","-noreset", "-dpi","96",
                &format!(":{display}")])
            .spawn()
            .expect("display failed to start");
        println!("display pid: {}", p.id());
        display_proc = Some(p);

        // wait for it
        while !util::is_display_server_running(display) {
            println!("Waiging display...");
            std::thread::sleep(std::time::Duration::from_millis(200));
        }    
        
        // run app and get pid
        let p = Command::new(&app)
            .args(&*args)
            .spawn()
            .expect("Could not run app");
        let pid = p.id();
        app_proc = Some(p);
        println!("app pid: {pid}");
        
        // find window ID,. wait for it
        //let window_name = app;
        //xid = util::get_window_id(pid, window_name, display);   
        xid = util::get_window_id(pid, display);   
        while xid == 0 {
            println!("Waiting window id...");
            std::thread::sleep(std::time::Duration::from_millis(200));
            xid = util::get_window_id(pid, display);
        } 
        println!("Window xid: {} ({:#06x})", xid, xid);

        geometry = util::get_window_geometry(xid, display);
        println!("Geometry: {:?}", geometry);

    }

    // handle contol+c
    ctrlc::set_handler(move || {
        if let Some(p) = &mut app_proc {
            p.kill().unwrap();
            println!("App stopped.");        
        }
        if let Some(p) = &mut display_proc {        
            p.kill().unwrap();        
            println!("Display :{display} stopped.");        
        }
        std::process::exit(0);
    }).unwrap();

    let listener = TcpListener::bind(&input_addr)?;
    println!("Listening on: {}", input_addr);

    // capture channel
    let (capture_img_tx,capture_img_rx) = std::sync::mpsc::channel();
    let (capture_req_tx,capture_req_rx) = std::sync::mpsc::channel();
    let mut capture = Capture::new(xid as u32);
    let (width, height) = capture.get_geometry();
        
    std::thread::spawn(move|| {
        loop {
            let initialized: bool = capture_req_rx.recv().unwrap();
            if !initialized {
                capture.clear();
            }
            let rectangles = capture.get_image(true);
            capture_img_tx.send(rectangles).unwrap();
        }    
    });

    loop {
        let (mut stream, source_addr) = listener.accept()?;
        println!("Connected to client {:?}", source_addr);
        
        // send geometry
        stream.write_u16::<BigEndian>(width as u16).unwrap();
        stream.write_u16::<BigEndian>(height as u16).unwrap();
        
        // setup input
        let mut input = Input::new();
        if !desktop {
            input.set_window(xid);
            let pid = input.get_window_pid();
            println!("window pid: {}", pid);
            input.set_server_geometry(geometry);
            input.focus();    
        }

        let mut capture_busy = false;
        let mut initialized = false;

        loop {
            let message = match ClientEvent::read_from(&mut stream) {
                Err(error) => {
                    println!("Client disconnected");
                    break;
                },
                Ok(message) => message,
            };
            //println!("message from client: {:?}", message);
            
            let mut rectangles = Vec::<Rec>::new();

            match message {
                ClientEvent::KeyEvent {down, key} => {
                    //let keyname = gdk::keys::Key::from(key).name().unwrap();
                    let action = if down {"pressed"} else {"released"};
                    println!("key {}: {}", action, key);
                    if down {
                        //input.key_down(&keyname);
                    } else {
                        //input.key_up(&keyname);
                    }
                },
                ClientEvent::PointerEvent { button_mask, x_position, y_position} => {
                    let action = if button_mask > 0 {"pressed"} else {"release"};
                    println!("button {}: {}, ({},{})", 
                        action, button_mask, x_position, y_position);   
                },
                ClientEvent::FramebufferUpdateRequest {
                    incremental, x_position, y_position, width, height } => {
                    //println!("Update req: {x_position} {y_position} {width} {height}");
                    
                    if !capture_busy {
                        capture_busy = true;
                        capture_req_tx.send(initialized).unwrap();
                        initialized = true
                    }
                    
                    // check if there is a capture ready
                    rectangles = match capture_img_rx.try_recv() {
                        Ok(o) => {capture_busy = false; o},
                        Err(_) => Vec::new(),
                    };
                    //image::save_buffer("image.jpg",
                    // &b[..], width as u32, height as u32, image::ColorType::Rgba8).unwrap();
                    
                },
                _ => {
                    println!("Unknown message");
                }
            }
            
            let message = ServerEvent::FramebufferUpdate {
                count: rectangles.len() as u16,
                rectangles,
            };                
            // send
            message.write_to(&mut stream).unwrap();  
            // for r in &rectangles {
            //     r.write_to(&mut stream).unwrap();
            // }         
        }
    }
    
    Ok(())
}


