use std::{ops, os::raw::c_void, process};
use gtk::prelude::*;
use gtk::{Application, ApplicationWindow};
use gst_video::prelude::*;

const APP_ID: &str = "org.gtk_rs.HelloWorld3";

fn main() {
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

    // let bus = pipeline.bus().unwrap();
    // bus.add_signal_watch();

    // let pipeline_weak = pipeline.downgrade();
    // bus.connect_message(None, move |_, msg| {
    //     let pipeline = match pipeline_weak.upgrade() {
    //         Some(pipeline) => pipeline,
    //         None => return,
    //     };

    //     match msg.view() {
    //         //  This is called when an End-Of-Stream message is posted on the bus.
    //         // We just set the pipeline to READY (which stops playback).
    //         gst::MessageView::Eos(..) => {
    //             println!("End-Of-Stream reached.");
    //             pipeline
    //                 .set_state(gst::State::Ready)
    //                 .expect("Unable to set the pipeline to the `Ready` state");
    //         }

    //         // This is called when an error message is posted on the bus
    //         gst::MessageView::Error(err) => {
    //             println!(
    //                 "Error from {:?}: {} ({:?})",
    //                 err.src().map(|s| s.path_string()),
    //                 err.error(),
    //                 err.debug()
    //             );
    //         }
    //         // This is called when the pipeline changes states. We use it to
    //         // keep track of the current state.
    //         gst::MessageView::StateChanged(state_changed) => {
    //             if state_changed.src().map(|s| s == &pipeline).unwrap_or(false) {
    //                 println!("State set to {:?}", state_changed.current());
    //             }
    //         }
    //         _ => (),
    //     }
    // });

    pipeline.set_state(gst::State::Playing)
        .expect("Unable to set the pipeline to the `Playing` state");


    // Create a new application
    let app = Application::builder().application_id(APP_ID).build();

    app.connect_activate(move |app| {
        let video_window = gtk::DrawingArea::new();
        
        // click
        let click = gtk::GestureClick::new();
        click.connect_pressed(|click, press,x,y| {
            click.set_state(gtk::EventSequenceState::Claimed);
            println!("press:{:?},x:{:?},y:{:?}",press,x,y);
        });
        video_window.add_controller(&click);
        
        // // right click
        // let right_click = gtk::GestureClick::new();
        // right_click.set_button(gtk::gdk::ffi::GDK_BUTTON_SECONDARY as u32);
        // right_click.connect_pressed(|right_click, press,x,y| {
        //     right_click.set_state(gtk::EventSequenceState::Claimed);
        //     println!("press:{:?},x:{:?},y:{:?}",press,x,y);
        // });
        // video_window.add_controller(&right_click);
        
        // // mouse move
        // let motion = gtk::EventControllerMotion::new();
        // motion.connect_motion(|_, x,y| {
        //     // motion.set_state(gtk::EventSequenceState::Claimed);
        //     println!("x:{:?},y:{:?}",x,y);
        // });
        // video_window.add_controller(&motion);
        // let video_overlay = sink
        //     .clone()
        //     .dynamic_cast::<gst_video::VideoOverlay>()
        //     .unwrap();

        // let playbin = gst::ElementFactory::make("playbin")
        //     .property("uri", "huri")
        //     .build()
        //     .unwrap();

        // let video_overlay = sink
        //     .clone()
        //     .dynamic_cast::<gst_video::VideoOverlay>()
        //     .unwrap();

        video_window.connect_realize(move |video_window| {
            //let video_overlay = &video_overlay;
            

            //let gdk_window = video_window.get_window();
            
            // if !gdk_window.ensure_native() {
            //     println!("Can't create native window for widget");
            //     process::exit(-1);
            // }

            
            
            // let display_type_name = gdk_window.display().type_().name();
            // println!("window: {:?}, name: {}", gdk_window, display_type_name);

            // extern "C" {
            //     fn gdk_win32_window_get_handle(window: *mut glib::gobject_ffi::GObject) -> u32;
            // }
            // unsafe {
            //     let xid = gdk_win32_window_get_handle(video_window.as_ptr() as *mut _);
            //     println!("xid: {}", xid);
            // }
            
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
        });



        let window = ApplicationWindow::builder()
            .application(app)
            .default_width(1000)
            .default_height(800)
            .title("XRS Client")
            .child(&video_window)
            .build();

        let keyboard = gtk::EventControllerKey::new();
        keyboard.connect_key_pressed(|_, key, code, modifier| {
            println!("key:{:?}, code:{:?}, modifier:{:?}", key, code, modifier);
            gtk::Inhibit(false)
        });
        
        window.add_controller(&keyboard);
        window.show(); // present()

    });

    // Run the application
    app.run();

    // clean up
    pipeline.set_state(gst::State::Null)
        .expect("Unable to set the pipeline to the `Null` state");
    //bus.remove_signal_watch();

}

