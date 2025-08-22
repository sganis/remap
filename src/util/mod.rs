#![allow(dead_code)]
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

use std::path::Path;
use std::process::Command;

// --- Public API re-exported per-OS (no `platform` module re-export) ---
#[cfg(target_os = "linux")]
pub use linux::{
    get_window_geometry, 
    get_window_id, 
    screen_size,
    resize_window_to,
    maximize_window,
};
//#[cfg(target_os = "macos")]
//pub use macos::{get_window_geometry, get_window_id};
//#[cfg(target_os = "windows")]
//pub use windows::{fix_path, get_window_geometry, get_window_id};


// --- Generic, cross-platform utilities live here ---
use std::net::TcpListener;
//use std::process::Command;
use crate::Rec;

/// Returns true if something is already bound to 127.0.0.1:port
pub fn port_is_listening(port: u16) -> bool {
    TcpListener::bind(("127.0.0.1", port)).is_err()
}

/// Convert a slice of bits (0/1) to a number (MSB-first).
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

// pub fn is_display_server_running(display: u32) -> bool {
//     let path = format!("/tmp/.X11-unix/X{display}");
//     std::path::Path::new(&path).exists()
// }
pub fn is_display_server_running(display: u32) -> bool {
    let cmd = format!("ps aux |grep Xvfb |grep \":{display}\" >/dev/null");
    let r = Command::new("sh").arg("-c").arg(cmd).output()
        .expect("Could not run ps command");
    r.status.code().unwrap() == 0
}

pub fn vec_equal(va: &[u8], vb: &[u8]) -> bool {
    va.len() == vb.len() && va.iter().zip(vb).all(|(a, b)| *a == *b)
}

pub fn get_rectangles(bytes: &[u8], swidth: u16, sheight: u16) -> Vec<Rec> {
    let side: u16 = 64;
    let xrects = swidth / side;
    let rwidth = swidth % side;
    let yrects = sheight / side;
    let rheight = sheight % side;
    let pwidth = side * xrects; // partial width without remainder
    let pheight = side * yrects; // partial height without remainder
    let mut rectangles = Vec::<Rec>::new();

    let mut buffer: Vec<u8> = vec![0; (side as usize) * (side as usize) * 4];

    // full tiles
    for y in (0..pheight).step_by(side as usize) {
        for x in (0..pwidth).step_by(side as usize) {
            let mut index = 0;
            for j in 0..side {
                let mut sindex =
                    ((x as usize + ((y + j) as usize * swidth as usize)) * 4) as usize;
                for _ in 0..side {
                    buffer[index] = bytes[sindex];
                    buffer[index + 1] = bytes[sindex + 1];
                    buffer[index + 2] = bytes[sindex + 2];
                    buffer[index + 3] = 255;
                    index += 4;
                    sindex += 4;
                }
            }
            let rec = Rec {
                x,
                y,
                width: side,
                height: side,
                bytes: buffer.clone(),
            };
            rectangles.push(rec);
        }
    }

    // remainder column
    if rwidth > 0 {
        buffer.resize(rwidth as usize * side as usize * 4, 0);
        for y in (0..pheight).step_by(side as usize) {
            let mut index = 0;
            for j in 0..side {
                let mut sindex =
                    ((pwidth as usize + ((y + j) as usize * swidth as usize)) * 4) as usize;
                for _ in 0..rwidth {
                    buffer[index] = bytes[sindex];
                    buffer[index + 1] = bytes[sindex + 1];
                    buffer[index + 2] = bytes[sindex + 2];
                    buffer[index + 3] = 255;
                    index += 4;
                    sindex += 4;
                }
            }
            let rec = Rec {
                x: pwidth,
                y,
                width: rwidth,
                height: side,
                bytes: buffer.clone(),
            };
            rectangles.push(rec);
        }
    }

    // remainder row
    if rheight > 0 {
        buffer.resize(side as usize * rheight as usize * 4, 0);
        for x in (0..pwidth).step_by(side as usize) {
            let mut index = 0;
            for j in 0..rheight {
                let mut sindex =
                    ((x as usize + ((pheight + j) as usize * swidth as usize)) * 4) as usize;
                for _ in 0..side {
                    buffer[index] = bytes[sindex];
                    buffer[index + 1] = bytes[sindex + 1];
                    buffer[index + 2] = bytes[sindex + 2];
                    buffer[index + 3] = 255;
                    index += 4;
                    sindex += 4;
                }
            }
            let rec = Rec {
                x,
                y: pheight,
                width: side,
                height: rheight,
                bytes: buffer.clone(),
            };
            rectangles.push(rec);
        }
    }

    // remainder corner
    if rwidth > 0 && rheight > 0 {
        buffer.resize(rwidth as usize * rheight as usize * 4, 0);
        let mut index = 0;
        for j in 0..rheight {
            let mut sindex =
                ((pwidth as usize + ((pheight + j) as usize * swidth as usize)) * 4) as usize;
            for _ in 0..rwidth {
                buffer[index] = bytes[sindex];
                buffer[index + 1] = bytes[sindex + 1];
                buffer[index + 2] = bytes[sindex + 2];
                buffer[index + 3] = 255;
                index += 4;
                sindex += 4;
            }
        }
        let rec = Rec {
            x: pwidth,
            y: pheight,
            width: rwidth,
            height: rheight,
            bytes: buffer.clone(),
        };
        rectangles.push(rec);
    }

    rectangles
}
