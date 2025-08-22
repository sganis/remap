//! Remap server (Linux-only): start Xvfb, launch the app (xterm by default),
//! capture rectangles via remap::capture::Capture and send to client,
//! handle input via remap::input::Input, and clean up on Ctrl+C.

use anyhow::Result;
use remap::input::Input;

#[cfg(target_os = "linux")]
mod linux_impl {
    use super::*;
    use clap::Parser;
    use log::{debug, info, trace};
    use std::io::Write;
    use std::net::TcpListener;
    use std::process::Command;
    use std::time::Instant;

    use remap::{util, ClientEvent, Message, Rec, ServerEvent};
    use remap::capture::Capture;

    /// Remap server (Linux only)
    #[derive(Parser, Debug)]
    #[command(author, version, about = "Remap server (Linux only)", long_about = None)]
    struct ServerArgs {
        /// X display number (e.g. 100 -> :100)
        #[arg(short, long, default_value_t = 100)]
        display: u32,

        /// App (and args) to run
        #[arg(short, long, default_value = "xterm -fa 'Monospace' -fs 14 -geometry 110x24")]
        app: String,

        /// TCP port to listen on
        #[arg(short, long, default_value_t = 10100)]
        port: u16,

        /// Increase verbosity (-v, -vv, -vvv)
        #[arg(short, long, action = clap::ArgAction::Count)]
        verbose: u8,
    }

