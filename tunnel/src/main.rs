use std::process::Stdio;
use clap::Parser;
use anyhow::{anyhow, Context, Result};
use dotenvy::dotenv;

use ssh2::Session;
use std::{
    io::{Read, Write},
    path::Path,
    //process::Command,
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
    shutdown_tx: mpsc::Sender<()>,
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

            // Track active connection tasks
            let mut connection_handles = Vec::new();

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
                                
                                let handle = tokio::task::spawn(async move {
                                    if let Err(e) = handle_connection(client, session, &remote_host, remote_port).await {
                                        eprintln!("Connection handler error: {:#}", e);
                                    }
                                    println!("Connection from {} finished", peer);
                                });
                                
                                connection_handles.push(handle);
                                
                                // Clean up finished connections
                                connection_handles.retain(|h| !h.is_finished());
                            }
                            Err(e) => {
                                eprintln!("Accept error: {}", e);
                                sleep(Duration::from_millis(100)).await;
                            }
                        }
                    }
                    
                    // Handle shutdown signal
                    _ = shutdown_rx.recv() => {
                        println!("Shutdown signal received, closing listener...");
                        break;
                    }
                }
            }

            // Wait for all active connections to finish
            println!("Waiting for {} active connections to close...", connection_handles.len());
            for (i, handle) in connection_handles.into_iter().enumerate() {
                match tokio::time::timeout(Duration::from_secs(2), handle).await {
                    Ok(_) => println!("Connection {} closed", i + 1),
                    Err(_) => {
                        println!("Connection {} timed out, forcing close", i + 1);
                        // Connection handle is dropped here, which should abort the task
                    }
                }
            }

            // Explicitly close the SSH session
            println!("Closing SSH session...");
            tokio::task::spawn_blocking(move || {
                if let Ok(sess) = session.try_lock() {
                    let _ = sess.disconnect(None, "Tunnel shutdown", None);
                }
            }).await.ok();

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
            shutdown_tx,
            handle,
        })
    }

    /// Shutdown the tunnel
    pub async fn shutdown(self) -> Result<()> {
        println!("Sending shutdown signal to tunnel...");
        // Send shutdown signal
        let _ = self.shutdown_tx.send(()).await;
        
        // Wait for tunnel to shutdown with a shorter timeout
        match tokio::time::timeout(Duration::from_secs(2), self.handle).await {
            Ok(result) => {
                result??;
                println!("Tunnel shutdown completed successfully");
            }
            Err(_) => {
                println!("Tunnel shutdown timed out after 2 seconds, forcing exit");
                // Don't return error, just log and continue
            }
        }
        Ok(())
    }
}

