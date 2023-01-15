use gtk::prelude::*;
use gtk::{Application, ApplicationWindow, Button};
use gst_video::prelude::*;

const APP_ID: &str = "org.gtk_rs.HelloWorld3";

fn post_app_message(playbin: &gst::Element) {
    let _ = playbin.post_message(gst::message::Application::new(gst::Structure::new_empty(
        "tags-changed",
    )));
}

fn main() {
    // // Initialize GTK
    // if let Err(err) = gtk::init() {
    //     eprintln!("Failed to initialize GTK: {}", err);
    //     return;
    // }

    // // Initialize GStreamer
    // if let Err(err) = gst::init() {
    //     eprintln!("Failed to initialize Gst: {}", err);
    //     return;
    // }

    // let uri = "https://www.freedesktop.org/software/gstreamer-sdk/\
    //         data/media/sintel_trailer-480p.webm";
    // let playbin = gst::ElementFactory::make("playbin")
    //     .property("uri", uri)
    //     .build()
    //     .unwrap();

    // playbin.connect("video-tags-changed", false, |args| {
    //     let pipeline = args[0]
    //         .get::<gst::Element>()
    //         .expect("playbin \"video-tags-changed\" args[0]");
    //     post_app_message(&pipeline);
    //     None
    // });

    // playbin.connect("audio-tags-changed", false, |args| {
    //     let pipeline = args[0]
    //         .get::<gst::Element>()
    //         .expect("playbin \"audio-tags-changed\" args[0]");
    //     post_app_message(&pipeline);
    //     None
    // });

    // playbin.connect("text-tags-changed", false, move |args| {
    //     let pipeline = args[0]
    //         .get::<gst::Element>()
    //         .expect("playbin \"text-tags-changed\" args[0]");
    //     post_app_message(&pipeline);
    //     None
    // });

// let window = create_ui(&playbin);

// let bus = playbin.bus().unwrap();
// bus.add_signal_watch();

// let pipeline_weak = playbin.downgrade();
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

// playbin
//     .set_state(gst::State::Playing)
//     .expect("Unable to set the playbin to the `Playing` state");

// gtk::main();
// window.hide();
// playbin
//     .set_state(gst::State::Null)
//     .expect("Unable to set the playbin to the `Null` state");

// bus.remove_signal_watch();
// }




    // Create a new application
    let app = Application::builder().application_id(APP_ID).build();

    // Connect to "activate" signal of `app`
    app.connect_activate(build_ui);

    // Run the application
    app.run();
}


fn build_ui(app: &Application) {
    // Create a button with label and margins
    let button = Button::builder()
        .label("Press me!")
        .margin_top(12)
        .margin_bottom(12)
        .margin_start(12)
        .margin_end(12)
        .build();

    // Connect to "clicked" signal of `button`
    button.connect_clicked(|button| {
        // Set the label to "Hello World!" after the button has been clicked on
        button.set_label("Hello World!");
    });

    // Create a window
    let window = ApplicationWindow::builder()
        .application(app)
        .title("My GTK App")
        .child(&button)
        .build();

    // Present window
    window.present();
}