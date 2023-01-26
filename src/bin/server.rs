use std::error::Error;
use std::io::{Read,Write};
use std::net::TcpListener;
use std::process::{Command, Stdio};
use enigo::{Enigo, Key, KeyboardControllable};

// #[path = "../command.rs"]
// mod command;


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

fn is_display_server_running(display: u32) -> bool {
    let cmd = "ps aux |grep Xvfb |grep \":{display}\" >/dev/null";
    let r = Command::new("sh").arg("-c").arg(cmd).output().expect("Could not run ps command");
    r.status.code().unwrap() == 0
}

fn find_window_id(pid: u32, display: u32) -> i32 {
    //let cmd = format!("xdotool search --pid {pid}");
    //println!("{}",cmd);
    let r = Command::new("xdotool")
        .env("DISPLAY",format!(":{display}"))
        .arg("search")
        .arg("--pid")
        .arg(pid.to_string())
        .output()
        .expect("Could not run find window id command");
    let stdout = String::from_utf8_lossy(&r.stdout).trim().to_string();
    // let stderr = String::from_utf8_lossy(&r.stderr).trim().to_string();
    // println!("stdout: {stdout}");
    // println!("stderr: {stderr}");
    let lines:Vec<String> = vec!(stdout.lines().collect());
    match lines[0].parse::<i32>() {
        Ok(xid) => xid,
        Err(_) => 0,
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    // get params
    let display = std::env::args().nth(1)
        .unwrap_or_else(|| "101".to_string())
        .parse::<u32>().unwrap();
    let app = std::env::args().nth(2)
        .unwrap_or_else(|| "xterm".to_string());
    let port = 7001;
    let port2 = 7002;
    let input_addr = format!("127.0.0.1:{port2}");

    std::env::set_var("DISPLAY",&format!(":{display}"));

    // run display_server
    let r = Command::new("Xvfb")
        .env("DISPLAY",&format!(":{display}"))
        .args(["+extension","GLX","+extension","Composite","-screen","0",
            "8192x4096x24+32","-nolisten","tcp","-noreset",
            "-auth","/run/user/1000/gdm/Xauthority","-dpi","96",
            &format!(":{display}")])
        .spawn()
        .expect("display failed to start");
    println!("display pid: {}", r.id());
    
    // wait for it
    while !is_display_server_running(display) {
        println!("Waiging display...");
        std::thread::sleep(std::time::Duration::from_millis(200));
    }    
    
    // // run app and get pid
    let r = Command::new(app)
        .env("DISPLAY",&format!(":{display}"))
        .spawn()
        .expect("Could not run app");
    println!("app pid: {}", r.id());
    let pid = r.id();
    
    // find window ID,. wait for it
    let mut xid = find_window_id(pid, display);   
    while xid == 0 {
        println!("Waiting window id...");
        std::thread::sleep(std::time::Duration::from_millis(200));
        xid = find_window_id(pid, display);
    } 
    println!("window xid: {} ({:#06x})", xid, xid);
        
    // run video server
    let r = Command::new("gst-launch-1.0")
        .env("DISPLAY",&format!(":{display}"))
        .args(["ximagesrc",&format!("xid={xid}"),"use-damage=0",
            "!","queue",
            "!","videoconvert",
            "!","video/x-raw,framerate=24/1",
            "!","jpegenc",
            "!","multipartmux",
            "!","tcpserversink","host=127.0.0.1","port=7001"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("video stream failed to start");
    println!("video pid: {}", r.id());

    let listener = TcpListener::bind(&input_addr)?;
    println!("Listening on: {}", input_addr);

    loop {
        let (mut socket, _) = listener.accept()?;
        println!("Connected to client");
        
        //let mut keyboard = Keyboard::new();
        //let xid = keyboard.search_window_by_pid(pid);
        //println!("window xid: {}", xid); 
        //let window : i32 = 2097165; // hardcoded
    
        let mut keyboard = Keyboard::new();
        keyboard.set_window(xid);
        keyboard.focus();
        let pid = keyboard.get_window_pid();
        println!("window pid: {}", pid);
        
        loop {
            let mut buf = vec![0; 12];            
            let n = socket.read(&mut buf).expect("failed to read data from socket");
            let c = String::from_utf8_lossy(&buf);
            print!(" key recieved: {:?}", c);
            let c = c.trim_matches(char::from(0));            
            std::io::stdout().flush().unwrap();
            assert!(!c.is_empty());
            
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


