use std::{ops, os::raw::c_void, process};
use std::io::{Read, Write};
use std::process::Command;
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
use std::net::{TcpListener, TcpStream};
use gdk::prelude::*;
use gst_video::prelude::*;
use gtk::prelude::*;
use std::rc::Rc;
use std::cell::RefCell;
use std::sync::{Arc, Mutex};

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
fn create_ui(playbin: &gst::Element, stream: &mut TcpStream) -> AppWindow {
    
    let main_window = gtk::Window::new(gtk::WindowType::Toplevel);
    
    // wrap it to mutate it in event
    let stream_clone = stream.try_clone().unwrap();
    let stream = Arc::new(stream_clone);

    // clone the handle to s and move it into the closure
    //let st = Arc::clone(&stream);
    main_window.connect("key_press_event", false, move |values| {
        let raw_event = &values[1].get::<gdk::Event>().unwrap();
        match raw_event.downcast_ref::<gdk::EventKey>() {
            Some(event) => {
                println!("Key: {:?}, {:?}", event.keyval(), event.state());
                if let Some(c) = event.keyval().to_unicode() {
                    println!("unicode: {}", c);
                    
                    let k = match event.keyval() {
                        gdk::keys::constants::Return => {
                            stream.as_ref().write(b"\n").unwrap();
                            let mut data = [0; 1]; // using 6 byte buffer
                            match stream.as_ref().read(&mut data) {
                                Ok(_) => {
                                    let c = String::from_utf8_lossy(&data[..]);
                                    if c == "K" {
                                        println!("ok");
                                    } else {
                                        println!("Unexpected reply: {}", c);
                                    }
                                },
                                Err(e) => {
                                    println!("Failed to receive data: {}", e);
                                }
                            }
                            "Return"
                        },
                        _ => "Unknown",
                    };
                   // println!("match: {}", k);
                    
                    
                    if let Err(e) = stream.as_ref().write(&[c as u8]) {
                        println!("Key send error: {e}");
                    }
                    //stream.flush().unwrap();
                } else {
                    let c = match event.keyval() {
                        gdk::keys::constants::Return => "Enter",
                        _ => "Unknown",
                    };
                    println!("{}", c);
                    
                }
                
                
            },
            None => {},
        }
        let result = glib::value::Value::from_type(glib::types::Type::BOOL);
        Some(result)
    });

    main_window.connect("button_press_event", false, |e| {
        let event = &e[1].get::<gdk::Event>().unwrap();
        println!("{:?}", event);    
        let result = glib::value::Value::from_type(glib::types::Type::BOOL);
        Some(result)
    });

    main_window.connect("scroll_event", false, |e| {
        let event = &e[1].get::<gdk::Event>().unwrap();
        println!("{:?}", event);    
        let result = glib::value::Value::from_type(glib::types::Type::BOOL);
        Some(result)
    });
    
    main_window.connect_delete_event(|_, _| {
        gtk::main_quit();
        Inhibit(false)
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

    let video_window = gtk::DrawingArea::new();

    let video_overlay = playbin
        .clone()
        .dynamic_cast::<gst_video::VideoOverlay>()
        .unwrap();

    video_window.connect_realize(move |video_window| {
        return;

        let video_overlay = &video_overlay;
        let gdk_window = video_window.window().unwrap();

        if !gdk_window.ensure_native() {
            println!("Can't create native window for widget");
            process::exit(-1);
        }

        let display_type_name = gdk_window.display().type_().name();
        println!("display type name: {display_type_name}");
        
        #[cfg(target_os = "windows")]
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
        #[cfg(target_os = "linux")]
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
        #[cfg(target_os = "macos")]
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

    //let streams_list = gtk::TextView::new();
    //streams_list.set_editable(false);
    // let pipeline_weak = playbin.downgrade();
    // //let streams_list_weak = glib::SendWeakRef::from(streams_list.downgrade());

    // let bus = playbin.bus().unwrap();

    // #[allow(clippy::single_match)]
    // bus.connect_message(Some("application"), move |_, msg| match msg.view() {
    //     gst::MessageView::Application(application) => {
    //         let pipeline = match pipeline_weak.upgrade() {
    //             Some(pipeline) => pipeline,
    //             None => return,
    //         };
    //     }
    //     _ => unreachable!(),
    // });

    let vbox = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    vbox.pack_start(&video_window, true, true, 0);
    main_window.add(&vbox);
    main_window.set_default_size(1000, 800);
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
    // let host = "166.87.201.134";
    let host = "192.168.100.202";
    let port: u16 = 7001;
    let port2: u16 = 7002;

    // make ssh connection
    let (tx,rx) = std::sync::mpsc::channel();

    // Spawn ssh tunnel thread
    std::thread::spawn(move|| {
        if port_is_listening(port) {
            println!("Tunnel exists, reusing...");            
            tx.send(()).expect("Could not send signal on channel.");
        } else {
            println!("Connecting...");
            let _handle = Command::new("ssh")
                .raw_arg(format!("-oStrictHostkeyChecking=no -N -L {port}:127.0.0.1:{port} -L {port2}:127.0.0.1:{port2} {user}@{host}"))
                .spawn().unwrap();
            while !port_is_listening(port) {
                std::thread::sleep(std::time::Duration::from_millis(200));
            }
            tx.send(()).expect("Could not send signal on channel.");
            // loop {
            //     println!("SSH working...");
            //     thread::sleep(Duration::from_millis(5000));
            // }
        }
    });
    
    // wait for signal
    rx.recv()
        .expect("Could not receive from channel.");
    println!("Tunnel Ok.");
    
    // event connection
    let mut event_stream = TcpStream::connect("127.0.0.1:7002")
        .expect("Cannot connect to input port");
    println!("Event connection Ok.");
    
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
        .property("port", 7001)
        .build()
        .expect("Could not create source element.");
    let demuxer = gst::ElementFactory::make("multipartdemux")
        .name("demuxer")
        .build()
        .expect("Could not create demuxer element");    
    let decoder = gst::ElementFactory::make("jpegdec")
        .name("decoder")
        .build()
        .expect("Could not create decoder element");    
    let sink = gst::ElementFactory::make("glimagesink")
    //let sink = gst::ElementFactory::make("d3dvideosink")
        .name("sink")
        .build()
        .expect("Could not create sink element");    


    
    // Create the empty pipeline
    let pipeline = gst::Pipeline::builder().name("pipeline").build();

    // Build the pipeline
    pipeline.add_many(&[&source, &demuxer, &decoder, &sink]).unwrap();

    // link elements, skip demuxer->decoder for later
    source.link(&demuxer).expect("Elements source-demuxer could not be linked.");
    decoder.link(&sink).expect("Elements decoder-sink could not be linked.");

    // Connect the pad-added signal
    demuxer.connect_pad_added(move |src, src_pad| {
        println!("Received new pad {} from {}", src_pad.name(), src.name());

        let sink_pad = decoder.static_pad("sink")
            .expect("Failed to get static sink pad from decoder");
        if sink_pad.is_linked() {
            println!("We are already linked. Ignoring.");
            return;
        }

        let new_pad_caps = src_pad.current_caps()
            .expect("Failed to get caps of new pad.");
        let new_pad_struct = new_pad_caps
            .structure(0)
            .expect("Failed to get first structure of caps.");
        let new_pad_type = new_pad_struct.name();

        let is_image = new_pad_type.starts_with("image/jpeg");
        if !is_image {
            println!(
                "It has type {} which is not jpeg image. Ignoring.",
                new_pad_type
            );
            return;
        }
        // attempt to link
        let res = src_pad.link(&sink_pad);
        if res.is_err() {
            println!("Type is {} but link failed.", new_pad_type);
        } else {
            println!("Link succeeded (type {}).", new_pad_type);
        }
    });

    // attach video to window
    let window = create_ui(&sink, &mut event_stream);

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
                println!("BUS: {:?}", m);
            },
        }
    });

    //sink.set_state(gst::State::Playing).expect("Unable to set the pipeline to the `Playing` state");
    pipeline.set_state(gst::State::Playing).expect("Unable to set the pipeline to the `Playing` state");

    gtk::main();

    window.hide();
    pipeline.set_state(gst::State::Null).expect("Unable to set the pipeline to the `Null` state");
    bus.remove_signal_watch();
}
