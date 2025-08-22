use std::net::TcpStream;
use std::process::{Command, Stdio};
use std::time::Duration;
use anyhow::{Context, Result};
use byteorder::{BigEndian, ReadBytesExt};
use log::{info, warn};
use remap::{ClientEvent, ServerEvent};
use remap::canvas::Canvas;
use remap::Message;

// helper: wait until a TCP connect to addr works (up to timeout)
fn wait_tcp(addr: &str, total_ms: u64) -> bool {
    let start = std::time::Instant::now();
    while start.elapsed() < Duration::from_millis(total_ms) {
        if TcpStream::connect(addr).is_ok() { return true; }
        std::thread::sleep(Duration::from_millis(120));
    }
    false
}

pub fn main() -> Result<()> {
    dotenv::dotenv().ok();
    env_logger::init();

    let user = std::env::var("REMAP_USER").context("REMAP_USER missing")?;
    let host = std::env::var("REMAP_HOST").context("REMAP_HOST missing")?;
    let port: u16 = 10100;               // consider picking a free one
    
    // If something already binds local_port, fail fast instead of assuming it's our tunnel.
    if TcpStream::connect(("127.0.0.1", port)).is_ok() {
        warn!("Local port {} already accepts connections; not starting SSH (might be the wrong process).", port);
    }

    // Start SSH tunnel, keep child handle so we can clean it up.
    let mut child = Command::new("ssh")
        .args([
            "-N", "-T",
            "-o", "BatchMode=yes",
            "-o", "ExitOnForwardFailure=yes",
            // dev-only hardening/telemetry options:
            "-o", "ServerAliveInterval=30",
            "-o", "ServerAliveCountMax=3",
            // For production, keep StrictHostKeyChecking=accept-new or yes:
            "-o", "StrictHostKeyChecking=no",
            "-L",
            &format!("{port}:127.0.0.1:{port}"),
            &format!("{user}@{host}"),
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())   // inherit stderr so you can see SSH errors
        .spawn()
        .context("failed to spawn ssh tunnel")?;

    // Wait until the tunnel truly accepts connects (up to ~5s)
    let local_addr = format!("127.0.0.1:{port}");
    if !wait_tcp(&local_addr, 5000) {
        // Try to reap SSH; if it already exited, its status will tell you why.
        let _ = child.try_wait();
        anyhow::bail!("SSH tunnel did not become ready on {}", local_addr);
    }
    info!("Tunnel ready on {local_addr}");

    // Connect application stream
    let stream = TcpStream::connect(&local_addr).context("connect to tunnel failed")?;
    stream.set_nodelay(true).ok();
    stream.set_read_timeout(Some(Duration::from_secs(10))).ok();
    stream.set_write_timeout(Some(Duration::from_secs(10))).ok();

    // Split into reader/writer clones
    let mut reader = stream.try_clone()?;
    let writer = stream; // keep original as writer

    // Read initial header (use the same handle consistently)
    let width  = reader.read_u16::<BigEndian>()?;
    let height = reader.read_u16::<BigEndian>()?;
    info!("Server geometry: {}x{}", width, height);

    let (client_tx, client_rx) = flume::unbounded::<ServerEvent>();
    let (canvas_tx, canvas_rx) = flume::unbounded::<ClientEvent>();

    // writer thread
    let mut w = writer;
    std::thread::spawn(move || {
        while let Ok(evt) = canvas_rx.recv() {
            if evt.write_to(&mut w).is_err() { break; }
        }
    });

    // reader thread
    let mut r = reader;
    std::thread::spawn(move || {
        while let Ok(reply) = ServerEvent::read_from(&mut r) {
            let _ = client_tx.send(reply);
        }
        info!("Server disconnected");
    });

    // UI loop
    info!("Connecting to server at 127.0.0.1:{}", port);
    let mut canvas = Canvas::new(canvas_tx, client_rx)?;
    canvas.resize(width as u32, height as u32)?;
    canvas.request_update(false)?;

    while canvas.is_open() {
        canvas.handle_input()?;
        canvas.handle_server_events()?;
        canvas.update()?;
    }

    // Cleanly terminate the ssh tunnel
    // (If you want to keep it, omit this.)
    #[allow(unused_must_use)]
    {
        child.kill();
        child.wait();
    }

    Ok(())
}
