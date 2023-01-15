use std::{ops, os::raw::c_void, process};
use gdk::prelude::*;
use gst_video::prelude::*;
use gtk::prelude::*;

// Custom struct to keep our window reference alive
// and to store the timeout id so that we can remove
// it from the main context again later and drop the
// references it keeps inside its closures
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

// This creates all the GTK+ widgets that compose our application, and registers the callbacks
fn create_ui(playbin: &gst::Element) -> AppWindow {
    let main_window = gtk::Window::new(gtk::WindowType::Toplevel);
    
    main_window.set_events(gdk::EventMask::ALL_EVENTS_MASK);

    main_window.connect_delete_event(|_, _| {
        gtk::main_quit();
        Inhibit(false)
    });

    main_window.connect("key_press_event", false, |values| {
        let raw_event = &values[1].get::<gdk::Event>().unwrap();
        match raw_event.downcast_ref::<gdk::EventKey>() {
            Some(event) => {
                println!("Key name: {:?}", event.keyval());
                println!("Modifier: {:?}", event.state());
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

    
    let video_window = gtk::DrawingArea::new();
    let video_overlay = playbin.clone().dynamic_cast::<gst_video::VideoOverlay>().unwrap();
    video_window.connect_realize(move |video_window| {
        let video_overlay = &video_overlay;
        let gdk_window = video_window.window().unwrap();
        
        if !gdk_window.ensure_native() {
            println!("Can't create native window for widget");
            process::exit(-1);
        }

        let display_type_name = gdk_window.display().type_().name();
        println!("window: {:?}, name: {}", gdk_window, display_type_name);

        // get window handle, does not work in windows yet
        // #[cfg(all(target_os = "windows"))] 
        //  {
        //     extern "C" {
        //         pub fn gdk_win32_window_get_handle(
        //             window: *mut glib::gobject_ffi::GObject,
        //         ) -> *mut c_void;
        //     }

        //     // extern "C" {
        //     //     pub fn gst_video_overlay_set_window_handle(
        //     //         overlay: *mut gst_video::VideoOverlay,
        //     //         handle: u64,
        //     //     ) -> *mut c_void;
        //     // }

        //     #[allow(clippy::cast_ptr_alignment)]
        //     unsafe {
        //         let xid = gdk_win32_window_get_handle(gdk_window.as_ptr() as *mut _) as u64;
        //         println!("window handle: {:?}", xid);
        //         //gst_video_overlay_set_window_handle(video_overlay as *mut _, xid);
        //         //println!("window handle: {:?}", xid);
        //         //let mut xid = gdk_window.get_
        //         video_overlay.set_window_handle(xid as usize);
        //         //video_overlay.set_window_handle((xid as u64).try_into().unwrap());
        //     }
        // }
        
        // #[cfg(all(target_os = "linux"))]
        // {
        //     // Check if we're using X11 or ...
        //     if display_type_name == "GdkX11Display" {
        //         extern "C" {
        //             pub fn gdk_x11_window_get_xid(
        //                 window: *mut glib::gobject_ffi::GObject,
        //             ) -> *mut c_void;
        //         }

        //         #[allow(clippy::cast_ptr_alignment)]
        //         unsafe {
        //             let xid = gdk_x11_window_get_xid(gdk_window.as_ptr() as *mut _);
        //             video_overlay.set_window_handle(xid as usize);
        //         }
        //     } else {
        //         println!("Add support for display type '{}'", display_type_name);
        //         process::exit(-1);
        //     }
        // }
        // #[cfg(all(target_os = "macos"))]
        // {
        //     if display_type_name == "GdkQuartzDisplay" {
        //         extern "C" {
        //             pub fn gdk_quartz_window_get_nsview(
        //                 window: *mut glib::gobject_ffi::GObject,
        //             ) -> *mut c_void;
        //         }

        //         #[allow(clippy::cast_ptr_alignment)]
        //         unsafe {
        //             let window = gdk_quartz_window_get_nsview(gdk_window.as_ptr() as *mut _);
        //             video_overlay.set_window_handle(window as usize);
        //         }
        //     } else {
        //         println!(
        //             "Unsupported display type '{}', compile with `--feature `",
        //             display_type_name
        //         );
        //         process::exit(-1);
        //     }
        // }
    });


    let vbox = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    vbox.pack_start(&video_window, true, true, 0);
    main_window.add(&vbox);
    main_window.set_default_size(640, 480);

    main_window.show_all();

    AppWindow {
        main_window,
        timeout_id: None, //Some(timeout_id),
    }
}

// We are possibly in a GStreamer working thread, so we notify the main
// thread of this event through a message in the bus
// fn post_app_message(playbin: &gst::Element) {
//     let _ = playbin.post_message(gst::message::Application::new(gst::Structure::new_empty(
//         "tags-changed",
//     )));
// }

pub fn main() {
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
        .name("sink")
        .build()
        .expect("Could not create sink element");    

    // Create the empty pipeline
    let pipeline = gst::Pipeline::builder().name("test-pipeline").build();

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

    let window = create_ui(&sink);

    let bus = pipeline.bus().unwrap();
    bus.add_signal_watch();

    let pipeline_weak = pipeline.downgrade();
    bus.connect_message(None, move |_, msg| {
        let pipeline = match pipeline_weak.upgrade() {
            Some(pipeline) => pipeline,
            None => return,
        };

        match msg.view() {
            //  This is called when an End-Of-Stream message is posted on the bus.
            // We just set the pipeline to READY (which stops playback).
            gst::MessageView::Eos(..) => {
                println!("End-Of-Stream reached.");
                pipeline
                    .set_state(gst::State::Ready)
                    .expect("Unable to set the pipeline to the `Ready` state");
            }

            // This is called when an error message is posted on the bus
            gst::MessageView::Error(err) => {
                println!(
                    "Error from {:?}: {} ({:?})",
                    err.src().map(|s| s.path_string()),
                    err.error(),
                    err.debug()
                );
            }
            // This is called when the pipeline changes states. We use it to
            // keep track of the current state.
            gst::MessageView::StateChanged(state_changed) => {
                if state_changed.src().map(|s| s == &pipeline).unwrap_or(false) {
                    println!("State set to {:?}", state_changed.current());
                }
            }
            _ => (),
        }
    });

    pipeline.set_state(gst::State::Playing)
        .expect("Unable to set the pipeline to the `Playing` state");

    gtk::main();

    window.hide();
    pipeline.set_state(gst::State::Null)
        .expect("Unable to set the pipeline to the `Null` state");
    bus.remove_signal_watch();
}
