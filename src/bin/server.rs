#![allow(dead_code)]
#![allow(unused_variables)]
#![allow(unused_imports)]
#![allow(unused_assignments)]


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

    // Client pointer bit masks (must match the client)
    const BTN_LEFT:       u8 = 0x01;
    const BTN_MIDDLE:     u8 = 0x02;
    const BTN_RIGHT:      u8 = 0x04;
    const BTN_WHEEL_UP:   u8 = 0x08;
    const BTN_WHEEL_DOWN: u8 = 0x10;

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

    fn set_xkb_base(display: u32) {
        let _ = Command::new("setxkbmap")
            .env("DISPLAY", format!(":{display}"))
            .args(["-rules","base","-model","pc105","-layout","us","-option",""])
            .status();
    }



    // --- RANDR helpers: pick or create a mode, best-effort ---

    fn run_cmd(env_display: &str, cmd: &str, args: &[&str]) -> std::io::Result<std::process::Output> {
        std::process::Command::new(cmd)
            .env("DISPLAY", env_display)
            .args(args)
            .output()
    }

    #[derive(Debug, Clone, Copy)]
    struct Mode { w: u16, h: u16 }

    fn parse_xrandr_query(out: &str) -> (Option<String>, Vec<Mode>) {
        // Return (first_connected_output_name, all_modes_for_it)
        let mut output_name: Option<String> = None;
        let mut modes: Vec<Mode> = Vec::new();

        for line in out.lines() {
            // match "<name> connected"
            if let Some((name, _rest)) = line.split_once(" connected") {
                if output_name.is_none() {
                    output_name = Some(name.trim().to_string());
                }
                continue;
            }

            // indented lines with modes like "   1024x768  60.00*"
            let t = line.trim_start();
            if let Some(first) = t.split_whitespace().next() {
                if let Some((w, h)) = first.split_once('x') {
                    if let (Ok(wi), Ok(hi)) = (w.parse::<u16>(), h.parse::<u16>()) {
                        modes.push(Mode { w: wi, h: hi });
                    }
                }
            }
        }
        (output_name, modes)
    }

    fn pick_closest(modes: &[Mode], want: Mode) -> Option<Mode> {
        use std::cmp::Ordering;
        modes.iter().copied().min_by(|a, b| {
            let dw_a = i32::from(a.w) - i32::from(want.w);
            let dh_a = i32::from(a.h) - i32::from(want.h);
            let da = dw_a * dw_a + dh_a * dh_a;

            let dw_b = i32::from(b.w) - i32::from(want.w);
            let dh_b = i32::from(b.h) - i32::from(want.h);
            let db = dw_b * dw_b + dh_b * dh_b;

            da.cmp(&db)
        })
    }

    fn try_set_existing_mode(display: u32, out_name: &str, m: Mode) -> bool {
        let disp = format!(":{}", display);
        // Prefer --output ... --mode WxH (works on most servers)
        let args = ["--output", out_name, "--mode", &format!("{}x{}", m.w, m.h)];
        if let Ok(o) = run_cmd(&disp, "xrandr", &args) {
            if o.status.success() {
                return true;
            }
        }
        // Fallback: -s WxH
        let args2 = ["-s", &format!("{}x{}", m.w, m.h)];
        if let Ok(o) = run_cmd(&disp, "xrandr", &args2) {
            if o.status.success() {
                return true;
            }
        }
        false
    }

    fn try_create_and_set_mode(display: u32, out_name: &str, want: Mode) -> bool {
        let disp = format!(":{}", display);

        // Generate a modeline with cvt (60 Hz). Use "cvt W H 60".
        let cvt_out = match run_cmd(&disp, "cvt", &[&want.w.to_string(), &want.h.to_string(), "60"]) {
            Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).into_owned(),
            _ => return false,
        };

        // Parse: last line like:
        // Modeline "995x781_60.00"  49.50  995 1040 1144 1292  781 784 787 806  -hsync +vsync
        let mut name = None::<String>;
        let mut params = None::<String>;
        for line in cvt_out.lines().rev() {
            if let Some(rest) = line.trim().strip_prefix("Modeline ") {
                // split into "name" and the rest
                if let Some(idx) = rest.find('"') {
                    let rest2 = &rest[idx+1..];
                    if let Some(idx2) = rest2.find('"') {
                        name = Some(rest2[..idx2].to_string());
                        params = Some(rest2[idx2+1..].trim().to_string());
                        break;
                    }
                }
            }
        }
        let (name, params) = match (name, params) {
            (Some(n), Some(p)) => (n, p),
            _ => return false,
        };

        // xrandr --newmode <name> <params...>
        let mut tokens: Vec<&str> = Vec::new();
        tokens.push("--newmode");
        tokens.push(&name);
        // Split params by whitespace preserving tokens
        for t in params.split_whitespace() {
            tokens.push(t);
        }
        if let Ok(o) = run_cmd(&disp, "xrandr", &tokens) {
            if !o.status.success() {
                return false;
            }
        } else {
            return false;
        }

        // xrandr --addmode <output> <name>
        if let Ok(o) = run_cmd(&disp, "xrandr", &["--addmode", out_name, &name]) {
            if !o.status.success() {
                return false;
            }
        } else {
            return false;
        }

        // xrandr --output <output> --mode <name>
        if let Ok(o) = run_cmd(&disp, "xrandr", &["--output", out_name, "--mode", &name]) {
            return o.status.success();
        }
        false
    }

    fn try_resize_display_best_effort(display: u32, w: u16, h: u16) {
        let disp = format!(":{}", display);
        let want = Mode { w, h };

        // 1) Query current outputs + modes
        let (out_name, all_modes) = match run_cmd(&disp, "xrandr", &["-q"]) {
            Ok(o) if o.status.success() => {
                let s = String::from_utf8_lossy(&o.stdout);
                parse_xrandr_query(&s)
            }
            _ => {
                log::debug!("xrandr -q not available or failed; cannot resize.");
                return;
            }
        };

        let Some(out) = out_name else {
            log::debug!("No connected output reported by xrandr; cannot resize.");
            return;
        };

        // 2) If exact mode exists, use it
        if all_modes.iter().any(|m| m.w == w && m.h == h) {
            if try_set_existing_mode(display, &out, want) {
                log::info!("X RANDR: switched to {}x{}", w, h);
                return;
            }
            // fallthrough if failed
        }

        // 3) Try to create the mode (may fail on some Xvfb builds)
        if try_create_and_set_mode(display, &out, want) {
            log::info!("X RANDR: created and switched to {}x{}", w, h);
            return;
        }

        // 4) Fallback to closest available
        if let Some(closest) = pick_closest(&all_modes, want) {
            if try_set_existing_mode(display, &out, closest) {
                log::info!(
                    "X RANDR: requested {}x{} not available; using closest {}x{}",
                    w, h, closest.w, closest.h
                );
                return;
            }
        }

        log::debug!("X RANDR: unable to set {}x{} (kept current mode)", w, h);
    }

    // Optional: if you want to try resizing the X screen when the client resizes.
    #[allow(dead_code)]
    fn try_resize_display(display: u32, w: u16, h: u16) {
        let mode = format!("{}x{}", w, h);
        let disp = format!(":{}", display);
        match Command::new("xrandr")
            .env("DISPLAY", &disp)
            .args(["-s", &mode])
            .output()
        {
            Ok(o) if o.status.success() => info!("X RANDR: set mode {mode}"),
            Ok(o) => {
                let err = String::from_utf8_lossy(&o.stderr);
                debug!("xrandr failed ({}): {}", o.status, err.trim());
            }
            Err(e) => debug!("xrandr not available: {e}"),
        }
    }

    pub fn main_linux() -> Result<()> {
        dotenv::dotenv().ok();
        env_logger::init();

        let args = ServerArgs::parse();
        let display = args.display;
        let port = args.port;

        // Parse command + args (allow quoted args in the default)
        let parts = shell_words::split(&args.app)
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
            // Start Xvfb with a decent 24-bit screen (with no alpha channel)
            let p = Command::new("Xvfb")
                .args([
                    "+extension", "GLX",
                    "+extension", "Composite",
                    "-screen", "0", "1280x800x24",
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
                debug!("Waiting for Xvfb :{display}...");
                std::thread::sleep(std::time::Duration::from_millis(200));
            }
            info!("Xvfb :{} is running.", display);
            set_xkb_base(display);
            info!("Keyboard layout set.");

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

                // // Enlarge the app to the full Xvfb screen
                if xid != 0 {
                    if let Ok((sw, sh)) = util::screen_size(display) {
                        info!("Target Xvfb size: {}x{}", sw, sh);

                        // 1) Politely ask any WM to maximize/fullscreen (EWMH). Harmless on Xvfb-without-WM.
                        if let Err(e) = util::maximize_window(display, xid as u32) {
                            debug!("maximize_window (EWMH) ignored/failed: {:?}", e);
                        }
                        std::thread::sleep(std::time::Duration::from_millis(120));

                        // 2) Force move+resize (works even with no WM). Retry a few times in case the app
                        //    does an initial layout pass and resizes itself once after mapping.
                        for attempt in 1..=8 {
                            // Try move+resize to (0,0, sw, sh). Fall back to util::resize_window_to if you prefer.
                            let _ = util::force_move_resize(display, xid as u32, 0, 0, sw, sh)
                                .or_else(|_| util::resize_window_to(display, xid as u32, sw, sh));

                            std::thread::sleep(std::time::Duration::from_millis(80));

                            let g = util::get_window_geometry(xid, display);
                            if g.width == sw as i32 && g.height == sh as i32 {
                                info!("Window matched screen on attempt {}: {}x{}", attempt, g.width, g.height);
                                break;
                            } else {
                                debug!("still {}x{}, want {}x{} (attempt {})", g.width, g.height, sw, sh, attempt);
                            }
                        }

                        // 3) Re-read final geometry for logging
                        geometry = util::get_window_geometry(xid, display);
                        info!("Window geometry after maximize: {:?} (server)", geometry);
                    } else {
                        info!("Could not read screen size; skipping maximize");
                    }
                }
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
            let (capture_tx, capture_rx) = flume::unbounded::<bool>(); // send 'incremental' flag
            let (writer_tx, writer_rx) = flume::unbounded::<Vec<Rec>>();

            // Create a Capture (xid=0 means screen, non-zero means window)
            let mut capture = Capture::new(xid.max(0) as u32);
            //let mut capture = Capture::new(0);
            let (width, height) = capture.get_geometry();

            // Send initial geometry header (u16 BE, twice)
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
                        // Avoid busy-spin; sleep a bit if nothing changed
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

            // Set up input injection
            let mut input = Input::new();
            if !desktop {
                input.set_window(xid);
                input.set_server_geometry(geometry);
                input.focus();
            }

            // Track latest client size (optional)
            let mut client_w: u16 = width.min(u16::MAX) as u16;
            let mut client_h: u16 = height.min(u16::MAX) as u16;
            let mut last_buttons: u8 = 0;
            
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
                        // Pass through to capture thread
                        let _ = capture_tx.send(incremental);
                    }

                    ClientEvent::KeyEvent { down, key, mods } => {
                        if down { 
                            input.key_down(key, mods);
                         } else { 
                            input.key_up(key, mods); 
                        }
                    }

                    ClientEvent::PointerEvent { buttons, x, y } => {
                        // 1) Always move the pointer first
                        input.mouse_move(x as i32, y as i32, 0);

                        // 2) Handle wheel pulses (client sends them as one-off events)
                        if (buttons & BTN_WHEEL_UP) != 0 {
                            input.mouse_click_button(4); // X11: 4 = wheel up
                        }
                        if (buttons & BTN_WHEEL_DOWN) != 0 {
                            input.mouse_click_button(5); // X11: 5 = wheel down
                        }

                        // 3) Edge-detect normal buttons against previous state
                        let pressed  =  buttons & !last_buttons;
                        let released =  last_buttons & !buttons;

                        // Left
                        if (pressed  & BTN_LEFT)   != 0 { input.mouse_press(1); }
                        if (released & BTN_LEFT)   != 0 { input.mouse_release(1); }

                        // Middle
                        if (pressed  & BTN_MIDDLE) != 0 { input.mouse_press(2); }
                        if (released & BTN_MIDDLE) != 0 { input.mouse_release(2); }

                        // Right
                        if (pressed  & BTN_RIGHT)  != 0 { input.mouse_press(3); }
                        if (released & BTN_RIGHT)  != 0 { input.mouse_release(3); }

                        last_buttons = buttons & (BTN_LEFT | BTN_MIDDLE | BTN_RIGHT);

                        // (Optional) debug
                        // debug!("pointer: btn={:#010b} x={} y={}", buttons, x, y);
                    }


                    ClientEvent::CutText(s) => {
                        debug!("cut text from client: {}", s);
                    }

                    ClientEvent::SetEncodings(_encs) => {
                        // Ignored in this simple server
                    }

                    ClientEvent::ClientResize { width, height } => {
                        info!("client resize -> {}x{}", width, height);
                        client_w = width;
                        client_h = height;

                        // Strategy A (simple): force a full update on next capture tick
                        let _ = capture_tx.send(false);

                        // Try to actually resize the X screen (best-effort; safe to fail)
                        try_resize_display_best_effort(display, width, height);
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
