#[cfg(unix)]
use enigo::{Enigo, MouseButton as EnigoMouseButton, MouseControllable, Key, KeyboardControllable};
use std::collections::HashMap;
use crate::Geometry;

#[cfg(unix)]
pub struct Input {
    enigo: Option<Enigo>,
    server_geometry: Geometry,
    client_geometry: Geometry,
    keymap: HashMap<u8, Key>,
}

#[cfg(unix)]
#[allow(dead_code)]
impl Input {
    pub fn new() -> Self {
        let keymap = HashMap::from([
            (10, Key::Layout('a')),
            (11, Key::Layout('b')),
            (12, Key::Layout('c')),
            (13, Key::Layout('d')),
            (14, Key::Layout('e')),
            (15, Key::Layout('f')),
            (16, Key::Layout('g')),
            (17, Key::Layout('h')),
            (18, Key::Layout('i')),
            (19, Key::Layout('j')),
            (20, Key::Layout('k')),
            (21, Key::Layout('l')),
            (22, Key::Layout('m')),
            (23, Key::Layout('m')),
            (24, Key::Layout('o')),
            (25, Key::Layout('p')),
            (26, Key::Layout('q')),
            (27, Key::Layout('r')),
            (28, Key::Layout('s')),
            (29, Key::Layout('t')),
            (30, Key::Layout('u')),
            (31, Key::Layout('v')),
            (32, Key::Layout('w')),
            (33, Key::Layout('x')),
            (34, Key::Layout('y')),
            (35, Key::Layout('z')),
            (51, Key::DownArrow),
            (52, Key::LeftArrow),
            (53, Key::RightArrow),
            (54, Key::UpArrow),
            (61, Key::Layout('-')),
            (66, Key::Backspace),
            (67, Key::Delete),
            (68, Key::End),
            (69, Key::Return),
            (70, Key::Escape),
            (71, Key::Home),
            (74, Key::PageDown),
            (75, Key::PageUp),
            (77, Key::Space),
            (78, Key::Tab),
        ]);
        Self { 
            enigo: Some(Enigo::new()),
            server_geometry : Geometry::default(),
            client_geometry : Geometry::default(),
            keymap,
        }
    }
    pub fn set_server_geometry(&mut self, geometry: Geometry) {
        self.server_geometry = geometry;
        println!("enigo: set window size");
        self.enigo.as_mut().unwrap().set_window_size(geometry.width, geometry.height);
    }
    pub fn set_client_geometry(&mut self, geometry: Geometry) {
        self.client_geometry = geometry;
    }
    pub fn focus(&mut self) {
        self.enigo.as_mut().unwrap().window_focus();
    }
    pub fn set_window(&mut self, window: i32) {
        self.enigo.as_mut().unwrap().set_window(window);
    }
    pub fn get_window_pid(&mut self) -> i32 {
        self.enigo.as_mut().unwrap().window_pid()
    }
    pub fn search_window_by_pid(&mut self, pid: i32) -> i32 {
        self.enigo.as_mut().unwrap().search_window_by_pid(pid)
    }
    pub fn mouse_click(&mut self, x: i32, y:i32, button: u32, modifiers: u32) {
        let button = match button {
            1 => EnigoMouseButton::Left,
            3 => EnigoMouseButton::Right,
            2 => EnigoMouseButton::Middle,
            _ => todo!()
        };
        self.enigo.as_mut().unwrap().mouse_move_to(x, y);
        self.enigo.as_mut().unwrap().mouse_click(button);
    }
    pub fn mouse_move(&mut self, x: i32, y:i32, modifiers: u32) {
        self.enigo.as_mut().unwrap().mouse_move_to(x, y);
    }
    pub fn key_down(&mut self, key: u8) {
        let k = self.keymap.get(&key).unwrap();
        self.enigo.as_mut().unwrap().key_down(*k);
    }
    pub fn key_up(&mut self, key: u8) {
        println!(" key to match: {:?}", key);
        let k = self.keymap.get(&key).unwrap();
        println!(" key detected: {:?}", k);
        self.enigo.as_mut().unwrap().key_up(*k);
    }
}
