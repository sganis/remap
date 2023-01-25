use std::error::Error;
use std::io::{Read,Write};
use std::net::TcpListener;
use enigo::{Enigo, Key, KeyboardControllable};

//#[derive(Default)]
struct Keyboard {
    enigo: Option<Enigo>,
}

//#[allow(dead_code)]
impl Keyboard {
    pub fn new() -> Self {
        Self { 
            enigo: Some(Enigo::new()),
        }
    }
    pub fn focus(&mut self) {
        self.enigo.as_mut().unwrap().window_focus();
    }
    pub fn set_window(&mut self, window: i32) {
        self.enigo.as_mut().unwrap().set_window(window);
    }
    pub fn get_window_pid(&mut self) -> i32 {
        self.enigo.as_mut().unwrap().window_pid()
    }
    pub fn search_window_by_pid(&mut self, pid: i32) -> i32 {
        self.enigo.as_mut().unwrap().search_window_by_pid(pid)
    }
    pub fn key(&mut self, key: &str) {
        println!(" key to match: {:?}", key);
        let k = match key {
            "Return" => Key::Return,
            "BackSpace" => Key::Backspace,
            "Delete" => Key::Delete,
            "Page_Up" => Key::PageUp,
            "Page_Down" => Key::PageDown,
            "Up" => Key::UpArrow,
            "Down" => Key::DownArrow,
            "Left" => Key::LeftArrow,
            "Right" => Key::RightArrow,
            "End" => Key::End,
            "Home" => Key::Home,
            "Tab" => Key::Tab,
            "Escape" => Key::Escape,
            c => Key::Layout(c.chars().next().unwrap()),
        };
        println!(" key detected: {:?}", k);
        self.enigo.as_mut().unwrap().key_click(k);
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let addr = std::env::args().nth(1)
        .unwrap_or_else(|| "127.0.0.1:7002".to_string());


    


    let listener = TcpListener::bind(&addr)?;
    println!("Listening on: {}", addr);

    loop {
        let (mut socket, _) = listener.accept()?;
        println!("Connected to client");
        
        let pid = 16007;
        let mut keyboard = Keyboard::new();
        let xid = keyboard.search_window_by_pid(pid);
        println!("window xid: {}", xid);
        
        let window : i32 = 2097165; // hardcoded
    
        let mut keyboard = Keyboard::new();
        keyboard.set_window(window);
        keyboard.focus();
        let pid = keyboard.get_window_pid();
        println!("window pid: {}", pid);
        
        loop {
            let mut buf = vec![0; 32];            
            let n = socket.read(&mut buf).expect("failed to read data from socket");
            let c = String::from_utf8_lossy(&buf);
            let c = c.trim_matches(char::from(0));
            print!(" key recieved: {:?}", c);
            std::io::stdout().flush().unwrap();

            // send key to window
            keyboard.key(&c);


            if n == 0 {
                break;
            }
            if c == "Return" {
                socket.write(b"OK").expect("failed to write data to socket");
            }
        }
        break
    }
    Ok(())
}




// #[tokio::main]
// async fn main() -> Result<(), Box<dyn Error>> {
//     let addr = std::env::args().nth(1)
//         .unwrap_or_else(|| "127.0.0.1:7002".to_string());

//     let listener = TcpListener::bind(&addr).await?;
//     println!("Listening on: {}", addr);

//     loop {
//         let (mut socket, _) = listener.accept().await?;
//         println!("Connected to client");
        
//         let pid = 16007;
//         let mut keyboard = Keyboard::new();
//         let xid = keyboard.search_window_by_pid(pid);
//         println!("window xid: {}", xid);
        
//         let window : i32 = 2097165; // hardcoded
    
//         let mut keyboard = Keyboard::new();
//         keyboard.set_window(window);
//         keyboard.focus();
//         let pid = keyboard.get_window_pid();
//         println!("window pid: {}", pid);
        

//         tokio::spawn(async move {
//             loop {
//                 let mut buf = vec![0; 32];            
//                 let n = socket.read(&mut buf).await.expect("failed to read data from socket");
//                 let c = String::from_utf8_lossy(&buf);
//                 let c = c.trim_matches(char::from(0));
//                 print!(" key recieved: {:?}", c);
//                 std::io::stdout().flush().unwrap();

//                 // send key to window
//                 keyboard.key(&c);


//                 if n == 0 {
//                     return;
//                 }
//                 if c == "Return" {
//                     socket.write(b"OK").await.expect("failed to write data to socket");
//                 }
//             }
//         });
//     }
// }


