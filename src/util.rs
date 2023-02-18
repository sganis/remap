use std::net::TcpListener;
use std::process::Command;
use std::path::Path;
use log::{debug,trace};
use crate::{Geometry, Rec};

pub fn port_is_listening(port: u16) -> bool {
    match TcpListener::bind(("127.0.0.1", port)) {
        Ok(_) => false,
        Err(_) => true,
    }
}

pub fn bits_to_number(bits: &[u8]) -> u8 {
    let mut result: u8 = 0;
    bits.iter().for_each(|&bit| {
        result <<= 1;
        result ^= bit;
    });
    result
}

#[cfg(target_os = "windows")]
pub fn fix_path<P: AsRef<Path>>(p: P) -> String {
    const VERBATIM_PREFIX: &str = r#"\\?\"#;
    let p = p.as_ref().display().to_string();
    if p.starts_with(VERBATIM_PREFIX) {
        p[VERBATIM_PREFIX.len()..].to_string()
    } else {
        p
    }
}

// pub fn get_window_id(pid: u32, name: &str, display: u32) -> i32 {
//     //let cmd = format!("xdotool search --pid {pid}");
//     //println!("{}",cmd);
//     let r = Command::new("xdotool")
//         .env("DISPLAY",format!(":{display}"))
//         .arg("search")
//         .arg("--maxdepth")
//         .arg("1")        
//         .arg("--all")
//         .arg("--pid")
//         .arg(pid.to_string())
//         .arg("--name")
//         .arg(name)        
//         .output()
//         .expect("Could not run find window id command");
//     let stdout = String::from_utf8_lossy(&r.stdout).trim().to_string();
//     // let stderr = String::from_utf8_lossy(&r.stderr).trim().to_string();
//     // println!("stdout: {stdout}");
//     // println!("stderr: {stderr}");
//     let lines:Vec<String> = vec!(stdout.lines().collect());
//     match lines[lines.len()-1].parse::<i32>() {
//         Ok(xid) => xid,
//         Err(_) => 0,
//     }
// }
pub fn get_window_id(pid: u32, name: &str, display: u32) -> i32 {
    let r = Command::new("xdotool")
        .env("DISPLAY",format!(":{display}"))
        .arg("search")
        .arg("--maxdepth")
        .arg("1")  
        .arg("--pid")
        .arg(pid.to_string())       
        .arg("--name")  
        .arg("--class")  
        .arg("--classname")
        .arg("--name")  
        .arg(name)
        .output()
        .expect("Could not run find window id command");
    let stdout = std::str::from_utf8(&r.stdout).unwrap();   
    let lines: Vec<&str> = stdout.lines().collect();
    debug!("xdotool: {stdout}, {:?}", lines);
    
    if lines.len() == 0 {

        0
    } else if lines.len() == 1 {
        match lines[0].parse::<i32>() {
            Ok(xid) => xid,
            Err(_) => 0,
        }    
    } else {
        // many windows
        debug!("many windows found, getting the largest");
        for id in lines {
            let r = Command::new("xwininfo")
                .env("DISPLAY",format!(":{display}"))
                .arg("-id")
                .arg(id)
                .output()
                .expect("Could not run find window id command"); 
            let stdout = String::from_utf8_lossy(&r.stdout).trim().to_string();    
            trace!("xwininfo: {stdout}");
            //let lines2:Vec<String> = vec!(stdout.lines().collect());
            let re = regex::Regex::new(r"-geometry (\d+)x(\d+)").unwrap();
            let caps = re.captures(&stdout).unwrap();
            debug!("captures: {:?}", caps);
            if caps.get(1).unwrap().as_str().parse::<i32>().unwrap() > 10 {
                return id.parse::<i32>().unwrap();                 
            }
        }
        0
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

pub fn vec_equal(va: &[u8], vb: &[u8]) -> bool {
    va.len() == vb.len() && 
        va.iter().zip(vb).all(|(a,b)| *a==*b)
}

pub fn get_rectangles(bytes: &[u8], swidth: u16, sheight: u16) -> Vec<Rec> {
    let side = 64 as u16;  
    let xrects = swidth as u16 / side;
    let rwidth = swidth as u16 % side;
    let yrects = sheight as u16 / side;
    let rheight = sheight as u16 % side;
    let pwidth = side * xrects; // partial width without reminder
    let pheight = side * yrects; // partial height without reminder
    let mut rectangles = Vec::<Rec>::new();
    
    let mut buffer: Vec<u8> = Vec::new();
    buffer.resize(side as usize * side as usize * 4, 0);

    //let mut n = 0;
    for y in (0..pheight).step_by(side as usize) {
        for x in (0..pwidth).step_by(side as usize) {
            // println!("x={x},y={y}");
            let mut index = 0;
            for j in 0..side {
                let mut sindex = ((x as usize + ((y+j) as usize * swidth as usize)) * 4) as usize;
                for _ in 0..side {
                    buffer[index] = bytes[sindex];
                    buffer[index+1] = bytes[sindex+1];
                    buffer[index+2] = bytes[sindex+2];
                    buffer[index+3] = 255;
                    index += 4;
                    sindex += 4;
                } 
            }
            //n += 1;
            let rec = Rec {
                x: x as u16,
                y: y as u16,
                width: side as u16,
                height: side as u16,
                bytes : buffer.clone(),
            };
            rectangles.push(rec);

            //let name = format!("C:\\Users\\san\\Pictures\\Screenshots\\Result_{n}.png");
            //image::save_buffer(&name, &buffer, side, side, image::ColorType::Rgb8).unwrap();
        }
        //n += 1;
    }

    // reminder column
    //println!("rwidth: {rwidth}, rheight: {rheight}");
    //n = xrects+1;
    buffer.resize(rwidth as usize * side as usize * 4, 0);
    for y in (0..pheight).step_by(side as usize) {
        let mut index = 0;
        for j in 0..side {
            let mut sindex = ((pwidth as usize + ((y+j) as usize* swidth as usize)) * 4) as usize;
            for _ in 0..rwidth {
                //println!("sindex={sindex},y={y},index={index}");
                buffer[index] = bytes[sindex];
                buffer[index+1] = bytes[sindex+1];
                buffer[index+2] = bytes[sindex+2];
                buffer[index+3] = 255;
                index += 4;
                sindex += 4;
            } 
        }
        let rec = Rec {
            x: pwidth as u16,
            y: y as u16,
            width: rwidth as u16,
            height: side as u16,
            bytes : buffer.clone(),
        };
        rectangles.push(rec);
        //let name = format!("C:\\Users\\san\\Pictures\\Screenshots\\Result_{n}.png");
        //n += xrects+1;
        //image::save_buffer(&name, &buffer, rwidth, side, image::ColorType::Rgb8).unwrap();
    }

    // reminder row
    //n = (xrects+1) * yrects +1;
    buffer.resize(side as usize * rheight as usize * 4, 0);
    for x in (0..pwidth).step_by(side as usize) {
        let mut index = 0;
        for j in 0..rheight {
            let mut sindex = ((x as usize+ ((pheight+j) as usize * swidth as usize)) * 4) as usize;
            for _ in 0..side {
                buffer[index] = bytes[sindex];
                buffer[index+1] = bytes[sindex+1];
                buffer[index+2] = bytes[sindex+2];
                buffer[index+3] = 255;
                index += 4;
                sindex += 4;
            } 
        }
        let rec = Rec {
            x: x as u16,
            y: pheight as u16,
            width: side as u16,
            height: rheight as u16,
            bytes : buffer.clone(),
        };
        rectangles.push(rec);
        //let name = format!("C:\\Users\\san\\Pictures\\Screenshots\\Result_{n}.png");
        //n += 1;
        //image::save_buffer(&name, &buffer, side, rheight, image::ColorType::Rgb8).unwrap();
    }

    // reminder last corner
    buffer.resize(rwidth as usize * rheight as usize * 4, 0);
    let mut index = 0;
    for j in 0..rheight {
        let mut sindex = ((pwidth as usize+ ((pheight+j) as usize* swidth as usize)) * 4) as usize;
        for _ in 0..rwidth {
            buffer[index] = bytes[sindex];
            buffer[index+1] = bytes[sindex+1];
            buffer[index+2] = bytes[sindex+2];
            buffer[index+3] = 255;
            index += 4;
            sindex += 4;
    } 
    }
    //n = (xrects+1)*(yrects+1);
    let rec = Rec {
        x: pwidth as u16,
        y: pheight as u16,
        width: rwidth as u16,
        height: rheight as u16,
        bytes : buffer.clone(),
    };
    rectangles.push(rec);
    rectangles
}