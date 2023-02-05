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
use glib::clone;
use clap::Parser;
use serde::Deserialize;
use remap::{Event, EventAction, Input, Geometry, ClientEvent, ServerEvent, Message};
use remap::util;

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
        String::from("xterm -fa 'Monospace' -fs 18 -geometry 120x30"));
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

    let mut error = 0;

    let window = unsafe {Window::new(xid as u32)};

    let (conn, index) = xcb::Connection::connect(None).unwrap();
    let setup = conn.get_setup();

    let drawable = if desktop {
        let screen = setup.roots().nth(index as usize).unwrap();
        Drawable::Window(screen.root())
    } else {
        Drawable::Window(window)
    };

    let reply = conn.wait_for_reply(
        conn.send_request(&GetGeometry { drawable })
    ).unwrap();
    let (width, height) = (reply.width(), reply.height());
    println!("Geometry xcb: {}x{}", width, height);

    loop {
        let (mut stream, source_addr) = listener.accept()?;
        println!("Connected to client {:?}", source_addr);
    
        let mut input = Input::new();
        input.set_window(xid);
        let pid = input.get_window_pid();
        println!("window pid: {}", pid);
        input.set_server_geometry(geometry);
        
        if !desktop {    
            input.focus();    
        }

        loop {
            let message = match ClientEvent::read_from(&mut stream) {
                Err(error) => {
                    println!("Client disconnected");
                    break;
                },
                Ok(message) => message,
            };
            match message {
                ClientEvent::KeyEvent {down, key} => {
                    let keyname = gdk::keys::Key::from(key).name().unwrap();
                    let action = if down {"pressed"} else {"released"};
                    println!("key {}: {}, name: {}", action, key, keyname);
                    if down {
                        input.key_down(&keyname);
                    } else {
                        input.key_up(&keyname);
                    }
                },
                ClientEvent::PointerEvent { button_mask, x_position, y_position} => {
                    let action = if button_mask > 0 {"pressed"} else {"release"};
                    println!("button {}: {}, ({},{})", 
                        action, button_mask, x_position, y_position);
                },
                ClientEvent::FramebufferUpdateRequest { incremental, x_position, y_position, width, height } => {
                    println!("Frame update: {x_position} {y_position} {width} {height}");
                    let ximage = conn.wait_for_reply(
                        conn.send_request(&GetImage {
                            format: ImageFormat::ZPixmap, drawable, 
                            x: 0, y: 0, width, height, plane_mask: u32::MAX,
                        })
                    ).unwrap();
                    let mut bytes = Vec::from(ximage.data());
                    // BGRA to RGBA
                    for i in (0..bytes.len()).step_by(4) {
                        let b = bytes[i];
                        let r = bytes[i + 2];      
                        bytes[i] = r;
                        bytes[i + 2] = b;
                        bytes[i + 3] = 255;
                    }
                    let message = ServerEvent::FramebufferUpdate {
                        count: 1
                    };
                    message.write_to(&mut stream).unwrap();
                    //image::save_buffer("image.jpg",
                    //    &bytes, width as u32, height as u32, image::ColorType::Rgba8).unwrap();
                    stream.write(&bytes[..]).unwrap();
                },
                _ => {
                    println!("Unknown message");
                }
            }

            continue;

            let mut buf = vec![0; 32];
            let n = stream.read(&mut buf)?;
            // println!("event recieved: {:?}", buf);

            if n == 0 {
                println!("Client disconnected.");
                break;
            }
            
            let event = Event::from_bytes(&buf[..]);
            //println!("event: {:?}", event);
            match event {
                Event { action : EventAction::FramebufferUpdateRequest {
                    incremental, x, y, width, height,
                }, modifiers: m } => {
                    

                },
                Event { action : EventAction::KeyPress {key}, modifiers: m} => {
                    input.key_press(&key, m);
                    if key == "Return" {
                        stream.write(b"OK").unwrap();



                    }
                },
                Event { action: EventAction::Click {x, y, button} , modifiers: m} => {
                    input.mouse_click(x, y, button, m);
                    if button == 1 {
                        stream.write(b"OK").unwrap();
                    }
                },
                Event { action: EventAction::MouseMove {x, y} , modifiers: m} => {
                    input.mouse_move(x, y, m);
                    stream.write(b"OK")?;
                },
                Event { action: EventAction::Scroll {value} , modifiers: m} => {
                    stream.write(b"NA")?;
                },  
                Event { action: EventAction::Resize {width,height} , modifiers: _} => {
                    let geometry = Geometry {width,height};                    
                    stream.write(b"OK")?;
                },  
                _ => {
                    println!("Client sent nothing");
                    break
                }            
            }
        }
    }
    
    Ok(())
}


