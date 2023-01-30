use std::net::TcpListener;
use std::process::Command;

pub fn port_is_listening(port: u16) -> bool {
    match TcpListener::bind(("127.0.0.1", port)) {
        Ok(_) => false,
        Err(_) => true,
    }
}

pub fn find_window_id(pid: u32, display: u32) -> i32 {
    //let cmd = format!("xdotool search --pid {pid}");
    //println!("{}",cmd);
    let r = Command::new("xdotool")
        .env("DISPLAY",format!(":{display}"))
        .arg("search")
        .arg("--maxdepth")
        .arg("1")        
        .arg("--pid")
        .arg(pid.to_string())
        .output()
        .expect("Could not run find window id command");
    let stdout = String::from_utf8_lossy(&r.stdout).trim().to_string();
    // let stderr = String::from_utf8_lossy(&r.stderr).trim().to_string();
    // println!("stdout: {stdout}");
    // println!("stderr: {stderr}");
    let lines:Vec<String> = vec!(stdout.lines().collect());
    match lines[lines.len()-1].parse::<i32>() {
        Ok(xid) => xid,
        Err(_) => 0,
    }
}

pub fn is_display_server_running(display: u32) -> bool {
    let cmd = format!("ps aux |grep Xvfb |grep \":{display}\" >/dev/null");
    let r = Command::new("sh").arg("-c").arg(cmd).output()
        .expect("Could not run ps command");
    r.status.code().unwrap() == 0
}