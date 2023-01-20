use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use std::env;
use std::error::Error;
use std::io::Write;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let addr = env::args().nth(1)
        .unwrap_or_else(|| "127.0.0.1:7003".to_string());

    let listener = TcpListener::bind(&addr).await?;
    println!("Listening on: {}", addr);

    loop {
        let (mut socket, _) = listener.accept().await?;
        tokio::spawn(async move {
            let mut buf = vec![0; 1];
            loop {
                let n = socket.read(&mut buf).await.expect("failed to read data from socket");
                let c = String::from_utf8_lossy(&buf);
                print!("{}", c);
                std::io::stdout().flush().unwrap();

                if n == 0 {
                    return;
                }
                if c == "\n" {
                    socket.write(b"K").await.expect("failed to write data to socket");
                }
            }
        });
    }
}


