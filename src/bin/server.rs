use std::error::Error;
use std::io::{Read,Write};
use std::net::TcpListener;
use std::process::{Command, Stdio};
use clap::Parser;
use serde::Deserialize;
use remap::{Event, EventAction, Input, Geometry};
use gst::prelude::*;
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
    let port1 = cli.port.unwrap_or(10100);
    let port2 = port1 + 100;
    let input_addr = format!("127.0.0.1:{port2}");
    let mut display_proc = None;
    let mut app_proc = None;
    let mut xid = 0;
    let mut geometry = Geometry::default();

    println!("Display: :{}", display);
    println!("App: {}", app);
    println!("Args: {:?}", args);
    println!("Port 1: {}", port1);
    println!("Port 2: {}", port2);
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

    // run video server
    let mut xidstr = String::from("");
    if !desktop {
        xidstr = format!("xid={xid}");
    }

    // Initialize GStreamer
    if let Err(err) = gst::init() {
        eprintln!("Failed to initialize Gst: {}", err);
        std::process::exit(1);
    }

    // let mut video_proc = Command::new("gst-launch-1.0")
    //     .args([
    //         "ximagesrc",&xidstr,"use-damage=0","show-pointer=0",
    //         "!","queue",
    //         "!","videoconvert",
    //         "!","video/x-raw,framerate=30/1",
    //         "!","jpegenc",
    //         "!","multipartmux",
    //         "!","tcpserversink", "host=127.0.0.1", &format!("port={port1}")
    //     ])
    //     //.stdout(Stdio::null())
    //     //.stderr(Stdio::null())
    //     .spawn()
    //     .expect("video stream failed to start");
    // println!("video pid: {}", video_proc.id());


    let source = gst::ElementFactory::make("ximagesrc")
        .name("source")
        .property_from_str("use-damage", "0")
        .property_from_str("xid", &format!("{xid}"))
        .build()
        .expect("Could not create source element.");
    let videoscale = gst::ElementFactory::make("videoscale")
        .name("videoscale")
        .build()
        .expect("Could not create videoconvert element");     
    let caps = gst::Caps::builder("video/x-raw")
        .field("width", &gst::IntRange::<i32>::new(0, geometry.width))
        .field("height", &gst::IntRange::<i32>::new(0, geometry.height))
        .field("framerate", &gst::FractionRange::new(
            gst::Fraction::new(0, 1), 
            gst::Fraction::new(10, 1))) // 10 fps
        .build();
    let capsfilter = gst::ElementFactory::make("capsfilter")
        .name("capsfilter")
        .property("caps", &caps)
        .build()
	    .expect("Could not create filter element.");
    let videoconvert = gst::ElementFactory::make("videoconvert")
        .name("videoconvert")
        .build()
        .expect("Could not create videoconvert element");     
    let jpegenc = gst::ElementFactory::make("jpegenc")
        .name("jpegenc")
        .build()
        .expect("Could not create jpegenc element");     
    let sink = gst::ElementFactory::make("tcpserversink")
        .name("sink")
        .property_from_str("host","127.0.0.1")
        .property_from_str("port", &format!("{port1}"))
        .build()
        .expect("Could not create sink element");    
    let pipeline = gst::Pipeline::builder().name("pipeline").build();
    pipeline.add_many(&[&source, &videoscale, &capsfilter, &videoconvert, &jpegenc, &sink])
        .expect("Could not add elements to pipeline");
    gst::Element::link_many(&[&source, &videoscale, &capsfilter, &videoconvert, &jpegenc, &sink])
        .expect("Could not link elements");
    pipeline.set_state(gst::State::Playing)
        .expect("Unable to set the pipeline to the Playing state");


    // handle contol+c
    ctrlc::set_handler(move || {
        //video_proc.kill().expect("Failed to stop streaming");
        //println!("Streaming stopped.");
        pipeline.set_state(gst::State::Null).
            expect("Unable to set the pipeline to the `Null` state");
    
        if let Some(p) = &mut app_proc {
            p.kill().expect("Failed to stop app");
            println!("App stopped.");        
        }
        if let Some(p) = &mut display_proc {        
            p.kill().expect("Failed to stop display");        
            println!("Display :{display} stopped.");        
        }
        std::process::exit(0);
    })
    .expect("Error setting Ctrl-C handler");

    let listener = TcpListener::bind(&input_addr)?;
    println!("Listening on: {}", input_addr);

    let mut error = 0;

    loop {
        let (mut stream, source_addr) = listener.accept()?;
        println!("Connected to client {:?}", source_addr);
    
        let mut input = Input::new();
        input.set_server_geometry(geometry);

        if !desktop {
            input.set_window(xid);
            input.focus();
            let pid = input.get_window_pid();
            println!("window pid: {}", pid);
        }

        loop {
            let mut buf = vec![0; 32];
            let n = stream.read(&mut buf)
                .expect("failed to read data from stream");
            println!("click recieved: {:?}", buf);

            if n == 0 {
                println!("Client disconnected.");
                break;
            }
            
            let event = Event::from_bytes(&buf[..]);
            println!("event: {:?}", event);
            match event {
                Event { action : EventAction::KeyPress {key}, modifiers: m} => {
                    input.key_press(&key, m);
                    if key == "Return" {
                        stream.write(b"OK")
                            .expect("failed to write data to socket");
                    }
                },
                Event { action: EventAction::Click {x, y, button} , modifiers: m} => {
                    input.mouse_click(x, y, button, m);
                    if button == 1 {
                        stream.write(b"OK")
                            .expect("failed to write data to socket");
                    }
                },
                Event { action: EventAction::MouseMove {x, y} , modifiers: m} => {
                    input.mouse_move(x, y, m);
                    stream.write(b"OK")
                        .expect("failed to write data to socket");                    
                },
                Event { action: EventAction::Scroll {value} , modifiers: m} => {
                    stream.write(b"NA")
                        .expect("failed to write data to socket");
                    // todo!();
                },  
                Event { action: EventAction::Resize {w,h} , modifiers: _} => {
                    //println!("resize: ")
                    stream.write(b"OK")
                        .expect("failed to write data to socket");
                    // todo!();
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


