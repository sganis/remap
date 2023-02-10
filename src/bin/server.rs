#![allow(unused)]
use std::process::Command;
use tokio::net::TcpListener;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use anyhow::Result;
use clap::Parser;
use remap::{Rec, Geometry, ClientEvent, ServerEvent};
use remap::capture::Capture;
use remap::input::Input;
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

#[tokio::main]
async fn main() -> Result<()> {
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

    let listener = TcpListener::bind(&input_addr).await?;
    println!("Listening on: {}", input_addr);

    // capture channel
    let (capture_tx, mut capture_rx) = tokio::sync::mpsc::channel(100);
    let (server_tx, server_rx) = tokio::sync::mpsc::channel(100);

    let mut capture = Capture::new(xid as u32);
    let (width, height) = capture.get_geometry();
       

    tokio::spawn(async move { 
        capture.run(capture_tx, server_rx).await.unwrap() 
    });


    loop {
        let (mut stream, source_addr) = listener.accept().await?;
        println!("Connected to client {:?}", source_addr);
        
        // send geometry
        stream.write_u16(width as u16).await?;
        stream.write_u16(height as u16).await?;
        
        // setup input
        let mut input = Input::new();
        if !desktop {
            input.set_window(xid);
            let pid = input.get_window_pid();
            println!("window pid: {}", pid);
            input.set_server_geometry(geometry);
            input.focus();    
        }

        loop {
            println!("server looping...");
            if let Ok(rectangles) = capture_rx.try_recv() {                  
                println!("capture recieved ############################");
                let count = rectangles.len() as u16;
                let mut i = 0;
                for r in rectangles.iter() {
                    i+= 1;
                    println!("rec {}: {}", i, r);                    
                }
                let message = ServerEvent::FramebufferUpdate { count, rectangles };    
                println!("sending message, count: {}", count);                
                message.write(&mut stream).await?;     

                println!("done sending message, count: {}", count);
                 
            } else {
                println!("no caputure yet");
            }
            
            println!("Waiting client message...");
            let client_msg = ClientEvent::read(&mut stream).await?;
            println!("message from client: {:?}", client_msg);
        
            let mut rectangles = Vec::<Rec>::new();

            match client_msg {
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
                ClientEvent::PointerEvent { button_mask, x, y} => {
                    let action = if button_mask > 0 {"pressed"} else {"release"};
                    println!("button {}: {}, ({},{})", 
                        action, button_mask, x, y);   
                },
                ClientEvent::FramebufferUpdateRequest {
                    incremental, x, y, width, height } => {
                    //println!("Update req: {x} {y} {width} {height}");                    
                    server_tx.send(incremental).await?;
                    
                },
                _ => {
                    println!("Unknown message");
                }
            }                    
        }
    }
    
    Ok(())
}


