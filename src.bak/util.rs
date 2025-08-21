use std::net::TcpListener;
use std::process::Command;
use log::{debug, trace};
use crate::{Geometry, Rec};

pub fn port_is_listening(port: u16) -> bool {
    TcpListener::bind(("127.0.0.1", port)).is_err()
}

pub fn vec_equal(va: &[u8], vb: &[u8]) -> bool {
    va.len() == vb.len() && va.iter().zip(vb).all(|(a,b)| *a==*b)
}

// splits raw framebuffer into 64x64 rects
pub fn get_rectangles(bytes: &[u8], swidth: u16, sheight: u16) -> Vec<Rec> {
    let side: u16 = 64;
    let mut rects = Vec::new();
    let stride = swidth as usize * 4;
    for y in (0..sheight).step_by(side as usize) {
        for x in (0..swidth).step_by(side as usize) {
            let w = side.min(swidth - x);
            let h = side.min(sheight - y);
            let mut buf = vec![0; (w*h*4) as usize];
            let mut dst = 0;
            for row in 0..h {
                let src = ((y+row) as usize * stride) + (x as usize * 4);
                buf[dst..dst+(w as usize*4)]
                    .copy_from_slice(&bytes[src..src+(w as usize*4)]);
                dst += w as usize * 4;
            }
            rects.push(Rec { x, y, width: w, height: h, bytes: buf });
        }
    }
    rects
}
