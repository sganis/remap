use std::error::Error;
use std::io::Write;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use enigo::{Enigo, Key, KeyboardControllable};

//#[derive(Default)]
struct Keyboard {
    enigo: Option<Enigo>,
}

//#[allow(dead_code)]
impl Keyboard {
    pub fn new(window: i32) -> Self {
        let mut e = Enigo::new();
        e.set_window(window);
        Self { 
            enigo: Some(e),
        }
    }
    pub fn focus(&mut self) {
        self.enigo.as_mut().unwrap().window_focus();
    }
    pub fn get_window_pid(&mut self) -> i32 {
        self.enigo.as_mut().unwrap().window_pid()
    }
    pub fn search_window_by_pid(&mut self, pid: i32) -> i32 {
        self.enigo.as_mut().unwrap().search_window_by_pid(pid)
    }
    pub fn key(&mut self, key: &str) {
        let k = match key {
            "\n" => Key::Return,
            "Backspace" => Key::Backspace,
            "Delete" => Key::Delete,
            "Down" => Key::DownArrow,
            "End" => Key::End,
            "Esc" => Key::Escape,
            "Home" => Key::Home,
            "Left" => Key::LeftArrow,
            "PageDown" => Key::PageDown,
            "PageUp" => Key::PageUp,
            "Right" => Key::RightArrow,
            "Tab" => Key::Tab,
            "Up" => Key::UpArrow,
            c => Key::Layout(c.chars().next().unwrap()),
        };
        println!(" key detected: {:?}", k);
        self.enigo.as_mut().unwrap().key_click(k);
    }
}


#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let addr = std::env::args().nth(1)
        .unwrap_or_else(|| "127.0.0.1:7002".to_string());

    let listener = TcpListener::bind(&addr).await?;
    println!("Listening on: {}", addr);

    loop {
        let (mut socket, _) = listener.accept().await?;
        println!("Connected to client");
        let window : i32 = 2097165; // hardcoded
        let mut keyboard = Keyboard::new(window);
        keyboard.focus();
        let pid = keyboard.get_window_pid();
        println!("window pid: {}", pid);
        //let xid = keyboard.search_window_by_pid(pid);
        //println!("window xid: {}", xid);
        

        tokio::spawn(async move {
            loop {
                let mut buf = vec![0; 32];            
                let n = socket.read(&mut buf).await.expect("failed to read data from socket");
                let c = String::from_utf8_lossy(&buf);
                let c = c.trim_matches(char::from(0));
                print!(" key recieved: {:?}", c);
                std::io::stdout().flush().unwrap();

                // send key to window
                keyboard.key(&c);


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


