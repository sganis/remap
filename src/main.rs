use std::{ops, os::raw::c_void, process};
use std::io::{Read, Write};
use std::process::Command;
use std::net::{TcpListener, TcpStream};
use gdk::{prelude::*, ModifierType};
use gst_video::prelude::*;
use gtk::prelude::*;
use std::sync::{Mutex};
use lazy_static::lazy_static;
use remap::{Event, EventAction, Modifier};


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

fn create_ui(playbin: &gst::Element) -> AppWindow {

    let main_window = gtk::Window::new(gtk::WindowType::Toplevel);
    main_window.set_events(
        gdk::EventMask::PROPERTY_CHANGE_MASK
    );
    let video_window = gtk::GLArea::new();
    video_window.set_events(
        gdk::EventMask::BUTTON_PRESS_MASK |
        gdk::EventMask::SCROLL_MASK |
        gdk::EventMask::PROPERTY_CHANGE_MASK
    );

    main_window.connect_key_press_event(move |_, e| {
        let name = e.keyval().name().unwrap().as_str().to_string();
        let modifiers = e.state();
        println!("Key: {:?}, code: {}, state: {:?}, unicode: {:?}, name: {:?}, modifiers: {}", 
            e.keyval(), 
            *e.keyval(),
            e.state(), 
            e.keyval().to_unicode(), 
            name, modifiers);
        let modifiers = e.state().bits();     
        // let a = ModifierType::CONTROL_MASK;
        // let m = e.state().contains(a);
        // println!(" contains control: {}", m);

        let mut stream = &TCP.lock().unwrap()[0];
        if name == "Return" {            
            let mut event = Event {
                action: EventAction::KeyPress { key: name },
                modifiers,
            };
            stream.write(&event.as_bytes()).unwrap();
            let mut data = [0; 2]; // using 2 byte buffer
            match stream.read(&mut data) {
                Ok(_) => {
                    let c = String::from_utf8_lossy(&data[..]);
                    println!("Response: {}", c);                            
                },
                Err(e) => {
                    println!("Failed to receive data: {}", e);
                }
            }
        } else if name == "BackSpace" || name == "Delete" ||
                name == "Page_Down" || name == "Page_Up" ||
                name == "Up" || name == "Down" ||
                name == "Left" || name == "Right" ||
                name == "Home" || name == "End" ||
                name == "Tab" || name == "Escape" {
            let mut event = Event {
                action: EventAction::KeyPress { key: name },
                modifiers,
            };
            stream.write(&event.as_bytes()).unwrap();
            //stream.flush().unwrap();
        } else {
            match e.keyval().to_unicode() {
                Some(k) => {
                    let mut event = Event {
                        action: EventAction::KeyPress { key: k.to_string() },
                        modifiers,
                    };
                    stream.write(&event.as_bytes()).unwrap();
                    //stream.flush().unwrap();
                    println!("key sent: {k}");
                            
                },
                None => {
                    println!("key not supported: {name}");
                }
            }                   
        }                
        Inhibit(true)
    });

    video_window.connect_button_press_event(|_, e| {
        //println!("{:?}", e);    
        println!("{:?}, state: {:?}, button: {}", e.position(), e.state(), e.button());
        let button = e.button();
        let modifiers = e.state().bits();
        let mut stream = &TCP.lock().unwrap()[0];
        let mut event = Event {
            action: EventAction::Click {
                x: e.position().0 as i32,
                y: e.position().1 as i32,
                button,
            },
            modifiers,
        };
        
        stream.write(&event.as_bytes()).expect("Could not send mouse event");

        if button == 1 {
            let mut data = [0; 2]; 
            match stream.read(&mut data) {
                Ok(_) => {
                    let c = String::from_utf8_lossy(&data[..]);
                    println!("Response: {}", c);                            
                },
                Err(e) => {
                    println!("Failed to receive data: {}", e);
                }
            }
        }
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
    
    video_window.connect_resize(|_, w, h| {
        println!("size: {w},{h}");
        ()
    });
    
    main_window.connect_property_notify_event(|_, e| {
        println!("notification: {:?}", e);
        Inhibit(true)
    });

    let pipeline = playbin.clone();

    // // Update the UI (seekbar) every second
    let timeout_id = glib::timeout_add_seconds_local(1, move || {
        let pipeline = &pipeline;
        if let Some(dur) = pipeline.query_duration::<gst::ClockTime>() {
            if let Some(pos) = pipeline.query_position::<gst::ClockTime>() {
                //lslider.block_signal(&slider_update_signal_id);
                //lslider.set_value(pos.seconds() as f64);
                //lslider.unblock_signal(&slider_update_signal_id);
            }
        }

        Continue(true)
    });

    
    let video_overlay = playbin
        .clone()
        .dynamic_cast::<gst_video::VideoOverlay>()
        .unwrap();

    video_window.connect_realize(move |video| {
        //return;

        let video_overlay = &video_overlay;
        let gdk_window = video.window().unwrap();

        if !gdk_window.ensure_native() {
            println!("Can't create native window for widget");
            process::exit(-1);
        }

        let display_type_name = gdk_window.display().type_().name();
        println!("display type name: {display_type_name}");
        
        #[cfg(windows)]
        {
            // Check if we're using X11 or ...
            if display_type_name == "GdkWin32Display" {
                extern "C" {
                    pub fn gdk_win32_window_get_handle(
                        window: *mut glib::gobject_ffi::GObject,
                    ) -> *mut c_void;
                }

                #[allow(clippy::cast_ptr_alignment)]
                unsafe {
                    let xid = gdk_win32_window_get_handle(gdk_window.as_ptr() as *mut _);
                    video_overlay.set_window_handle(xid as usize);
                }
            } else {
                println!("Add support for display type '{}'", display_type_name);
                process::exit(-1);
            }
        } 
        #[cfg(linux)]
        {
            // Check if we're using X11 or ...
            if display_type_name == "GdkX11Display" {
                extern "C" {
                    pub fn gdk_x11_window_get_xid(
                        window: *mut glib::gobject_ffi::GObject,
                    ) -> *mut c_void;
                }

                #[allow(clippy::cast_ptr_alignment)]
                unsafe {
                    let xid = gdk_x11_window_get_xid(gdk_window.as_ptr() as *mut _);
                    video_overlay.set_window_handle(xid as usize);
                }
            } else {
                println!("Add support for display type '{}'", display_type_name);
                process::exit(-1);
            }
        }
        #[cfg(macos)]
        {
            if display_type_name == "GdkQuartzDisplay" {
                extern "C" {
                    pub fn gdk_quartz_window_get_nsview(
                        window: *mut glib::gobject_ffi::GObject,
                    ) -> *mut c_void;
                }

                #[allow(clippy::cast_ptr_alignment)]
                unsafe {
                    let window = gdk_quartz_window_get_nsview(gdk_window.as_ptr() as *mut _);
                    video_overlay.set_window_handle(window as usize);
                }
            } else {
                println!(
                    "Unsupported display type '{}', compile with `--feature `",
                    display_type_name
                );
                process::exit(-1);
            }
        }
    });

    let vbox = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    vbox.pack_start(&video_window, true, true, 0);
    main_window.add(&vbox);
    main_window.set_default_size(1600, 1000);
    main_window.show_all();

    AppWindow {
        main_window,
        timeout_id: Some(timeout_id),
    }
}
fn port_is_listening(port: u16) -> bool {
    match TcpListener::bind(("127.0.0.1", port)) {
        Ok(_) => false,
        Err(_) => true,
    }
}
pub fn main() {
    
    let user = "san";
    //let host = "ecclap.chaintrust.com";
    let host = "192.168.100.202";
    let port1: u16 = 10100;
    let port2 = port1 + 100;
    
    // video overlay does not work with this env var
    std::env::set_var("GTK_CSD","0");
    std::env::set_var("GDK_WIN32_LAYERED","0");
    
    // make ssh connection
    let (tx,rx) = std::sync::mpsc::channel();

    // Spawn ssh tunnel thread
    std::thread::spawn(move|| {
        if port_is_listening(port1) {
            println!("Tunnel exists, reusing...");            
            tx.send(()).expect("Could not send signal on channel.");
        } else {
            println!("Connecting...");
            let _handle = Command::new("ssh")
                .args(["-oStrictHostkeyChecking=no","-N","-L", 
                    &format!("{port1}:127.0.0.1:{port1}"),"-L",
                    &format!("{port2}:127.0.0.1:{port2}"),
                    &format!("{user}@{host}")])
                .spawn().unwrap();
            while !port_is_listening(port1) {
                std::thread::sleep(std::time::Duration::from_millis(200));
            }
            tx.send(()).expect("Could not send signal on channel.");
        }
    });
    
    // wait for signal
    rx.recv()
        .expect("Could not receive from channel.");
    println!("Tunnel Ok.");
    
    // event connection
    let event_stream = TcpStream::connect(&format!("127.0.0.1:{port2}"))
        .expect("Cannot connect to input port");
    println!("Event connection Ok.");
    {
        let mut guard = TCP.lock().unwrap();
        guard.push(event_stream);
    }
    // Initialize GTK
    if let Err(err) = gtk::init() {
        eprintln!("Failed to initialize GTK: {}", err);
        return;
    }

    // Initialize GStreamer
    if let Err(err) = gst::init() {
        eprintln!("Failed to initialize Gst: {}", err);
        return;
    }

    let source = gst::ElementFactory::make("tcpclientsrc")
        .name("source")
        .property_from_str("host", "127.0.0.1")
        .property_from_str("port", &format!("{port1}"))
        .build()
        .expect("Could not create source element."); 
    let decoder = gst::ElementFactory::make("jpegdec")
        .name("decoder")
        .build()
        .expect("Could not create decoder element");    
    let sink = gst::ElementFactory::make("glimagesink")
        .name("sink")
        .build()
        .expect("Could not create sink element");    
    let window = create_ui(&sink);

    // Create the empty pipeline
    let pipeline = gst::Pipeline::builder().name("pipeline").build();
    pipeline.add_many(&[&source, &decoder, &sink]).unwrap();
    source.link(&decoder).expect("Elements source-demuxer could not be linked.");
    decoder.link(&sink).expect("Elements decoder-sink could not be linked.");

    // attach video to window
    //let window = create_ui(&sink);

    // // attache test video
    // let uri = "https://www.freedesktop.org/software/gstreamer-sdk/\
    //             data/media/sintel_trailer-480p.webm";
    // let playbin = gst::ElementFactory::make("playbin")
    //     .property("uri", uri).build().unwrap();
    
    // let window = create_ui(&playbin);
    // playbin.set_state(gst::State::Playing).unwrap();
    


    let bus = pipeline.bus().unwrap();
    bus.add_signal_watch();

    let pipeline_weak = pipeline.downgrade();
    let sink_weak = sink.downgrade();

    bus.connect_message(None, move |_, msg| {        
        let pipeline = pipeline_weak.upgrade();
        let sink = sink_weak.upgrade();
        if pipeline.is_none() && sink.is_none() {
            return;
        }
        let pipeline = pipeline.unwrap();
        let sink = sink.unwrap();

        //println!("bus message: {:?} ", msg.view());

        match msg.view() {
            //  This is called when an End-Of-Stream message is posted on the bus.
            // We just set the pipeline to READY (which stops playback).
            gst::MessageView::Eos(..) => {
                println!("End-Of-Stream reached.");
                pipeline
                    .set_state(gst::State::Ready)
                    .expect("Unable to set the pipeline to the `Ready` state");
            },
            // This is called when an error message is posted on the bus
            gst::MessageView::Error(err) => {
                println!(
                    "ERROR: {:?}: {} ({:?})",
                    err.src().map(|s| s.path_string()),
                    err.error(),
                    err.debug()
                );
            },
            // This is called when the pipeline changes states. We use it to
            // keep track of the current state.
            gst::MessageView::StateChanged(state_changed) => {
                if state_changed.src().map(|s| s == &pipeline).unwrap_or(false) {
                    println!("State set to {:?}", state_changed.current());
                }
            },
            gst::MessageView::Tag(m) => {
                println!("TAG: {:?}", m);
            },
            gst::MessageView::Element(m) => {
                // println!("ELEMENT: {:?}", m);
            },
            m => {
                //println!("BUS: {:?}", m);
            },
        }
    });

    //sink.set_state(gst::State::Playing).expect("Unable to set the pipeline to the `Playing` state");
    pipeline.set_state(gst::State::Playing).expect("Unable to set the pipeline to the `Playing` state");

    //gdk::set_show_events(true);

    gtk::main();

    window.hide();
    pipeline.set_state(gst::State::Null).expect("Unable to set the pipeline to the `Null` state");
    bus.remove_signal_watch();
}
