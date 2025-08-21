use anyhow::Result;
use env_logger;
use log::{info};
use std::net::{TcpListener, TcpStream};
use std::io::Write;

use remap_protocol::{ClientEvent, ServerEvent, Message, Rec};

#[cfg(target_os = "linux")]
mod x11; // put your xcb/XDamage capture + (optional) XTest input here

fn main() -> Result<()> {
    env_logger::init();
    let port: u16 = 10100;
    let listener = TcpListener::bind(("127.0.0.1", port))?;
    info!("Server listening on {}", port);

    for conn in listener.incoming() {
        let mut stream = conn?;
        // Example: send geometry first (handshake like your existing code)
        let (width, height) = (1280u16, 800u16); // Replace with real geometry from capture
        stream.write_all(&width.to_be_bytes())?;
        stream.write_all(&height.to_be_bytes())?;
        // Spawn per-client handler that:
        // - reads ClientEvent::FramebufferUpdateRequest
        // - produces ServerEvent::FramebufferUpdate {rects}
        // - loops with damage tracking
    }
    Ok(())
}
