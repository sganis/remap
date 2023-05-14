// we import the necessary modules (only the core X module in this application).
use xcb::{x};
// we need to import the `Xid` trait for the `resource_id` call down there.
use xcb::{Xid};
// use xcb::damage::NotifyEvent;

// Many xcb functions return a `xcb::Result` or compatible result.
fn main() -> xcb::Result<()> {
    // Connect to the X server.
    let (conn, screen_num) = xcb::Connection::connect_with_extensions(
        None,
        &[xcb::Extension::Damage],
        &[]
    )?;

    // Fetch the `x::Setup` and get the main `x::Screen` object.
    let setup = conn.get_setup();
    let screen = setup.roots().nth(screen_num as usize).unwrap();

    

    // Generate an `Xid` for the client window.
    // The type inference is needed here.
    let window: x::Window = conn.generate_id();

    // We can now create a window. For this we pass a `Request`
    // object to the `send_request_checked` method. The method
    // returns a cookie that will be used to check for success.
    let cookie = conn.send_request_checked(&x::CreateWindow {
        depth: x::COPY_FROM_PARENT as u8,
        wid: window,
        parent: screen.root(),
        x: 0,
        y: 0,
        width: 600,
        height: 400,
        border_width: 10,
        class: x::WindowClass::InputOutput,
        visual: screen.root_visual(),
        // this list must be in same order than `Cw` enum order
        value_list: &[
            x::Cw::BackPixel(screen.white_pixel()),
            x::Cw::EventMask(
                x::EventMask::EXPOSURE | 
                x::EventMask::KEY_PRESS |
                //x::EventMask::KEY_RELEASE |
                x::EventMask::BUTTON_PRESS |
                //x::EventMask::BUTTON_RELEASE |
                //x::EventMask::COLOR_MAP_CHANGE |
                //x::EventMask::STRUCTURE_NOTIFY |
                x::EventMask::SUBSTRUCTURE_NOTIFY 
                //x::EventMask::NO_EVENT
                //x::EventMask::SUBSTRUCTURE_REDIRECT |
                //x::EventMask::RESIZE_REDIRECT 
                //x::EventMask::PROPERTY_CHANGE
                //x::EventMask::POINTER_MOTION
            ),
            
        ],
    });
    // We now check if the window creation worked.
    // A cookie can't be cloned; it is moved to the function.
    conn.check_request(cookie)?;

    // Let's change the window title
    let cookie = conn.send_request_checked(&x::ChangeProperty {
        mode: x::PropMode::Replace,
        window,
        property: x::ATOM_WM_NAME,
        r#type: x::ATOM_STRING,
        data: b"My XCB Window",
    });
    conn.check_request(cookie)?;
    
    // We now show ("map" in X terminology) the window.
    // This time we do not check for success, so we discard the cookie.
    conn.send_request(&x::MapWindow { window });

    // We need a few atoms for our application.
    // We send a few requests in a row and wait for the replies after.
    let (wm_protocols, wm_del_window, wm_state, wm_state_maxv, wm_state_maxh) = {
        let cookies = (
            conn.send_request(&x::InternAtom {
                only_if_exists: true,
                name: b"WM_PROTOCOLS",
            }),
            conn.send_request(&x::InternAtom {
                only_if_exists: true,
                name: b"WM_DELETE_WINDOW",
            }),
            conn.send_request(&x::InternAtom {
                only_if_exists: true,
                name: b"_NET_WM_STATE",
            }),
            conn.send_request(&x::InternAtom {
                only_if_exists: true,
                name: b"_NET_WM_STATE_MAXIMIZED_VERT",
            }),
            conn.send_request(&x::InternAtom {
                only_if_exists: true,
                name: b"_NET_WM_STATE_MAXIMIZED_HORZ",
            }),
        );
        (
            conn.wait_for_reply(cookies.0)?.atom(),
            conn.wait_for_reply(cookies.1)?.atom(),
            conn.wait_for_reply(cookies.2)?.atom(),
            conn.wait_for_reply(cookies.3)?.atom(),
            conn.wait_for_reply(cookies.4)?.atom(),
        )
    };

    // We now activate the window close event by sending the following request.
    // If we don't do this we can still close the window by clicking on the "x" button,
    // but the event loop is notified through a connection shutdown error.
    conn.check_request(conn.send_request_checked(&x::ChangeProperty {
        mode: x::PropMode::Replace,
        window,
        property: wm_protocols,
        r#type: x::ATOM_ATOM,
        data: &[wm_del_window],
    }))?;


    // graphic context to draw something
    let gc: x::Gcontext = conn.generate_id();
    let rectangles: &[x::Rectangle] = &[
        x::Rectangle {
            x: 10,
            y: 50,
            width: 40,
            height: 20,
        },
        x::Rectangle {
            x: 80,
            y: 50,
            width: 10,
            height: 40,
        },
    ];
    conn.send_request(&x::CreateGc {
        cid: gc,
        drawable: x::Drawable::Window(window),
        value_list: &[
            x::Gc::Foreground(screen.black_pixel()),
            x::Gc::GraphicsExposures(false),
        ],
    });
    conn.flush()?;

    conn.wait_for_reply(conn.send_request(&xcb::damage::QueryVersion {
		client_major_version: xcb::damage::MAJOR_VERSION,
		client_minor_version: xcb::damage::MINOR_VERSION,
	}))?;

	// conn.check_request(conn.send_request_checked(&xcb::damage::Create {
	// 	damage: conn.generate_id(),
	// 	drawable: xcb::x::Drawable::Window(root_window),
	// 	level: xcb::damage::ReportLevel::RawRectangles,
	// })).unwrap();


    // create damage
    let damage: xcb::damage::Damage = conn.generate_id();   
    conn.send_request(&xcb::damage::Create {
        damage,
        drawable: x::Drawable::Window(window),
        //drawable: x::Drawable::Window(screen.root()),
        level: xcb::damage::ReportLevel::NonEmpty
        //level: xcb::damage::ReportLevel::RawRectangles,
    });
    
    //let damage_data = xcb::damage::get_extension_data(&conn);
    //println!("damage extension: {:#?}", damage_data);
    
    // Previous request was checked, so a flush is not necessary in this case.
    // Otherwise, here is how to perform a connection flush.
    conn.flush()?;

    let mut maximized = false;




    // We enter the main event loop
    loop {
        let event = match conn.wait_for_event() {
            Err(xcb::Error::Connection(xcb::ConnError::Connection)) => {
                // graceful shutdown, likely "x" close button clicked 
                break Ok(());
            }
            Err(err) => {
                println!("unexpected error: {:#?}", err);
                continue;
            }
            Ok(event) => event,
        };
        println!("event: {:?}", event);

        match event {
            xcb::Event::X(x::Event::Expose(_ev)) => {
                // let drawable = x::Drawable::Window(window);

                // /* We draw the points */
                // conn.send_request(&x::PolyPoint {
                //     coordinate_mode: x::CoordMode::Origin,
                //     drawable,
                //     gc,
                //     points,
                // });
                // conn.flush()?;
            }
            xcb::Event::X(x::Event::KeyPress(ev)) => {
                println!("key event: {:?}", ev);

                if ev.detail() == 0x3a {
                    // The M key was pressed
                    // (M only on qwerty keyboards. Keymap support is done
                    // with the `xkb` extension and the `xkbcommon-rs` crate)

                    // We toggle maximized state, for this we send a message
                    // by building a `x::ClientMessageEvent` with the proper
                    // atoms and send it to the server.

                    let data = x::ClientMessageData::Data32([
                        if maximized { 0 } else { 1 },
                        wm_state_maxv.resource_id(),
                        wm_state_maxh.resource_id(),
                        0,
                        0,
                    ]);
                    let event = x::ClientMessageEvent::new(window, wm_state, data);
                    let cookie = conn.send_request_checked(&x::SendEvent {
                        propagate: false,
                        destination: x::SendEventDest::Window(screen.root()),
                        event_mask: x::EventMask::STRUCTURE_NOTIFY,
                        event: &event,
                    });
                    conn.check_request(cookie)?;

                    // Same as before, if we don't check for error, we have to flush
                    // the connection.
                    // conn.flush()?;

                    maximized = !maximized;
                } else if ev.detail() == 0x18 {
                    // Q (on qwerty)

                    // We exit the event loop (and the program)
                    break Ok(());
                }
            }
            xcb::Event::X(x::Event::KeyRelease(ev)) => {
                println!("key release: {:?}", ev);
            }            
            xcb::Event::X(x::Event::ButtonPress(ev)) => {
                println!("Button pressed: {:?}", ev);

                let drawable = x::Drawable::Window(window);

                /* We draw the rectangles */
                conn.send_request(&x::PolyRectangle {
                    drawable,
                    gc,
                    rectangles,
                });

                conn.flush()?;

            }
            xcb::Event::X(x::Event::ButtonRelease(ev)) => {
                println!("Button released: {:?}", ev);
            }
            xcb::Event::X(x::Event::MotionNotify(ev)) => {
                println!("Motion notify: {:?}", ev);
            }       
            xcb::Event::X(x::Event::ClientMessage(ev)) => {
                // We have received a message from the server
                println!("event: {:?}", ev);

                if let x::ClientMessageData::Data32([atom, ..]) = ev.data() {
                    if atom == wm_del_window.resource_id() {
                        // The received atom is "WM_DELETE_WINDOW".
                        // We can check here if the user needs to save before
                        // exit, or in our case, exit right away.
                        break Ok(());
                    }
                }
            }         
            xcb::Event::Damage(xcb::damage::Event::Notify(ev)) => {
                println!("damage event: {:#?}", ev);
            }
            e => {
                println!("other event: {:#?}", e);
            }
        }
    }
}