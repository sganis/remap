
use xcb::x::{Window, Drawable, GetImage, ImageFormat, GetGeometry};
use xcb::{Connection, XidNew};
use crate::util;
use crate::{Rec};

pub struct Capture {
    connection: Connection,
    drawable: Drawable,
    pub rectangles: Vec<Rec>,
    width: u16,
    height: u16,
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
        
        Self {
            connection,
            drawable,
            rectangles: vec![],
            width,
            height,
        }
    }

    pub fn get_image(&mut self, incremental: bool) -> Vec<Rec> {
        let cookie = GetImage {
            format: ImageFormat::ZPixmap, 
            drawable: self.drawable, 
            x: 0, y: 0, 
            width: self.width, height: self.height, 
            plane_mask: u32::MAX,
        };
        let request = self.connection.send_request(&cookie);
        let ximage = self.connection.wait_for_reply(request).unwrap();

        // // BGRA to RGBA
        // let mut bytes = ximage.data();
        // for i in (0..bytes.len()).step_by(4) {
        //     let b = bytes[i];
        //     let r = bytes[i + 2];      
        //     bytes[i] = r;
        //     bytes[i + 2] = b;
        //     bytes[i + 3] = 255;
        // }
        //println!("image captrued");

        // save buffer
        let new_rectangles = util::get_rectangles(&ximage.data(), self.width, self.height);
        
        if !incremental || self.rectangles.len() == 0 {
            println!("full image, rectangles send: {}", new_rectangles.len());                         
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
            println!("diff rectangles send: {}", different_rectangles.len());                      
            different_rectangles
        }
    }
    
    pub fn get_geometry(&mut self) -> (u16, u16) {
        (self.width, self.height)
    }
    pub fn clear(&mut self) {
        self.rectangles = vec![];
    }
    pub fn get_rectangles(&mut self) {
        self.rectangles = vec![];
    }
}


