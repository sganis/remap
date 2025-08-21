use clap::Parser;
use anyhow::{Result, anyhow};
use log::info;
use std::net::TcpStream;
use std::process::Command;

use remap::{ClientEvent, ServerEvent, Message};
use remap::canvas::Canvas;
use remap::util;

/// Remote desktop client
#[derive(Parser, Debug)]
#[command(author, version, about = "Remap client", long_about = None, arg_required_else_help = true)]
struct ClientArgs {
    /// SSH username
    user: String,

    /// SSH host
    host: String,

    /// X11 display (e.g. :0 → port 5900)
    display: String,
}

fn x11_display_to_port(display: &str) -> Result<u16> {
    // e.g. ":0" → 5900, ":1" → 5901
    if let Some(rest) = display.strip_prefix(':') {
        let idx: u16 = rest.parse()?;
        Ok(5900 + idx)
    } else {
        Err(anyhow!("Invalid display format: {display}. Use e.g. :0"))
    }
}

fn ensure_ssh_tunnel(port: u16, ssh_user: &str, ssh_host: &str) -> Result<()> {
    if util::port_is_listening(port) {
        info!("SSH tunnel already up on 127.0.0.1:{port}, reusing.");
        return Ok(());
    }

    info!("Starting SSH tunnel to {ssh_user}@{ssh_host}, forwarding 127.0.0.1:{port} -> 127.0.0.1:{port}...");
    let _child = Command::new("ssh")
        .args([
            "-oStrictHostKeyChecking=no",
            "-N",
            "-L",
            &format!("{port}:127.0.0.1:{port}"),
            &format!("{ssh_user}@{ssh_host}"),
        ])
        .spawn()?;

    for _ in 0..120 {
        if util::port_is_listening(port) {
            info!("SSH tunnel is ready on 127.0.0.1:{port}");
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(150));
    }

    Err(anyhow!("Timed out waiting for SSH tunnel on 127.0.0.1:{port}"))
}

fn main() -> Result<()> {
    dotenv::dotenv().ok();
    env_logger::init();

    let args = ClientArgs::parse();
    let port = x11_display_to_port(&args.display)?;

    ensure_ssh_tunnel(port, &args.user, &args.host)?;
    let (connect_host, connect_port) = ("127.0.0.1".to_string(), port);

    info!("Connecting to {}:{}", connect_host, connect_port);
    let mut stream = TcpStream::connect((connect_host.as_str(), connect_port))?;
    let mut writer = stream.try_clone()?;
    let mut reader = stream.try_clone()?;

    // Read geometry
    use byteorder::{BigEndian, ReadBytesExt};
    let width = stream.read_u16::<BigEndian>()?;
    let height = stream.read_u16::<BigEndian>()?;
    info!("Server geometry: {}x{}", width, height);

    // Channels
    let (client_tx, client_rx) = flume::unbounded::<ServerEvent>();
    let (canvas_tx, canvas_rx) = flume::unbounded::<ClientEvent>();

    // writer thread
    std::thread::spawn(move || {
        while let Ok(evt) = canvas_rx.recv() {
            if evt.write_to(&mut writer).is_err() {
                break;
            }
        }
    });

    // reader thread
    std::thread::spawn(move || {
        while let Ok(reply) = ServerEvent::read_from(&mut reader) {
            let _ = client_tx.send(reply);
        }
    });

    // UI
    let mut canvas = Canvas::new(canvas_tx, client_rx)?;
    canvas.resize(width as u32, height as u32)?;
    canvas.request_update(false)?;

    while canvas.is_open() {
        canvas.handle_input()?;
        canvas.handle_server_events()?;
        canvas.update()?;
    }

    Ok(())
}
