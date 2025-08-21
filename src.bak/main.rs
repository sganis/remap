use std::process::Command;
use std::net::TcpStream;
use byteorder::{BigEndian, ReadBytesExt};
use anyhow::Result;
use log::{info, debug, trace};
use remap::{ClientEvent, ServerEvent, Message};
use remap::canvas::Canvas;
use remap::util;

pub fn main() -> Result<()> {
    dotenv::dotenv().ok();
    env_logger::init();
    let user = std::env::var("REMAP_USER").expect("REMAP_USER missing");
    let host = std::env::var("REMAP_HOST").expect("REMAP_HOST missing");
    let port: u16 = 10100;

    // setup SSH tunnel
    let (tx,rx) = std::sync::mpsc::channel();
    std::thread::spawn(move|| {
        if util::port_is_listening(port) {
            info!("Tunnel exists");
            tx.send(()).unwrap();
        } else {
            info!("Connecting SSH...");
            let _ = Command::new("ssh")
                .args(["-oStrictHostkeyChecking=no","-N","-L",
                    &format!("{port}:127.0.0.1:{port}"),
                    &format!("{user}@{host}")])
                .spawn().unwrap();
            while !util::port_is_listening(port) {
                std::thread::sleep(std::time::Duration::from_millis(200));
            }
            tx.send(()).unwrap();
        }
    });
    rx.recv().unwrap();
    info!("Tunnel ready");

    // connect
    let mut stream = TcpStream::connect(format!("127.0.0.1:{port}"))?;
    let mut writer = stream.try_clone()?;
    let mut reader = stream.try_clone()?;
    let width = stream.read_u16::<BigEndian>()?;
    let height = stream.read_u16::<BigEndian>()?;
    info!("Server geometry: {}x{}", width, height);

    let (client_tx, client_rx) = flume::unbounded::<ServerEvent>();
    let (canvas_tx, canvas_rx) = flume::unbounded::<ClientEvent>();

    std::thread::spawn(move || {
        while let Ok(evt) = canvas_rx.recv() {
            if evt.write_to(&mut writer).is_err() { break; }
        }
    });
    std::thread::spawn(move || {
        while let Ok(reply) = ServerEvent::read_from(&mut reader) {
            let _ = client_tx.send(reply);
        }
        info!("Server disconnected");
    });

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
