use std::process::Command;
//use std::time::Instant;
use std::sync::mpsc;
use anyhow::Result;
use remap::canvas::Canvas;
use remap::util;
use remap::{ClientEvent, ServerEvent};
use tokio::{
    io::{self, AsyncRead, AsyncReadExt, AsyncWrite},
    sync::mpsc::{Receiver, Sender}, 
    net::TcpStream,
};

pub struct Client<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    stream: T,
    width: u16,
    height: u16,
}

impl<T> Client<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    pub fn new(stream: T, width: u16, height: u16) -> Self {
        Self { stream, width, height }
    }

    /// Drives the protocol:
    /// - sends initial full-frame request
    /// - forwards Canvas input (ClientEvent) -> server
    /// - forwards server replies (ServerEvent) -> Canvas
    pub async fn run(
        &mut self,
        client_tx: Sender<ServerEvent>,
        mut canvas_rx: Receiver<ClientEvent>,
    ) -> Result<()> {
        // Split the stream so reads and writes can proceed concurrently.
        // This borrows `self.stream` for the duration of the loop.
        let (mut reader, mut writer) = tokio::io::split(&mut self.stream);

        // Initial full framebuffer request
        let init = ClientEvent::FramebufferUpdateRequest {
            incremental: false,
            x: 0,
            y: 0,
            width: self.width,
            height: self.height,
        };
        println!("Client: sending initial full-frame request");
        init.write(&mut writer).await?;

        loop {
            tokio::select! {
                // Canvas → Server (write outbound client events)
                maybe_evt = canvas_rx.recv() => {
                    match maybe_evt {
                        Some(evt) => {
                            println!("Client: sending event {:?}", evt);
                            evt.write(&mut writer).await?;
                        }
                        None => {
                            // Canvas dropped the sender: graceful shutdown.
                            break;
                        }
                    }
                }

                // Server → Canvas (read inbound server events)
                srv = ServerEvent::read(&mut reader) => {
                    match srv {
                        Ok(msg) => {
                            // Forward to Canvas; if Canvas is gone, we can exit.
                            println!("Client: received event {:?}", msg);
                            if client_tx.send(msg).await.is_err() {
                                break;
                            }
                        }
                        Err(e) => {
                            
                            // I/O error or EOF
                            // Treat UnexpectedEof as clean disconnect; others bubble up.
                            if let Some(ioe) = e.downcast_ref::<io::Error>() {
                                if ioe.kind() == io::ErrorKind::UnexpectedEof {
                                    break;
                                }
                            }
                            return Err(e);
                        }
                    }
                }
            }
        }

        Ok(())
    }
}


#[tokio::main]
async fn main() -> Result<()> {
    dotenv::dotenv().expect(".env file missing");

    let user = std::env::var("REMAP_USER").expect("REMAP_USER env var missing");
    let host = std::env::var("REMAP_HOST").expect("REMAP_HOST env var missing");
    let port: u16 = 10100;

    // ---- SSH tunnel (same pattern you like) ----
    let (tx, rx) = mpsc::channel();

    std::thread::spawn(move || {
        if util::port_is_listening(port) {
            println!("Tunnel exists, reusing...");
            tx.send(()).expect("Could not send signal on channel.");
        } else {
            println!("Connecting...");
            let _handle = Command::new("ssh")
                .args([
                    "-oStrictHostkeyChecking=no",
                    "-N",
                    "-L",
                    &format!("{port}:127.0.0.1:{port}"),
                    &format!("{user}@{host}"),
                ])
                .spawn()
                .expect("failed to spawn ssh");
            while !util::port_is_listening(port) {
                std::thread::sleep(std::time::Duration::from_millis(200));
            }
            tx.send(()).expect("Could not send signal on channel.");
        }
    });

    // wait for tunnel
    rx.recv().expect("Could not receive from channel.");
    println!("Tunnel Ok.");

    // ---- Async TCP connection ----
    let mut stream = TcpStream::connect(&format!("127.0.0.1:{port}")).await?;
    println!("Connected");

    // Read geometry header: 2×u16 (big-endian)
    let mut hdr = [0u8; 4];
    stream.read_exact(&mut hdr).await?;
    let width = u16::from_be_bytes([hdr[0], hdr[1]]);
    let height = u16::from_be_bytes([hdr[2], hdr[3]]);
    println!("Geometry: {}x{}", width, height);

    // ---- Channels (Tokio mpsc) ----
    let (canvas_tx, canvas_rx) = tokio::sync::mpsc::channel(100);
    let (client_tx, client_rx) = tokio::sync::mpsc::channel(100);

    // ---- Network client task (owns the stream) ----
    let mut client = Client::new(stream, width, height);
    tokio::spawn(async move {
        if let Err(e) = client.run(client_tx, canvas_rx).await {
            eprintln!("client error: {e}");
        }
    });

    // ---- Canvas/UI loop (async) ----
    let mut canvas = Canvas::new(canvas_tx, client_rx).await?;
    canvas.resize(width as u32, height as u32).await?;

    //let mut frames = 0_u32;
    //let mut start = Instant::now();

    while canvas.is_open() {
        canvas.handle_input().await?;
        canvas.handle_server_events().await?;
        canvas.update().await?;
        canvas.request_update(true).await?;

        // frames += 1;
        // if start.elapsed().as_secs() >= 1 {
        //     let fps = frames as f64 / start.elapsed().as_millis() as f64 * 1000.0;
        //     println!("{fps:.0}");
        //     start = Instant::now();
        //     frames = 0;
        // }
    }

    canvas.close().await?;
    Ok(())
}
