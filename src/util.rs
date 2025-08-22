use std::net::TcpListener;
use std::path::Path;
use anyhow::Result;
use std::process::Command;
use crate::{Geometry, Rec};


#[cfg(target_os = "linux")]
mod platform {
    use std::env;
    use std::net::TcpListener;
    use std::path::Path;

    use anyhow::Result;
    use log::debug;
    use crate::{Geometry, Rec};
    use x11rb::connection::Connection as _;
    use x11rb::protocol::xproto::{
        Atom, ConnectionExt as _, GetGeometryReply, GetWindowAttributesReply, Window,
    };
    use x11rb::rust_connection::RustConnection;

    pub fn get_window_id(pid: u32, _needle: &str, display: u32) -> i32 {
        match find_window_bfs(pid, display) {
            Ok(Some(w)) => w as i32,
            _ => 0,
        }
    }
    fn connect_display(display: u32) -> Result<(RustConnection, Window)> {
        env::set_var("DISPLAY", format!(":{display}"));
        let (conn, screen_num) = x11rb::connect(None)?;
        let root = conn.setup().roots[screen_num].root;
        Ok((conn, root))
    }
    fn intern(conn: &RustConnection, s: &str) -> Result<Atom> {
        Ok(conn.intern_atom(true, s.as_bytes())?.reply()?.atom)
    }

    // Breadth-first search across the full window tree, choose the best candidate.
    fn find_window_bfs(target_pid: u32, display: u32) -> Result<Option<Window>> {
        let (conn, root) = connect_display(display)?;
        let NET_WM_PID = intern(&conn, "_NET_WM_PID")?;
        let WM_STATE = intern(&conn, "WM_STATE")?;
        let ATOM_CARDINAL = intern(&conn, "CARDINAL")?;
        let ATOM_INTEGER = intern(&conn, "INTEGER")?; // some WMs use INTEGER for WM_STATE

        let mut queue = vec![root];
        let mut out: Vec<(Window, u32)> = Vec::new(); // (win, area)

        while let Some(w) = queue.pop() {
            // Enqueue children
            if let Ok(tree) = conn.query_tree(w)?.reply() {
                queue.extend(tree.children);
            }

            // Read PID; many nodes will not have it
            let pid_prop = conn
                .get_property(false, w, NET_WM_PID, ATOM_CARDINAL, 0, 1)?
                .reply();
            let Some(win_pid) = pid_prop
                .ok()
                .and_then(|r| r.value32().and_then(|mut it| it.next()))
            else {
                continue;
            };
            if win_pid != target_pid {
                continue;
            }

            // Must be mapped & not override-redirect (skip tooltips/menus)
            if !is_mapped_normal(&conn, w, WM_STATE, ATOM_INTEGER)? {
                continue;
            }

            // Area > small threshold
            if let Ok(geo) = conn.get_geometry(w)?.reply() {
                let wpx = geo.width as i16 as i32;
                let hpx = geo.height as i16 as i32;
                if wpx > 10 && hpx > 10 {
                    let area = (wpx as u32) * (hpx as u32);
                    out.push((w, area));
                }
            }
        }

        // pick largest
        out.sort_by_key(|&(_, a)| a);
        Ok(out.pop().map(|(w, _)| w))
    }

    fn is_mapped_normal(
        conn: &RustConnection,
        w: Window,
        WM_STATE: Atom,
        ATOM_INTEGER: Atom,
    ) -> Result<bool> {
        // Quick check: attributes
        if let Ok(GetWindowAttributesReply {
            map_state,
            override_redirect,
            ..
        }) = conn.get_window_attributes(w)?.reply()
        {
            if override_redirect {
                return Ok(false);
            }
            // MapStateViewable == 2
            if map_state != 2.into() {
                return Ok(false);
            }
        }

        // Soft signal: presence of WM_STATE
        let _ = conn
            .get_property(false, w, WM_STATE, ATOM_INTEGER, 0, 2)?
            .reply();
        Ok(true)
    }

    // ---------------- Geometry ----------------

    pub fn get_window_geometry(xid: i32, display: u32) -> Geometry {
        match geometry_x11rb(xid as u32, display) {
            Ok(g) => g,
            Err(e) => {
                debug!("get_window_geometry error: {e:?}");
                Geometry {
                    width: 0,
                    height: 0,
                }
            }
        }
    }

    fn geometry_x11rb(xid: u32, display: u32) -> Result<Geometry> {
        let (conn, _root) = connect_display(display)?;
        // Drawable/Window are u32 type aliases; pass the id directly:
        let reply: GetGeometryReply = conn.get_geometry(xid)?.reply()?;
        Ok(Geometry {
            width: reply.width as i32,
            height: reply.height as i32,
        })
    }

    // ---------------- Optional: name/class filtered search (unused) ----------------
    // Keep if you later want name/class filtering. Currently not used by get_window_id.

    #[allow(dead_code)]
    fn get_prop_string(
        conn: &RustConnection,
        win: Window,
        prop: Atom,
        ty: Atom,
    ) -> Result<Option<String>> {
        let r = conn
            .get_property(false, win, prop, ty, 0, u32::MAX)?
            .reply()?;
        if r.value_len == 0 {
            return Ok(None);
        }
        Ok(Some(String::from_utf8_lossy(&r.value).to_string()))
    }
}

// ---------------- Misc utils (unchanged) ----------------

pub fn port_is_listening(port: u16) -> bool {
    TcpListener::bind(("127.0.0.1", port)).is_err()
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

pub fn is_display_server_running(display: u32) -> bool {
    let cmd = format!("ps aux | grep Xvfb | grep \":{display}\" >/dev/null");
    let r = Command::new("sh")
        .arg("-c")
        .arg(cmd)
        .output()
        .expect("Could not run ps command");
    r.status.code().unwrap_or(1) == 0
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
