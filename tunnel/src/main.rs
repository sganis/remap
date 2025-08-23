use std::process::Stdio;
use clap::Parser;
use anyhow::{anyhow, Context, Result};
use dotenvy::dotenv;

use ssh2::Session;
use std::{
    path::{Path, PathBuf},
    process::Command,
    sync::Arc,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::{mpsc, Mutex},
    time::{sleep, Duration},
};

/// Command-line options
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// SSH user (can come from .env or env USER)
    #[arg(long, env = "USER", default_value = "")]
    user: String,

    /// SSH host (can come from .env or env HOST)
    #[arg(long, env = "HOST", default_value = "")]
    host: String,

    /// SSH port (default = 22)
    #[arg(long, env = "SSH_PORT", default_value = "22")]
    ssh_port: u16,

    /// Local port to bind for tunnel (default = 9000)
    #[arg(long, env = "LOCAL_PORT", default_value = "9000")]
    local_port: u16,

    /// Remote port to forward to (default = 9000)
    #[arg(long, env = "REMOTE_PORT", default_value = "9000")]
    remote_port: u16,
}

pub async fn curl_through_tunnel(port: u16) -> Result<String> {
    let url = format!("http://127.0.0.1:{port}/");

    // Use tokio::process::Command for async execution
    let output = tokio::process::Command::new("curl")
        .args([
            "-4",                       // force IPv4, matches 127.0.0.1
            "--silent", "--show-error", // quiet stdout, keep errors
            "--fail-with-body",         // non-2xx returns error + body on stderr
            "--http1.1",
            "--header", "Connection: close",
            &url,
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        let err = String::from_utf8_lossy(&output.stderr);
        Err(anyhow!("curl exited with {}: {}", output.status, err))
    }
}

/// SSH tunnel manager that handles the SSH session and connections
pub struct SshTunnel {
    session: Arc<Mutex<Session>>,
    _shutdown_tx: mpsc::Sender<()>,
    handle: tokio::task::JoinHandle<Result<()>>,
}

impl SshTunnel {
    /// Create a new SSH tunnel
    pub async fn new(
        ssh_host: &str,
        ssh_port: u16,
        user: &str,
        privkey_path: &Path,
        local_bind: (&str, u16),
        remote_dst: (&str, u16),
    ) -> Result<Self> {
        let (ready_tx, mut ready_rx) = mpsc::channel::<Result<()>>(1);
        let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);

        let ssh_host = ssh_host.to_string();
        let user = user.to_string();
        let pk = privkey_path.to_path_buf();
        let local_bind = (local_bind.0.to_string(), local_bind.1);
        let remote_dst = (remote_dst.0.to_string(), remote_dst.1);

        let handle = tokio::task::spawn(async move {
            // Connect SSH session in a blocking task since ssh2 is synchronous
            let session = tokio::task::spawn_blocking(move || -> Result<Session> {
                println!("Connecting to SSH server at {}:{}...", ssh_host, ssh_port);
                
                let tcp = std::net::TcpStream::connect((ssh_host.as_str(), ssh_port))
                    .with_context(|| format!("connect SSH {}:{}", ssh_host, ssh_port))?;

                let mut sess = Session::new().context("new ssh2::Session")?;
                sess.set_tcp_stream(tcp);
                sess.handshake().context("SSH handshake failed")?;

                if !pk.exists() {
                    return Err(anyhow!("Private key not found at: {}", pk.display()));
                }

                println!("Authenticating user: {}", user);
                sess.userauth_pubkey_file(&user, None, &pk, None)
                    .with_context(|| format!("userauth_pubkey_file for {} with key {}", user, pk.display()))?;
                
                if !sess.authenticated() {
                    return Err(anyhow!("SSH authentication failed for user: {}", user));
                }

                println!("SSH authentication successful");
                sess.set_keepalive(true, 30);
                Ok(sess)
            })
            .await
            .context("SSH connection task panicked")??;

            let session = Arc::new(Mutex::new(session));

            // Bind local listener
            let bind_addr = format!("{}:{}", local_bind.0, local_bind.1);
            println!("Binding local listener on: {}", bind_addr);
            let listener = TcpListener::bind(&bind_addr)
                .await
                .with_context(|| format!("bind local listener {}", bind_addr))?;

            println!("Tunnel ready on port {}", local_bind.1);
            let _ = ready_tx.send(Ok(())).await;

            // Accept connections
            loop {
                tokio::select! {
                    // Handle new connections
                    result = listener.accept() => {
                        match result {
                            Ok((client, peer)) => {
                                println!("New connection from: {}", peer);
                                
                                let session = Arc::clone(&session);
                                let remote_host = remote_dst.0.clone();
                                let remote_port = remote_dst.1;
                                
                                tokio::task::spawn(async move {
                                    if let Err(e) = handle_connection(client, session, &remote_host, remote_port).await {
                                        eprintln!("Connection handler error: {:#}", e);
                                    }
                                });
                            }
                            Err(e) => {
                                eprintln!("Accept error: {}", e);
                                sleep(Duration::from_millis(100)).await;
                            }
                        }
                    }
                    
                    // Handle shutdown signal
                    _ = shutdown_rx.recv() => {
                        println!("Shutdown signal received");
                        break;
                    }
                }
            }

            println!("Tunnel shutdown complete");
            Ok(())
        });

        // Wait for tunnel to be ready
        if let Some(result) = ready_rx.recv().await {
            result?;
        } else {
            return Err(anyhow!("Tunnel setup failed"));
        }

        Ok(Self {
            session: Arc::new(Mutex::new(Session::new().unwrap())), // Placeholder, not used
            _shutdown_tx: shutdown_tx,
            handle,
        })
    }

    /// Shutdown the tunnel
    pub async fn shutdown(self) -> Result<()> {
        // shutdown_tx is dropped when self is dropped, signaling shutdown
        self.handle.await??;
        Ok(())
    }
}

