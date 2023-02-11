#![allow(unused)]
use std::io::{Read,Write};
use std::net::TcpListener;
use std::process::{Command, Stdio};
use std::str::FromStr;
use std::{time::Instant};
use clap::Parser;
use anyhow::{Result};
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

fn main() -> Result<()> {
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

    loop {
        let (mut stream, source_addr) = listener.accept()?;
        println!("Connected to client {:?}", source_addr);

        // channels
        let (capture_tx, capture_rx) = std::sync::mpsc::channel();
        let (writer_tx, writer_rx) = std::sync::mpsc::channel();
        let (input_tx, input_rx) = std::sync::mpsc::channel();
              
        // capture thread
        let mut capture = Capture::new(xid as u32);
        let (width, height) = capture.get_geometry();
        stream.write_u16::<BigEndian>(width as u16).unwrap();
        stream.write_u16::<BigEndian>(height as u16).unwrap();

        std::thread::spawn(move|| {
            //let mut ncaptures = 0;
            //let mut ncaptures_req = 0;
            loop {
                //let mut queue = Vec::new();
                let mut incremental = true;
                while let Ok(inc) = capture_rx.try_recv() {
                    //ncaptures_req += 1;
                    incremental = inc;    
                    //queue.push(inc);
                }                

                let rectangles = capture.get_image(incremental);
                //ncaptures += 1;
                //let ignored = queue.len() as i32 - 1;
                //println!("req: {}, captures: {}, requests ignored: {}", 
                //    ncaptures_req, ncaptures, ignored);
                if writer_tx.send(rectangles).is_err() {
                    break;
                }
            }    
        });
        
        // writer thread
        let writer = stream.try_clone()?;
        std::thread::spawn(move || {
            let mut writer = writer;
            loop {
                let rectangles: Vec<Rec> = match writer_rx.recv() {
                    Err(_) => break,
                    Ok(o) => o,
                };
                let message = ServerEvent::FramebufferUpdate {
                    count: rectangles.len() as u16,
                    rectangles,
                };                
                if message.write_to(&mut writer).is_err() { 
                    break; 
                }
            }    
        });
        
        // setup input
        let mut input = Input::new();
        if !desktop {
            input.set_window(xid);
            let pid = input.get_window_pid();
            println!("window pid: {}", pid);
            input.set_server_geometry(geometry);
            input.focus();    
        }

        // input thread
        std::thread::spawn(move || {
            loop {
                let message: ClientEvent = match input_rx.recv() {
                    Err(e) => break,
                    Ok(o) => o,
                };
                match message {
                    ClientEvent::KeyEvent {down, key} => {
                        //let action = if down {"pressed"} else {"released"};
                        //println!("key {}: {}", action, key);
                        if down {
                            input.key_down(key);
                        } else {
                            input.key_up(key);
                        }
                    },
                    ClientEvent::PointerEvent { buttons, x, y} => {
                        let action = if buttons > 0 {"pressed"} else {"release"};
                        println!("button {}: {}, ({},{})", 
                            action, buttons, x, y);   
                    },
                    _ => ()   
                }                
            }    
        });
        
        loop {
            let message = match ClientEvent::read_from(&mut stream) {
                Err(_) => { println!("Client disconnected");  break; },
                Ok(message) => message,
            };
            match message {
                ClientEvent::KeyEvent {..} |
                ClientEvent::PointerEvent {..} => {                    
                    if input_tx.send(message).is_err() { 
                        break; 
                    }    
                },
                ClientEvent::FramebufferUpdateRequest {incremental, .. } => {
                    if capture_tx.send(incremental).is_err() {
                        break; 
                    }
                },
                _ => { println!("Unknown message from client"); }
            }                               
        }
    }
    
    Ok(())
}


