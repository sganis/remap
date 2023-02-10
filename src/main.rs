use std::process::Command;
use tokio::net::TcpStream;
use tokio::io::AsyncReadExt;
use std::time::Instant;
use anyhow::Result;
use remap::canvas::Canvas;
use remap::util;
use remap::client::Client;

#[tokio::main]
async fn main() -> Result<()> {
    
    dotenv::dotenv().expect(".env file missing");
    let user = std::env::var("REMAP_USER").expect("REMAP_USER env var missing");
    let host = std::env::var("REMAP_HOST").expect("REMAP_HOST env var missing");
    let port: u16 = 10100;

    // make ssh connection
    let (tx,rx) = std::sync::mpsc::channel();

    // Spawn ssh tunnel thread
    std::thread::spawn(move|| {
        if util::port_is_listening(port) {
            println!("Tunnel exists, reusing...");            
            tx.send(()).expect("Could not send signal on channel.");
        } else {
            println!("Connecting...");
            let _handle = Command::new("ssh")
                .args(["-oStrictHostkeyChecking=no","-N","-L", 
                    &format!("{port}:127.0.0.1:{port}"),
                    &format!("{user}@{host}")])
                .spawn().unwrap();
            while !util::port_is_listening(port) {
                std::thread::sleep(std::time::Duration::from_millis(200));
            }
            tx.send(()).expect("Could not send signal on channel.");
        }
    });
    
    // wait for signal
    rx.recv().expect("Could not receive from channel.");
    println!("Tunnel Ok.");
    
    //  connection
    let mut stream = TcpStream::connect(&format!("127.0.0.1:{port}"))?;
    let writer = stream.try_clone()?;
    let reader = stream.try_clone()?;
    println!("Connected");
    let width = stream.read_u16().await?;
    let height = stream.read_u16().await?;
    println!("Geometry: {}x{}", width, height);

    let (client_tx, client_rx) = std::sync::mpsc::channel();
    let (canvas_tx, canvas_rx) = std::sync::mpsc::channel();

    std::thread::spawn(move || {        
        let mut writer = writer;
        loop {
            let request: ClientEvent = canvas_rx.recv().unwrap();
            request.write_to(&mut writer).unwrap(); 
        }
    });

    std::thread::spawn(move || {        
        let mut reader = reader;
        loop {
            let reply = match ServerEvent::read_from(&mut reader) {
                Err(e) => {
                    println!("Server disconnected: {:?}", e);
                    break;
                },
                Ok(o) => o,
            };   
            client_tx.send(reply).unwrap();        
        }
    });
 
    let mut canvas = Canvas::new(canvas_tx, client_rx)?;
    canvas.resize(width as u32, height as u32)?;

    let mut frames = 0;
    let mut start = Instant::now();

    // loop at update rate
    while canvas.is_open() {
        canvas.handle_input().await?;
        canvas.handle_server_events().await?;
        canvas.update().await?;
        canvas.request_update().await?;

        frames += 1;
        if start.elapsed().as_secs() >= 1 {
            println!("{:.0}", frames as f64 / start.elapsed().as_millis() as f64 * 1000.0);
            start = Instant::now();
            frames = 0;
        }
    }

    canvas.close();
    Ok(())

}
