use anyhow::Result;
use byteorder::{BigEndian, ReadBytesExt};
use log::info;
use std::net::TcpStream;
use std::process::Command;

use remap::{ClientEvent, ServerEvent, Message};
use remap::canvas::Canvas;
use remap::util;

fn main() -> Result<()> {
    dotenv::dotenv().ok();
    env_logger::init();

    let user = std::env::var("REMAP_USER")?;
    let host = std::env::var("REMAP_HOST")?;
    let port: u16 = 10100;

    // Ensure SSH tunnel is up
    if !util::port_is_listening(port) {
        let _ = Command::new("ssh")
            .args([
                "-oStrictHostkeyChecking=no","-N","-L",
                &format!("{port}:127.0.0.1:{port}"),
                &format!("{user}@{host}"),
            ])
            .spawn()?;
        while !util::port_is_listening(port) {
            std::thread::sleep(std::time::Duration::from_millis(150));
        }
    }
    info!("Tunnel ready on {port}");

    // Connect and read geometry
    let mut stream = TcpStream::connect(("127.0.0.1", port))?;
    let mut writer = stream.try_clone()?;
    let mut reader = stream.try_clone()?;
    let width = stream.read_u16::<BigEndian>()?;
    let height = stream.read_u16::<BigEndian>()?;
    info!("Server geometry: {}x{}", width, height);

    // Channels (explicit types)
    let (client_tx, client_rx) = flume::unbounded::<ServerEvent>();
    let (canvas_tx, canvas_rx) = flume::unbounded::<ClientEvent>();

    // writer thread: canvas -> server
    std::thread::spawn(move || {
        while let Ok(evt) = canvas_rx.recv() {
            if evt.write_to(&mut writer).is_err() { break; }
        }
    });

    // reader thread: server -> canvas
    std::thread::spawn(move || {
        while let Ok(reply) = ServerEvent::read_from(&mut reader) {
            let _ = client_tx.send(reply);
        }
    });

    // UI loop
    let mut canvas = Canvas::new(canvas_tx, client_rx)?;
    canvas.resize(width as u32, height as u32)?;
    canvas.request_update(false)?; // initial full frame

    while canvas.is_open() {
        canvas.handle_input()?;
        canvas.handle_server_events()?;
        canvas.update()?;
    }
    Ok(())
}
