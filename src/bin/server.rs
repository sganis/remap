//! Linux-only Remap server: responds to client FramebufferUpdateRequest with a moving gradient.
//! On non-Linux platforms, it prints a message and exits.

use anyhow::Result;

#[cfg(target_os = "linux")]
mod linux_impl {
    use super::*;
    use clap::Parser;
    use log::{info, warn};
    use std::io::Write;
    use std::net::TcpListener;
    use std::thread;
    use std::time::{Duration, Instant};

    use remap::{ClientEvent, Message, Rec, ServerEvent};

    /// Remap server (Linux only)
    #[derive(Parser, Debug)]
    #[command(author, version, about = "Remap server (Linux only)", long_about = None)]
    struct ServerArgs {
        /// Bind address
        #[arg(short, long, default_value = "127.0.0.1")]
        bind: String,

        /// Port to listen on
        #[arg(short, long, default_value_t = 10100)]
        port: u16,

        /// Reported framebuffer width
        #[arg(long, default_value_t = 1280)]
        width: u16,

        /// Reported framebuffer height
        #[arg(long, default_value_t = 800)]
        height: u16,

        /// Max FPS (when the client requests frames rapidly)
        #[arg(long, default_value_t = 60)]
        fps: u32,
    }

    pub fn main_linux() -> Result<()> {
        dotenv::dotenv().ok();
        env_logger::init();

        let args = ServerArgs::parse();
        let listener = TcpListener::bind((args.bind.as_str(), args.port))?;
        info!("Remap server listening on {}:{}", args.bind, args.port);
        info!("Announcing geometry {}x{}, max {} FPS", args.width, args.height, args.fps);

        let frame_interval = Duration::from_secs_f64(1.0 / args.fps as f64);

        loop {
            let (mut stream, peer) = listener.accept()?;
            info!("Client connected: {}", peer);

            // Send geometry header (big-endian u16, u16)
            stream.write_all(&args.width.to_be_bytes())?;
            stream.write_all(&args.height.to_be_bytes())?;

            // Handle this client in a thread
            let w = args.width;
            let h = args.height;
            let fi = frame_interval;

            thread::spawn(move || -> Result<()> {
                let mut tick: u32 = 0;
                let mut last_frame = Instant::now();

                loop {
                    // Wait for a client message (blocks)
                    let req = match ClientEvent::read_from(&mut stream) {
                        Ok(ev) => ev,
                        Err(e) => {
                            info!("Client disconnected ({e:#})");
                            break;
                        }
                    };

                    match req {
                        ClientEvent::FramebufferUpdateRequest { incremental: _, x, y, width, height } => {
                            // Simple FPS cap in case the client spams requests
                            let elapsed = last_frame.elapsed();
                            if elapsed < fi {
                                thread::sleep(fi - elapsed);
                            }
                            last_frame = Instant::now();
                            tick = tick.wrapping_add(1);

                            // Generate the requested rectangle as a BGRA gradient
                            let bytes = gen_gradient_bgra(width, height, tick);

                            let rect = Rec { x, y, width, height, bytes };
                            let reply = ServerEvent::FramebufferUpdate { count: 1, rectangles: vec![rect] };

                            if let Err(e) = reply.write_to(&mut stream) {
                                warn!("Write error, closing connection: {e:#}");
                                break;
                            }
                        }
                        ClientEvent::KeyEvent { down, key } => {
                            info!("Key {:?} {}", key, if down { "down" } else { "up" });
                        }
                        ClientEvent::PointerEvent { buttons, x, y } => {
                            info!("Pointer buttons={:#04x} at ({},{})", buttons, x, y);
                        }
                        ClientEvent::SetEncodings(_) => {
                            // Not required for this demo; ignore.
                        }
                        ClientEvent::CutText(s) => {
                            info!("CutText: {}", s);
                        }
                    }
                }

                Ok(())
            });
        }
    }

    /// Generate a BGRA gradient for a `width` x `height` rectangle.
    fn gen_gradient_bgra(width: u16, height: u16, tick: u32) -> Vec<u8> {
        let w = width as usize;
        let h = height as usize;
        let mut bytes = vec![0u8; w * h * 4];

        let t = (tick & 0xFF) as u8;
        for y in 0..h {
            for x in 0..w {
                let idx = (y * w + x) * 4;
                // B = x ^ t, G = y ^ t, R = (x+y) ^ t, A = 255
                bytes[idx + 0] = (x as u8) ^ t;                  // B
                bytes[idx + 1] = (y as u8) ^ t;                  // G
                bytes[idx + 2] = ((x as u16 + y as u16) as u8) ^ t; // R
                bytes[idx + 3] = 255;                            // A
            }
        }
        bytes
    }
}

#[cfg(not(target_os = "linux"))]
mod linux_impl {
    use super::*;
    use clap::Parser;

    /// Stub to keep the binary friendly on non-Linux systems.
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
