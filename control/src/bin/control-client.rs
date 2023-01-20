use std::io::{stdout, Read, Write};
use crossterm::event::{read, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::style::Print;
use crossterm::ExecutableCommand;
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use std::net::TcpStream;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {

    enable_raw_mode()?;
    println!("Ctrl+C to exit");

    match TcpStream::connect("localhost:7003") {
        Ok(mut stream) => {
            println!("Connected to port 7003");

            loop {
                match read()? {
                    Event::Key(KeyEvent {
                        code: KeyCode::Char('c'),
                        modifiers: KeyModifiers::CONTROL,
                    }) => break, 
                    Event::Key(e) => {
                        match e.code {
                            KeyCode::Char(c) => {
                                stdout().execute(Print(c))?;
                                stream.write(c.to_string().as_bytes())?;
                                stream.flush()?;
                            },
                            KeyCode::Enter => {
                                write!(stdout(), "\n")?;
                                stream.write(b"\n")?;
                                let mut data = [0; 1]; // using 6 byte buffer
                                match stream.read(&mut data) {
                                    Ok(_) => {
                                        let c = String::from_utf8_lossy(&data[..]);
                                        if c == "K" {
                                            println!("ok");
                                        } else {
                                            println!("Unexpected reply: {}", c);
                                        }
                                    },
                                    Err(e) => {
                                        println!("Failed to receive data: {}", e);
                                    }
                                }
                            },                    
                            e => {
                                println!("key: {:?}", e);
                            }                    
                        }
                    },
                    Event::Mouse(e) => {
                        println!("mouse: {:?}", e);
                    },
                    Event::Resize(w,h) => {
                        println!("resize: w: {}, h: {}", w, h);
                    }
                    //e => println!("{:?}", e)
                }
            }
        },
        Err(e) => {
            println!("Failed to connect: {}", e);
        }
    }
    
    disable_raw_mode().unwrap();
    Ok(())
}