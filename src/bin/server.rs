use clap::Parser;
use anyhow::Result;

#[cfg(target_os = "linux")]
mod linux {
    use super::*;
    use remap::{ClientEvent, ServerEvent, Rec, Message};
    use log::{info, warn, error};
    use std::io::{Write};
    use std::net::{TcpListener, TcpStream};
    use std::thread;
    use std::time::{Duration, Instant};

    /// Remote desktop server (Linux only)
    #[derive(Parser, Debug)]
    #[command(author, version, about = "Remap server (Linux only)", long_about = None)]
    pub struct ServerArgs {
        /// Bind address
        #[arg(short, long, default_value = "127.0.0.1")]
        bind: String,

        /// Port to listen on
        #[arg(short, long, default_value_t = 10100)]
        port: u16,

        /// Logical framebuffer width reported to client
        #[arg(long, default_value_t = 1280)]
        width: u16,

        /// Logical framebuffer height reported to client
        #[arg(long, default_value_t = 800)]
        height: u16,

        /// Max FPS to generate when the client requests frames rapidly
        #[arg(long, default_value_t = 60)]
        fps: u32,
    }

    pub fn run() -> Result<()> {
        dotenv::dotenv().ok();
        env_logger::init();

        let args = ServerArgs::parse();
        let bind_addr = (args.bind.as_str(), args.port);
        let listener = TcpListener::bind(bind_addr)?;
        info!("Server listening on {}:{}", args.bind, args.port);

        loop {
            let (mut stream, peer) = listener.accept()?;
            info!("Client connected: {}", peer);

            // Send geometry header (big-endian u16, u16)
            stream.write_all(&args.width.to_be_bytes())?;
            stream.write_all(&args.height.to_be_bytes())?;

            let max_frame_interval = Duration::from_secs_f64(1.0 / args.fps as f64);

            // Per-connection handler
            thread::spawn(move || -> Result<()> {
                let mut tick: u32 = 0;
                let mut last_frame = Instant::now();

                loop {
                    // Block for a client request
                    let req = match ClientEvent::read_from(&mut stream) {
                        Ok(ev) => ev,
                        Err(e) => {
                            info!("Client disconnected: {e:#}");
                            break;
                        }
                    };

                    match req {
                        ClientEvent::FramebufferUpdateRequest { incremental: _, x, y, width, height } => {
                            // Simple rate limit (if client spams requests)
                            let elapsed = last_frame.elapsed();
                            if elapsed < max_frame_interval {
                                thread::sleep(max_frame_interval - elapsed);
                            }
                            last_frame = Instant::now();
                            tick = tick.wrapping_add(1);

                            // Generate a moving gradient BGRA buffer for the requested rect
                            let bytes = gen_gradient_bgra(width, height, tick);

                            let rect = Rec { x, y, width, height, bytes };
                            let reply = ServerEvent::FramebufferUpdate {
                                count: 1,
                                rectangles: vec![rect],
                            };
                            if let Err(e) = reply.write_to(&mut stream) {
                                warn!("Write error, closing connection: {e:#}");
                                break;
                            }
                        }
                        ClientEvent::KeyEvent { down, key } => {
                            info!("Key {:?} {}", key, if down { "down" } else { "up" });
                            // no-op for now
                        }
                        ClientEvent::PointerEvent { buttons, x, y } => {
                            info!("Pointer buttons={:#04x} at ({},{})", buttons, x, y);
                            // no-op for now
                        }
                        ClientEvent::SetEncodings(_) => {
                            // not strictly needed for our raw demo
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
                // Make something that obviously changes frame-to-frame:
                // B = x ^ t, G = y ^ t, R = (x+y) ^ t, A = 255
                bytes[idx + 0] = (x as u8) ^ t;                 // B
                bytes[idx + 1] = (y as u8) ^ t;                 // G
                bytes[idx + 2] = ((x as u16 + y as u16) as u8) ^ t; // R
                bytes[idx + 3] = 255;                           // A
            }
        }
        bytes
    }
}

#[cfg(not(target_os = "linux"))]
mod linux {
    use super::*;
    #[derive(Parser, Debug)]
    #[command(author, version, about = "Remap server (Linux only)", long_about = None)]
    pub struct ServerArgs {}

    pub fn run() -> Result<()> {
        eprintln!("remap server: this binary is implemented for Linux only.");
        Ok(())
    }
}

fn main() -> Result<()> {
    linux::run()
}