    pub fn main_linux() -> Result<()> {
        dotenv::dotenv().ok();
        //env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
        env_logger::init();
        

        let args = ServerArgs::parse();
        let display = args.display;
        let port = args.port;

        // Parse command + args (allow quoted args in the default)
        let mut parts = shell_words::split(&args.app)
            .unwrap_or_else(|_| args.app.split_whitespace().map(|s| s.to_string()).collect());
        let app = parts.get(0).cloned().unwrap_or_else(|| "xterm".to_string());
        let app_args = if parts.len() > 1 { &parts[1..] } else { &[] };

        let desktop = app == "desktop"; // if you ever want a headless "desktop" mode

        info!("Display: :{}", display);
        info!("App: {}", app);
        info!("Args: {:?}", app_args);
        info!("Port: {}", port);
        info!("Verbosity: {}", args.verbose);

        if !desktop {
            std::env::set_var("DISPLAY", format!(":{display}"));
        }

        // Keep child handles here so Ctrl+C can kill them.
        let mut display_proc: Option<std::process::Child> = None;
        let mut app_proc: Option<std::process::Child> = None;

        // Start Xvfb if not desktop mode
        if !desktop {
            // Start Xvfb with a decent 24-bit screen (with alpha channel)
            let p = Command::new("Xvfb")
                .args([
                    "+extension", "GLX",
                    "+extension", "Composite",
                    "-screen", "0", "2048x1024x24+32",
                    "-nolisten", "tcp",
                    "-noreset",
                    "-dpi", "96",
                    &format!(":{display}"),
                ])
                .spawn()
                .expect("Failed to start Xvfb");
            info!("Xvfb pid: {}", p.id());
            display_proc = Some(p);

            // Wait until Xvfb is up
            while !util::is_display_server_running(display) {
                info!("Waiting for Xvfb :{display}...");
                std::thread::sleep(std::time::Duration::from_millis(200));
            }

            // Launch the app
            let p = Command::new(&app)
                .args(app_args)
                .spawn()
                .expect("Failed to start app");
            info!("App pid: {}", p.id());
            app_proc = Some(p);
        }

        // Find the app's window and its geometry
        let mut xid: i32 = 0;
        let mut geometry = remap::Geometry::default();
        if !desktop {
            // Try to identify window by pid+name (like your original)
            // We use the process we just spawned; if app_proc is None, keep xid=0
            if let Some(ref p) = app_proc {
                let pid = p.id();
                xid = util::get_window_id(pid, &app, display);
                info!("Waiting for window id...");
                while xid == 0 {                    
                    std::thread::sleep(std::time::Duration::from_millis(200));
                    xid = util::get_window_id(pid, &app, display);
                }
                info!("Window xid: {} ({:#06x})", xid, xid);
                geometry = util::get_window_geometry(xid, display);
                info!("Window geometry: {:?} (server)", geometry);
            }
        }

        // On Ctrl+C: kill the child processes and exit
        {
            let mut app_proc = app_proc.take();
            let mut display_proc = display_proc.take();
            ctrlc::set_handler(move || {
                if let Some(p) = &mut app_proc {
                    let _ = p.kill();
                    info!("App stopped.");
                }
                if let Some(p) = &mut display_proc {
                    let _ = p.kill();
                    info!("Display :{} stopped.", display);
                }
                std::process::exit(0);
            })?;
        }

        // Listen for client connections
        let addr = format!("127.0.0.1:{port}");
        let listener = TcpListener::bind(&addr)?;
        info!("Listening on {}", addr);

        loop {
            let (mut stream, peer) = listener.accept()?;
            info!("Client connected: {}", peer);

            // Channels for capture→writer pipeline and capture control
            let (capture_tx, capture_rx) = flume::unbounded::<bool>();
            let (writer_tx, writer_rx) = flume::unbounded::<Vec<Rec>>();

            // Create a Capture (xid=0 means screen, non-zero means window)
            let mut capture = Capture::new(xid.max(0) as u32);
            let (width, height) = capture.get_geometry();
            // Send initial geometry header
            stream.write_all(&u16::try_from(width).unwrap_or(0).to_be_bytes())?;
            stream.write_all(&u16::try_from(height).unwrap_or(0).to_be_bytes())?;

            // Spawn capture thread
            std::thread::spawn(move || {
                loop {
                    // Default to incremental=true unless a request arrives
                    let mut incremental = true;
                    while let Ok(inc) = capture_rx.try_recv() {
                        trace!("capture_rx: set incremental={inc}");
                        incremental = inc;
                    }
                    let t0 = Instant::now();
                    let rects = capture.get_image(incremental);
                    trace!("capture.get_image({incremental}) took {:?}", t0.elapsed());

                    if !rects.is_empty() {
                        debug!("capture produced {} rectangles -> writer", rects.len());
                        if writer_tx.send(rects).is_err() {
                            break;
                        }
                    } else {
                        // Avoid busy-spin; sleep a tiny bit if nothing changed
                        std::thread::sleep(std::time::Duration::from_millis(4));
                    }
                }
            });

            // Spawn writer thread (sends ServerEvent::FramebufferUpdate)
            let writer_stream = stream.try_clone()?;
            std::thread::spawn(move || {
                let mut writer = writer_stream;
                while let Ok(rects) = writer_rx.recv() {
                    let evt = ServerEvent::FramebufferUpdate {
                        count: rects.len() as u16,
                        rectangles: rects,
                    };
                    if evt.write_to(&mut writer).is_err() {
                        break;
                    }
                }
            });

            // Set up input injection (optional if you compiled remap::input)
            let mut input = Input::new();
            if !desktop {
                input.set_window(xid);
                input.set_server_geometry(geometry);
                input.focus();
            }

            // Handle client messages on this connection
            loop {
                let msg = match ClientEvent::read_from(&mut stream) {
                    Ok(m) => m,
                    Err(_) => {
                        info!("Client disconnected");
                        break;
                    }
                };

                match msg {
                    ClientEvent::FramebufferUpdateRequest { incremental, .. } => {
                        let _ = capture_tx.send(incremental);
                    }
                    ClientEvent::KeyEvent { down, key, mods } => {
                        let action = if down { "down" } else { "up" };
                        debug!("key {}: {}", action, key);
                        if down { input.key_down(key, mods); } else { input.key_up(key, mods); }
                    }
                    ClientEvent::PointerEvent { buttons, x, y } => {
                        let action = if buttons > 0 { "pressed" } else { "released" };
                        debug!("pointer {}: {}, ({},{})", action, buttons, x, y);
                        // (Add button->click mapping in Input if desired)
                    }
                    other => {
                        debug!("Unhandled client event: {:?}", other);
                    }
                }
            }
        }
    }
}

#[cfg(not(target_os = "linux"))]
mod linux_impl {
    use super::*;
    use clap::Parser;

    #[derive(Parser, Debug)]
    #[command(author, version, about = "Remap server (Linux only)", long_about = None)]
    struct ServerArgs {}

    pub fn main_linux() -> Result<()> {
        eprintln!("remap server: this binary is implemented for Linux only.");
        Ok(())
    }
}

fn main() -> Result<()> {
    linux_impl::main_linux()
}
