use xcb::x::{Window, Drawable, GetImage, ImageFormat, GetGeometry};
use xcb::{Connection, XidNew};
use anyhow::Result;
use std::time::Instant;
use crate::util;
use crate::Rec;

pub struct Capture {
    connection: Connection,
    drawable: Drawable,
    pub rectangles: Vec<Rec>,
    width: u16,
    height: u16,
    cookie: GetImage,
    pub busy: bool,
}

impl Capture {
    // xid: windows id, 0 if desktop
    pub fn new(xid: u32) -> Self {
        let window = unsafe {Window::new(xid)};
        let (connection, index) = Connection::connect(None).unwrap();
        let setup = connection.get_setup();
        let drawable = if xid == 0 {
            let screen = setup.roots().nth(index as usize).unwrap();
            Drawable::Window(screen.root())
        } else {
            Drawable::Window(window)
        };
        
        let cookie = GetGeometry { drawable };
        let request = connection.send_request(&cookie);
        let reply = connection.wait_for_reply(request).unwrap();
        let (width, height) = (reply.width(), reply.height());
        println!("Geometry xcb: {}x{}", width, height);
        
        let cookie = GetImage {
            format: ImageFormat::ZPixmap, 
            drawable, 
            x: 0, y: 0, width, height, 
            plane_mask: u32::MAX,
        };

        Self {
            connection,
            drawable,
            rectangles: vec![],
            width,
            height,
            cookie,
            busy: false,
        }
    }

    pub fn get_image(&mut self, incremental: bool) -> Vec<Rec> {
        self.busy = true;
        if !incremental {
            self.clear();
        }        
        let request = self.connection.send_request(&self.cookie);
        let ximage = self.connection.wait_for_reply(request).unwrap();

        // save buffer
        let new_rectangles = util::get_rectangles(&ximage.data(), self.width, self.height);
        
        if !incremental || self.rectangles.len() == 0 {
            //println!("full image, rectangles send: {}", new_rectangles.len());                         
            self.rectangles = new_rectangles.clone();
            new_rectangles
        } else {
            // return incremental rectangle
            let mut different_rectangles = vec![];            
            for a in 0..self.rectangles.len()-1 {
                let ra = &self.rectangles[a];
                let rb = &new_rectangles[a];
                if !util::vec_equal(&ra.bytes, &rb.bytes) {
                    let rec = Rec {
                        x: rb.x,
                        y: rb.y,
                        width: rb.width,
                        height: rb.height,
                        bytes: rb.bytes.clone(),
                    };            
                    different_rectangles.push(rec);            
                }
            }   
            self.rectangles = new_rectangles.clone();
            //println!("diff rectangles send: {}", different_rectangles.len());                      
            self.busy = false;
            different_rectangles
        }
    }
    
    pub fn get_geometry(&mut self) -> (u16, u16) {
        (self.width, self.height)
    }
    pub fn clear(&mut self) {
        self.rectangles = vec![];
    }

    
}