/// Handle a single client connection
async fn handle_connection(
    mut client: TcpStream,
    session: Arc<Mutex<Session>>,
    remote_host: &str,
    remote_port: u16,
) -> Result<()> {
    println!("Opening channel to {}:{}", remote_host, remote_port);

    // Create SSH channel in blocking task
    let channel = {
        let session = Arc::clone(&session);
        let remote_host = remote_host.to_string();
        tokio::task::spawn_blocking(move || -> Result<ssh2::Channel> {
            // Note: This is a simplified approach. In production, you'd want to handle
            // the SSH session more carefully to avoid blocking the async runtime.
            let sess = session.try_lock()
                .map_err(|_| anyhow!("Failed to acquire session lock"))?;
            
            let channel = sess.channel_direct_tcpip(&remote_host, remote_port, None)
                .with_context(|| format!("channel_direct_tcpip to {}:{}", remote_host, remote_port))?;
            
            Ok(channel)
        })
        .await
        .context("Channel creation task failed")??
    };

    let channel = Arc::new(Mutex::new(channel));

    // Split the client stream
    let (mut client_reader, mut client_writer) = client.split();

    // Channel for coordinating shutdown
    let (close_tx, mut close_rx) = mpsc::channel::<()>(1);

    // Task 1: client -> channel (upstream)
    let channel_upstream = Arc::clone(&channel);
    let close_tx_upstream = close_tx.clone();
    let upstream = tokio::task::spawn(async move {
        let mut buffer = [0u8; 32 * 1024];
        loop {
            match client_reader.read(&mut buffer).await {
                Ok(0) => {
                    println!("Client closed connection (upstream)");
                    // Send EOF to channel in blocking task
                    let channel = Arc::clone(&channel_upstream);
                    let _ = tokio::task::spawn_blocking(move || {
                        if let Ok(mut ch) = channel.try_lock() {
                            let _ = ch.send_eof();
                            let _ = ch.flush();
                        }
                    }).await;
                    break;
                }
                Ok(n) => {
                    // Write to channel in blocking task
                    let channel = Arc::clone(&channel_upstream);
                    let data = buffer[..n].to_vec();
                    match tokio::task::spawn_blocking(move || -> Result<()> {
                        let mut ch = channel.try_lock()
                            .map_err(|_| anyhow!("Failed to acquire channel lock"))?;
                        ch.write_all(&data)?;
                        ch.flush()?;
                        Ok(())
                    }).await {
                        Ok(Ok(())) => {},
                        Ok(Err(e)) => {
                            eprintln!("Failed to write to channel: {}", e);
                            break;
                        }
                        Err(e) => {
                            eprintln!("Channel write task failed: {}", e);
                            break;
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Error reading from client: {}", e);
                    break;
                }
            }
        }
        let _ = close_tx_upstream.send(()).await;
    });

    // Task 2: channel -> client (downstream)
    let channel_downstream = Arc::clone(&channel);
    let close_tx_downstream = close_tx.clone();
    let downstream = tokio::task::spawn(async move {
        let mut buffer = [0u8; 32 * 1024];
        loop {
            // Read from channel in blocking task
            let read_result = {
                let channel = Arc::clone(&channel_downstream);
                tokio::task::spawn_blocking(move || -> Result<Option<Vec<u8>>> {
                    let mut ch = channel.try_lock()
                        .map_err(|_| anyhow!("Failed to acquire channel lock"))?;
                    
                    // Set non-blocking mode
                    ch.set_blocking(false);
                    
                    match ch.read(&mut buffer) {
                        Ok(0) => Ok(None), // EOF
                        Ok(n) => Ok(Some(buffer[..n].to_vec())),
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            Ok(Some(Vec::new())) // No data available
                        }
                        Err(e) => Err(e.into()),
                    }
                })
                .await
            };

            match read_result {
                Ok(Ok(Some(data))) => {
                    if data.is_empty() {
                        // No data available, sleep briefly
                        sleep(Duration::from_millis(10)).await;
                        continue;
                    }
                    
                    if let Err(e) = client_writer.write_all(&data).await {
                        eprintln!("Failed to write to client: {}", e);
                        break;
                    }
                }
                Ok(Ok(None)) => {
                    println!("Remote closed connection (downstream)");
                    break;
                }
                Ok(Err(e)) => {
                    eprintln!("Error reading from channel: {}", e);
                    break;
                }
                Err(e) => {
                    eprintln!("Channel read task failed: {}", e);
                    break;
                }
            }
        }
        let _ = close_tx_downstream.send(()).await;
    });

    // Wait for either direction to close
    tokio::select! {
        _ = upstream => {
            println!("Upstream closed");
        }
        _ = downstream => {
            println!("Downstream closed");
        }
        _ = close_rx.recv() => {
            println!("Connection close signaled");
        }
    }

    // Clean up the channel
    let _ = tokio::task::spawn_blocking(move || {
        if let Ok(mut ch) = channel.try_lock() {
            let _ = ch.wait_close();
        }
    }).await;

    println!("Connection handler finished");
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();
    let args = Args::parse();

    // Validate required arguments
    if args.user.is_empty() {
        return Err(anyhow!("User must be specified via --user argument or USER environment variable"));
    }
    if args.host.is_empty() {
        return Err(anyhow!("Host must be specified via --host argument or HOST environment variable"));
    }

    println!("User: {}", args.user);
    println!("Host: {}", args.host);
    println!("SSH Port: {}", args.ssh_port);
    println!("Local Port: {}", args.local_port);
    println!("Remote Port: {}", args.remote_port);

    let pkey = dirs::home_dir()
        .context("Could not determine home directory")?
        .join(".ssh/id_rsa");
    
    if !pkey.exists() {
        return Err(anyhow!("Private key not found at: {}. Please ensure your SSH key exists.", pkey.display()));
    }
    
    println!("Using private key: {}", pkey.display());

    let local = ("127.0.0.1", args.local_port);
    let remote = ("127.0.0.1", args.remote_port);

    // Create SSH tunnel
    println!("Setting up SSH tunnel...");
    let tunnel = SshTunnel::new(&args.host, args.ssh_port, &args.user, &pkey, local, remote).await?;

    println!("Connected! Try: curl http://127.0.0.1:{}/", args.local_port);
    
    // Give the tunnel a moment to stabilize
    sleep(Duration::from_millis(500)).await;
    
    let res = curl_through_tunnel(args.local_port).await;
    match res {
        Ok(body) => {
            println!("Response body:\n{}", body);
        }
        Err(e) => {
            eprintln!("Error during curl: {:#}", e);
            eprintln!("This might be expected if no service is running on the remote port {}.", args.remote_port);
        }
    }

    println!("Shutting down tunnel...");
    tunnel.shutdown().await?;
    
    println!("Program completed successfully");
    Ok(())
}

