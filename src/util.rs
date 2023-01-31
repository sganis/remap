use std::net::TcpListener;
use std::process::Command;
use crate::Geometry;

pub fn port_is_listening(port: u16) -> bool {
    match TcpListener::bind(("127.0.0.1", port)) {
        Ok(_) => false,
        Err(_) => true,
    }
}

pub fn get_window_id(pid: u32, display: u32) -> i32 {
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

pub fn get_window_geometry(xid: i32, display: u32) -> Geometry {
    let r = Command::new("xdotool")
        .env("DISPLAY",format!(":{display}"))
        .arg("getwindowgeometry")
        .arg(xid.to_string())
        .output()
        .expect("Could not run get window geometry command");
    let stdout = String::from_utf8_lossy(&r.stdout).trim().to_string();
    let lines:Vec<String> = vec!(stdout.lines().filter(|&l| l.contains("Geometry:")).collect());
    let geometry = lines[0].rsplit_once(':').unwrap().1.trim();
    let (w,h) = geometry.split_once('x').unwrap();
    //println!("geometry: {} {}", width, height);
    let width = w.parse::<i32>().expect("Could not parse width geometry");
    let height = h.parse::<i32>().expect("Could not parse height geometry");
    Geometry { width, height }
}
