// src/util/linux.rs
use std::collections::VecDeque;
use std::env;
use std::path::Path;

use anyhow::Result;
use log::debug;

use crate::Geometry;

use x11rb::connection::Connection as _;
use x11rb::protocol::xproto::{
    Atom, ConnectionExt as _, GetGeometryReply, GetWindowAttributesReply, Window,
};
use x11rb::rust_connection::RustConnection;
use x11rb::NONE; // <-- use NONE (0) to mean "AnyPropertyType"

// ---------- Public API ----------

/// Return the best window id for `pid` on `:{display}` (0 if not found).
pub fn get_window_id(pid: u32, _needle: &str, display: u32) -> i32 {
    match find_window_bfs(pid, display) {
        Ok(Some(w)) => {
            debug!("get_window_id: selected window=0x{w:08x} for pid={pid}");
            w as i32
        }
        Ok(None) => {
            debug!("get_window_id: no window found for pid={pid}");
            0
        }
        Err(e) => {
            debug!("get_window_id: error: {e:?}");
            0
        }
    }
}

/// Return the window geometry for `xid` on `:{display}` (0x0 on error).
pub fn get_window_geometry(xid: i32, display: u32) -> Geometry {
    match geometry_x11rb(xid as u32, display) {
        Ok(g) => g,
        Err(e) => {
            debug!("get_window_geometry error: {e:?}");
            Geometry { width: 0, height: 0 }
        }
    }
}

/// No-op on Linux; returned as a String for cross-platform call sites.
pub fn fix_path<P: AsRef<Path>>(p: P) -> String {
    p.as_ref().display().to_string()
}

// ---------- Internals ----------

fn connect_display(display: u32) -> Result<(RustConnection, Window)> {
    let d = format!(":{display}");
    env::set_var("DISPLAY", &d);
    debug!("connect_display: DISPLAY={d}");
    let (conn, screen_num) = x11rb::connect(None)?;
    let root = conn.setup().roots[screen_num].root;
    debug!("connect_display: screen={screen_num}, root=0x{root:08x}");
    Ok((conn, root))
}

fn intern(conn: &RustConnection, s: &str) -> Result<Atom> {
    // IMPORTANT: only_if_exists = false — ensures a valid (non-zero) atom id
    let a = conn.intern_atom(false, s.as_bytes())?.reply()?.atom;
    debug!("intern: {s} -> {a}");
    Ok(a)
}

/// Safe helper: read a single u32 property value (e.g., _NET_WM_PID).
/// Uses NONE (0) as "AnyPropertyType" to avoid type-mismatch errors.
fn get_prop_u32(conn: &RustConnection, win: Window, prop: Atom) -> Result<Option<u32>> {
    let r = conn.get_property(false, win, prop, NONE, 0, 1)?.reply()?;
    let val = r.value32().and_then(|mut it| it.next());
    Ok(val)
}

/// Compute area and (w,h); returns None if tiny or on error.
fn window_area(conn: &RustConnection, w: Window) -> Option<(u32, i32, i32)> {
    let reply: GetGeometryReply = conn.get_geometry(w).ok()?.reply().ok()?;
    let wpx = reply.width as i16 as i32;
    let hpx = reply.height as i16 as i32;
    if wpx > 10 && hpx > 10 {
        Some(((wpx as u32) * (hpx as u32), wpx, hpx))
    } else {
        None
    }
}

