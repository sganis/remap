//use anyhow::Result;
use xcb::x::{Drawable, GetGeometry, GetImage, ImageFormat, Window};
use xcb::{Connection, XidNew};

use crate::Rec;

pub struct Capture {
    conn: Connection,
    drawable: Drawable,
    width: u16,
    height: u16,
    // previous full frame (BGRA/XRGB) so we can compare tiles cheaply
    prev_frame: Vec<u8>,
    // reusable GetImage request template
    img_req: GetImage,
    pub busy: bool,
}

impl Capture {
    /// `xid`: X window id, or 0 for the root window (desktop)
    pub fn new(xid: u32) -> Self {
        let win = unsafe { Window::new(xid) };
        let (conn, screen_index) = Connection::connect(None).expect("XCB connect failed");
        let setup = conn.get_setup();

        // Pick drawable
        let drawable = if xid == 0 {
            let screen = setup.roots().nth(screen_index as usize).expect("no screen");
            Drawable::Window(screen.root())
        } else {
            Drawable::Window(win)
        };

        // Query geometry
        let geo_cookie = GetGeometry { drawable };
        let geo = conn
            .wait_for_reply(conn.send_request(&geo_cookie))
            .expect("GetGeometry failed");
        let (width, height) = (geo.width(), geo.height());

        // Prepare a GetImage request descriptor (we'll reuse it)
        let img_req = GetImage {
            format: ImageFormat::ZPixmap,
            drawable,
            x: 0,
            y: 0,
            width,
            height,
            plane_mask: u32::MAX,
        };

        Self {
            conn,
            drawable,
            width,
            height,
            prev_frame: Vec::new(),
            img_req,
            busy: false,
        }
    }

    /// Returns (width, height)
    pub fn get_geometry(&self) -> (u16, u16) {
        (self.width, self.height)
    }

    /// Clear stored previous frame (forces next call to send a full frame’s tiles)
    pub fn clear(&mut self) {
        self.prev_frame.clear();
    }

    /// Capture the image and return:
    ///  - if `incremental == false`: all tiles (full frame)
    ///  - else: only changed tiles (tile diff vs previous frame)
    ///
    /// Tiles are fixed-size 64x64 (with remainder tiles at right/bottom edges).
    pub fn get_image(&mut self, incremental: bool) -> Vec<Rec> {
        self.busy = true;

        // In case window/root got resized, refresh geometry and request
        self.refresh_geometry_if_needed();

        // Request current frame
        let cookie = self.conn.send_request(&self.img_req);
        let reply = match self.conn.wait_for_reply(cookie) {
            Ok(r) => r,
            Err(_) => {
                self.busy = false;
                return Vec::new();
            }
        };

        let data = reply.data(); // raw server-native XRGB/BGRA bytes, 4 bytes per pixel expected
        // Ensure `prev_frame` is same length
        if self.prev_frame.len() != data.len() {
            self.prev_frame.clear();
            self.prev_frame.resize(data.len(), 0);
        }

        // Produce rectangles
        let rects = if !incremental || self.prev_frame.is_empty() {
            // Full-frame tiling
            self.tile_diff_full(data)
        } else {
            // Incremental: only tiles whose bytes differ
            self.tile_diff_changed(data)
        };

        // Update prev_frame
        self.prev_frame.copy_from_slice(data);
        self.busy = false;
        rects
    }

    /// If the drawable size changed, update width/height and the cached GetImage descriptor.
    fn refresh_geometry_if_needed(&mut self) {
        let gc = GetGeometry { drawable: self.drawable };
        if let Ok(geo) = self.conn.wait_for_reply(self.conn.send_request(&gc)) {
            let (w, h) = (geo.width(), geo.height());
            if w != self.width || h != self.height {
                self.width = w;
                self.height = h;
                self.img_req = GetImage {
                    format: ImageFormat::ZPixmap,
                    drawable: self.drawable,
                    x: 0,
                    y: 0,
                    width: self.width,
                    height: self.height,
                    plane_mask: u32::MAX,
                };
                // force full frame next time
                self.prev_frame.clear();
            }
        }
    }

    /// Return all tiles (full frame) as `Rec`s.
    fn tile_diff_full(&self, frame: &[u8]) -> Vec<Rec> {
        self.build_tiles(frame, |_tile, _prev| true)
    }

    /// Return only changed tiles by comparing `frame` vs `prev_frame`.
    fn tile_diff_changed(&self, frame: &[u8]) -> Vec<Rec> {
        self.build_tiles(frame, |tile, prev| tile != prev)
    }

    /// Build `Rec`s for tiles where `predicate(curr_tile_bytes, prev_tile_bytes)` is true.
    fn build_tiles<F>(&self, frame: &[u8], mut predicate: F) -> Vec<Rec>
    where
        F: FnMut(&[u8], &[u8]) -> bool,
    {
        const TILE: u16 = 64;
        let w = self.width as usize;
        let _h = self.height as usize;
        let stride = w * 4;

        let mut rects = Vec::new();

        let full_tiles_x = (self.width / TILE) as usize;
        let rem_w = (self.width % TILE) as usize;
        let full_tiles_y = (self.height / TILE) as usize;
        let rem_h = (self.height % TILE) as usize;

        // helper to push a tile rectangle by copying into a tight vec
        let mut push_tile = |tx: usize, ty: usize, tw: usize, th: usize| {
            // compute byte ranges for current and previous frame tile
            let mut buf = Vec::with_capacity(tw * th * 4);
            //let mut prev = Vec::with_capacity(tw * th * 4); // only allocate if we need predicate

            // To avoid double-copy, we compare line-by-line slices without allocating `prev`,
            // but we do need a contiguous `buf` for the outgoing Rec.
            // Gather current tile into `buf` and (if needed) check equality against prev_frame.
            let mut equal = true;
            for row in 0..th {
                let src_off = (ty + row) * stride + (tx * 4);
                let slice = &frame[src_off..src_off + tw * 4];
                buf.extend_from_slice(slice);

                if !self.prev_frame.is_empty() {
                    let prev_slice = &self.prev_frame[src_off..src_off + tw * 4];
                    if equal && slice != prev_slice {
                        equal = false;
                    }
                } else {
                    equal = false;
                }
            }

            if predicate(&buf, if equal { &buf } else { &[] /*unused*/ }) {
                rects.push(Rec {
                    x: tx as u16,
                    y: ty as u16,
                    width: tw as u16,
                    height: th as u16,
                    bytes: buf,
                });
            }
        };

        // full tiles grid
        for ty in (0..full_tiles_y * TILE as usize).step_by(TILE as usize) {
            for tx in (0..full_tiles_x * TILE as usize).step_by(TILE as usize) {
                push_tile(tx, ty, TILE as usize, TILE as usize);
            }
            // remainder column
            if rem_w > 0 {
                push_tile(full_tiles_x * TILE as usize, ty, rem_w, TILE as usize);
            }
        }

        // remainder row
        if rem_h > 0 {
            for tx in (0..full_tiles_x * TILE as usize).step_by(TILE as usize) {
                push_tile(tx, full_tiles_y * TILE as usize, TILE as usize, rem_h);
            }
            if rem_w > 0 {
                push_tile(full_tiles_x * TILE as usize, full_tiles_y * TILE as usize, rem_w, rem_h);
            }
        }

        rects
    }
}