/// Handle a single client connection
async fn handle_connection(
    client: TcpStream,
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
            let sess = session.try_lock()
                .map_err(|_| anyhow!("Failed to acquire session lock"))?;
            
            let channel = sess.channel_direct_tcpip(&remote_host, remote_port, None)
                .with_context(|| format!("channel_direct_tcpip to {}:{}", remote_host, remote_port))?;
            
            Ok(channel)
        })
        .await
        .context("Channel creation task failed")??
    };

    // Instead of using Arc<Mutex<Channel>>, we'll handle the channel in a single task
    // and use mpsc channels to communicate with it
    let (client_to_channel_tx, mut client_to_channel_rx) = mpsc::channel::<Vec<u8>>(100);
    let (channel_to_client_tx, mut channel_to_client_rx) = mpsc::channel::<Vec<u8>>(100);
    let (close_tx, mut close_rx) = mpsc::channel::<()>(1);
    let (channel_shutdown_tx, mut channel_shutdown_rx) = mpsc::channel::<()>(1);

    // Split the client stream
    let (client_reader, client_writer) = client.into_split();

    // Channel handler task - the only task that directly accesses the SSH channel
    let close_tx_channel = close_tx.clone();
    let channel_task = tokio::task::spawn_blocking(move || -> Result<()> {
        let mut channel = channel;
        let mut buffer = [0u8; 32 * 1024];
        let mut should_exit = false;
        
        while !should_exit {
            // Check for shutdown signal first
            if let Ok(_) = channel_shutdown_rx.try_recv() {
                println!("Channel handler received shutdown signal");
                should_exit = true;
                continue;
            }
            
            // Check for data to send to remote (non-blocking)
            match client_to_channel_rx.try_recv() {
                Ok(data) => {
                    if data.is_empty() {
                        // Empty data signals EOF
                        let _ = channel.send_eof();
                        let _ = channel.flush();
                        should_exit = true;
                        continue;
                    } else {
                        if let Err(e) = channel.write_all(&data) {
                            eprintln!("Failed to write to channel: {}", e);
                            should_exit = true;
                            continue;
                        }
                        if let Err(e) = channel.flush() {
                            eprintln!("Failed to flush channel: {}", e);
                            should_exit = true;
                            continue;
                        }
                    }
                }
                Err(mpsc::error::TryRecvError::Disconnected) => {
                    // Sender is dropped, exit
                    should_exit = true;
                    continue;
                }
                Err(mpsc::error::TryRecvError::Empty) => {
                    // No data available
                }
            }
            
            // Try to read from remote (non-blocking)
            match channel.read(&mut buffer) {
                Ok(0) => {
                    // EOF from remote
                    let _ = channel_to_client_tx.try_send(Vec::new());
                    should_exit = true;
                    continue;
                }
                Ok(n) => {
                    let data = buffer[..n].to_vec();
                    if channel_to_client_tx.try_send(data).is_err() {
                        // Client receiver is closed
                        should_exit = true;
                        continue;
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    // No data available, continue
                }
                Err(e) => {
                    eprintln!("Error reading from channel: {}", e);
                    should_exit = true;
                    continue;
                }
            }
            
            // Small delay to prevent busy loop and allow shutdown signal processing
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        
        let _ = close_tx_channel.try_send(());
        let _ = channel.wait_close();
        println!("Channel handler task finished");
        Ok(())
    });

    // Task 1: client -> channel (upstream)
    let close_tx_upstream = close_tx.clone();
    let upstream = tokio::task::spawn(async move {
        let mut client_reader = client_reader;
        let mut buffer = [0u8; 32 * 1024];
        loop {
            match client_reader.read(&mut buffer).await {
                Ok(0) => {
                    println!("Client closed connection (upstream)");
                    // Send EOF signal (empty vec)
                    let _ = client_to_channel_tx.send(Vec::new()).await;
                    break;
                }
                Ok(n) => {
                    let data = buffer[..n].to_vec();
                    if client_to_channel_tx.send(data).await.is_err() {
                        // Channel handler is closed
                        break;
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
    let close_tx_downstream = close_tx.clone();
    let downstream = tokio::task::spawn(async move {
        let mut client_writer = client_writer;
        loop {
            match channel_to_client_rx.recv().await {
                Some(data) => {
                    if data.is_empty() {
                        // EOF from remote
                        println!("Remote closed connection (downstream)");
                        break;
                    }
                    if let Err(e) = client_writer.write_all(&data).await {
                        eprintln!("Failed to write to client: {}", e);
                        break;
                    }
                }
                None => {
                    // Channel handler closed
                    break;
                }
            }
        }
        let _ = close_tx_downstream.send(()).await;
    });

    // Wait for any task to complete or close signal
    tokio::select! {
        result = channel_task => {
            if let Err(e) = result {
                eprintln!("Channel task panicked: {}", e);
            } else if let Err(e) = result.unwrap() {
                eprintln!("Channel task error: {}", e);
            }
            println!("Channel task finished");
        }
        _ = upstream => {
            println!("Upstream closed");
            // Signal channel handler to shutdown
            let _ = channel_shutdown_tx.send(()).await;
        }
        _ = downstream => {
            println!("Downstream closed");
            // Signal channel handler to shutdown
            let _ = channel_shutdown_tx.send(()).await;
        }
        _ = close_rx.recv() => {
            println!("Connection close signaled");
            // Signal channel handler to shutdown
            let _ = channel_shutdown_tx.send(()).await;
        }
    }

    // Give the channel task a moment to finish cleanly
    tokio::time::sleep(Duration::from_millis(100)).await;

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
