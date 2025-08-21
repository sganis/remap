use anyhow::Result;
use log::info;

#[cfg(target_os = "linux")]
mod linux {
    use super::*;
    use remap::{Message, ServerEvent, Rec};
    use std::io::Write;
    use std::net::{TcpListener};
    use std::thread;
    use std::time::Duration;

    pub fn run() -> Result<()> {
        env_logger::init();
        let port: u16 = 10100;
        let listener = TcpListener::bind(("127.0.0.1", port))?;
        info!("Server listening on {}", port);

        for stream in listener.incoming() {
            let mut stream = stream?;
            thread::spawn(move || -> Result<()> {
                // Send geometry header (example 1280x800)
                let (w, h) = (1280u16, 800u16);
                stream.write_all(&w.to_be_bytes())?;
                stream.write_all(&h.to_be_bytes())?;

                // Very basic demo: periodically send zero rectangles and sleep.
                loop {
                    // No updates (count = 0)
                    let evt = ServerEvent::FramebufferUpdate { count: 0, rectangles: Vec::<Rec>::new() };
                    evt.write_to(&mut stream)?;
                    stream.flush()?;
                    thread::sleep(Duration::from_millis(100));
                }
            });
        }
        Ok(())
    }
}

#[cfg(not(target_os = "linux"))]
mod linux {
    use super::*;
    pub fn run() -> Result<()> {
        env_logger::init();
        eprintln!("Server binary is only implemented on Linux for now.");
        Ok(())
    }
}

fn main() -> Result<()> {
    linux::run()
}
