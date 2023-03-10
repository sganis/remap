use std::process::Command;
use std::net::{TcpStream};
use byteorder::{BigEndian, ReadBytesExt};
use anyhow::Result;
use log::{debug,info,warn,error,trace};
use remap::{ClientEvent, ServerEvent, Message};
use remap::canvas::Canvas;
use remap::util;


pub fn main() -> Result<()> {
    dotenv::dotenv().expect(".env file missing");
    env_logger::init();
    let user = std::env::var("REMAP_USER").expect("REMAP_USER env var missing");
    let host = std::env::var("REMAP_HOST").expect("REMAP_HOST env var missing");
    let port: u16 = 10100;

    // make ssh connection
    let (tx,rx) = std::sync::mpsc::channel();

    // Spawn ssh tunnel thread
    std::thread::spawn(move|| {
        if util::port_is_listening(port) {
            info!("Tunnel exists, reusing...");            
            tx.send(()).expect("Could not send signal on channel.");
        } else {
            info!("Connecting...");
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
    info!("Tunnel Ok.");
    
    //  connection
    let mut stream = TcpStream::connect(&format!("127.0.0.1:{port}"))?;
    let mut writer = stream.try_clone()?;
    let mut reader = stream.try_clone()?;
    info!("Connected");
    let width = stream.read_u16::<BigEndian>()?;
    let height = stream.read_u16::<BigEndian>()?;
    info!("Geometry: {}x{}", width, height);

    let (client_tx, client_rx) = flume::unbounded();
    let (canvas_tx, canvas_rx) = flume::unbounded();

    std::thread::spawn(move || loop {        
        trace!("canvas_rx.recv()...");
        let request: ClientEvent = canvas_rx.recv().unwrap();
        debug!("writing to network {:?}...", request);
        if request.write_to(&mut writer).is_err() {
            break;
        }
    });

    std::thread::spawn(move || loop {        
        debug!("reading from network...");
        let reply = match ServerEvent::read_from(&mut reader) {
            Err(_) => {
                info!("Server disconnected");
                break;
            },
            Ok(o) => o,
        };   
        trace!("client_tx.send()...");
        client_tx.send(reply).unwrap();           
    });
 
    let mut canvas = Canvas::new(canvas_tx, client_rx)?;
    canvas.resize(width as u32, height as u32)?;

    // loop at update rate when calling canvas.update()
    while canvas.is_open() {
        canvas.handle_input()?;
        canvas.handle_server_events()?;
        canvas.update()?;
        //canvas.request_update()?;
    }
    
    canvas.close();
    Ok(())
}
