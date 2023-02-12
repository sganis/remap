#![allow(unused)]
use std::io::{Read,Write};
use std::net::TcpListener;
use std::process::{Command, Stdio};
use std::str::FromStr;
use std::{time::Instant};
use clap::Parser;
use anyhow::{Result};
use log::{debug,info,warn,error,trace};
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
    env_logger::init();
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

    info!("Display: :{}", display);
    info!("App: {}", app);
    info!("Args: {:?}", args);
    info!("Port: {}", port);
    info!("Verbosity: {}", cli.verbose);

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
        info!("display pid: {}", p.id());
        display_proc = Some(p);

        // wait for it
        while !util::is_display_server_running(display) {
            info!("Waiging display...");
            std::thread::sleep(std::time::Duration::from_millis(200));
        }    
        
        // run app and get pid
        let p = Command::new(&app)
            .args(&*args)
            .spawn()
            .expect("Could not run app");
        let pid = p.id();
        app_proc = Some(p);
        info!("app pid: {pid}");
        
        // find window ID,. wait for it
        //let window_name = app;
        //xid = util::get_window_id(pid, window_name, display);   
        xid = util::get_window_id(pid, display);   
        while xid == 0 {
            info!("Waiting window id...");
            std::thread::sleep(std::time::Duration::from_millis(200));
            xid = util::get_window_id(pid, display);
        } 
        info!("Window xid: {} ({:#06x})", xid, xid);

        geometry = util::get_window_geometry(xid, display);
        info!("Geometry: {:?}", geometry);

    }

    // handle contol+c
    ctrlc::set_handler(move || {
        if let Some(p) = &mut app_proc {
            p.kill().unwrap();
            info!("App stopped.");        
        }
        if let Some(p) = &mut display_proc {        
            p.kill().unwrap();        
            info!("Display :{display} stopped.");        
        }
        std::process::exit(0);
    }).unwrap();

    let listener = TcpListener::bind(&input_addr)?;
    info!("Listening on: {}", input_addr);

    loop {
        let (mut stream, source_addr) = listener.accept()?;
        info!("Connected to client {:?}", source_addr);

        // channels
        let (capture_tx, capture_rx) = flume::unbounded();
        let (writer_tx, writer_rx) = flume::unbounded();
        // let (input_tx, input_rx) = flume::unbounded();
              
        // capture thread
        let mut capture = Capture::new(xid as u32);
        let (width, height) = capture.get_geometry();
        stream.write_u16::<BigEndian>(width as u16).unwrap();
        stream.write_u16::<BigEndian>(height as u16).unwrap();

        std::thread::spawn(move|| {
            loop {
                let mut incremental = true;
                while let Ok(inc) = capture_rx.try_recv() {
                    trace!("capture_rx.try_recv() ok");
                    incremental = inc;    
                }                
                let t = Instant::now();
                let rectangles = capture.get_image(incremental);
                trace!("capture took: {:?}", t.elapsed());
                if rectangles.len() > 0 {
                    debug!("new rectanges: {}, sending to writer...", rectangles.len());    
                    if writer_tx.send(rectangles).is_err() {
                        break;
                    }
                }
            }    
        });
        
        // writer thread
        let writer = stream.try_clone()?;
        std::thread::spawn(move || {
            let mut writer = writer;
            loop {
                debug!("writer_rx.recv()");
                let rectangles: Vec<Rec> = match writer_rx.recv() {
                    Err(_) => break,
                    Ok(o) => o,
                };
                let message = ServerEvent::FramebufferUpdate {
                    count: rectangles.len() as u16,
                    rectangles,
                };                
                debug!("writing capture to network...");
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
            info!("window pid: {}", pid);
            input.set_server_geometry(geometry);
            input.focus();    
        }

        // // input thread
        // std::thread::spawn(move || {
        //     loop {
        //         let message: ClientEvent = match input_rx.recv() {
        //             Err(e) => break,
        //             Ok(o) => o,
        //         };
        //         match message {
        //             ClientEvent::KeyEvent {down, key} => {
        //                 //let action = if down {"pressed"} else {"released"};
        //                 //info!("key {}: {}", action, key);
        //                 if down {
        //                     input.key_down(key);
        //                 } else {
        //                     input.key_up(key);
        //                 }
        //             },
        //             ClientEvent::PointerEvent { buttons, x, y} => {
        //                 let action = if buttons > 0 {"pressed"} else {"release"};
        //                 info!("button {}: {}, ({},{})", action, buttons, x, y);   
        //             },
        //             _ => ()   
        //         }                
        //     }    
        // });
        
        loop {
            let message = match ClientEvent::read_from(&mut stream) {
                Err(_) => { info!("Client disconnected");  break; },
                Ok(message) => message,
            };
            match message {
                ClientEvent::KeyEvent {down, key} => {
                    let action = if down {"pressed"} else {"released"};
                    info!("key {}: {}", action, key);
                    if down {
                        input.key_down(key);
                    } else {
                        input.key_up(key);
                    }
                },
                ClientEvent::PointerEvent { buttons, x, y} => {
                    let action = if buttons > 0 {"pressed"} else {"release"};
                    info!("button {}: {}, ({},{})", action, buttons, x, y);   
                },                 
                ClientEvent::FramebufferUpdateRequest {incremental, .. } => {
                    if capture_tx.send(incremental).is_err() {
                        break; 
                    }
                },
                _ => { info!("Unknown message from client"); }
            }                               
        }
    }
    
    Ok(())
}