/// Breadth-first search; choose the largest mapped window that matches `target_pid`.
/// Follows WM_CLIENT_LEADER → _NET_WM_PID for clients that don’t expose PID on the leaf.
fn find_window_bfs(target_pid: u32, display: u32) -> Result<Option<Window>> {
    let (conn, root) = connect_display(display)?;

    // Common atoms (created if missing)
    let NET_WM_PID = intern(&conn, "_NET_WM_PID")?;
    let WM_STATE = intern(&conn, "WM_STATE")?;
    let WM_CLIENT_LEADER = intern(&conn, "WM_CLIENT_LEADER")?;

    let mut queue = VecDeque::from([root]);
    let mut best: Option<(Window, u32)> = None; // (win, area)

    while let Some(w) = queue.pop_front() {
        // Enqueue children (true BFS)
        if let Ok(tree) = conn.query_tree(w)?.reply() {
            for ch in &tree.children {
                queue.push_back(*ch);
            }
        }

        // PID directly on this window?
        let pid_here = get_prop_u32(&conn, w, NET_WM_PID)?;
        debug!("scan: win=0x{w:08x}, pid_here={pid_here:?}");

        // If missing, try WM_CLIENT_LEADER → _NET_WM_PID
        let mut pid_match = false;
        let mut leader_win: Option<Window> = None;

        if let Some(pid) = pid_here {
            pid_match = pid == target_pid;
        } else if let Some(lw) = get_prop_u32(&conn, w, WM_CLIENT_LEADER)? {
            let lw = lw as Window;
            leader_win = Some(lw);
            let pid_leader = get_prop_u32(&conn, lw, NET_WM_PID)?;
            debug!("scan: win=0x{w:08x} no pid; leader=0x{lw:08x} pid_leader={pid_leader:?}");
            if let Some(pid) = pid_leader {
                pid_match = pid == target_pid;
            }
        }

        if !pid_match {
            continue;
        }

        // Must be viewable & not override-redirect (skip tooltips/menus)
        if !is_mapped_normal(&conn, w, WM_STATE)? {
            debug!("skip: win=0x{w:08x} not mapped/normal");
            continue;
        }

        // Prefer the largest area
        if let Some((area, wpx, hpx)) = window_area(&conn, w) {
            debug!(
                "candidate: win=0x{w:08x}{} area={area} size={}x{}",
                leader_win
                    .map(|lw| format!(" leader=0x{lw:08x}"))
                    .unwrap_or_default(),
                wpx,
                hpx
            );
            match best {
                None => best = Some((w, area)),
                Some((_, a)) if area > a => best = Some((w, area)),
                _ => {}
            }
        } else {
            debug!("skip: win=0x{w:08x} tiny or no geometry");
        }
    }

    if let Some((w, a)) = best {
        debug!("find_window_bfs: chosen win=0x{w:08x} area={a}");
        Ok(Some(w))
    } else {
        debug!("find_window_bfs: no match for pid={target_pid}");
        Ok(None)
    }
}

fn is_mapped_normal(conn: &RustConnection, w: Window, WM_STATE: Atom) -> Result<bool> {
    // Quick check: attributes
    if let Ok(GetWindowAttributesReply {
        map_state,
        override_redirect,
        ..
    }) = conn.get_window_attributes(w)?.reply()
    {
        let mapped = map_state == 2u8.into(); // MapStateViewable == 2
        if override_redirect {
            debug!("is_mapped_normal: win=0x{w:08x} override_redirect=true");
            return Ok(false);
        }
        if !mapped {
            debug!("is_mapped_normal: win=0x{w:08x} not viewable");
            return Ok(false);
        }
    }

    // Soft signal: presence of WM_STATE; use NONE (AnyPropertyType) to avoid type issues
    let _ = conn.get_property(false, w, WM_STATE, NONE, 0, 2)?.reply();
    Ok(true)
}

fn geometry_x11rb(xid: u32, display: u32) -> Result<Geometry> {
    let (conn, _root) = connect_display(display)?;
    let reply: GetGeometryReply = conn.get_geometry(xid)?.reply()?;
    Ok(Geometry {
        width: reply.width as i32,
        height: reply.height as i32,
    })
}

#[allow(dead_code)]
fn get_prop_string(conn: &RustConnection, win: Window, prop: Atom) -> Result<Option<String>> {
    let r = conn.get_property(false, win, prop, NONE, 0, u32::MAX)?.reply()?;
    if r.value_len == 0 {
        return Ok(None);
    }
    Ok(Some(String::from_utf8_lossy(&r.value).to_string()))
}
