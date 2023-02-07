
use xcb::x::{Window, Drawable, GetImage, ImageFormat, GetGeometry};
use xcb::{Connection, XidNew};

pub struct Capture {
    connection: Connection,
    drawable: Drawable,
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
            width,
            height,
        }
    }

    pub fn get_image(&mut self) -> Vec<u8> {
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

        Vec::from(ximage.data())
    }
    
    pub fn get_geometry(&mut self) -> (u16, u16) {
        (self.width, self.height)
    }
}


