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
            (0, Key::Layout('0')),
            (1, Key::Layout('1')),
            (2, Key::Layout('2')),
            (3, Key::Layout('3')),
            (4, Key::Layout('4')),
            (5, Key::Layout('5')),
            (6, Key::Layout('6')),
            (7, Key::Layout('7')),
            (8, Key::Layout('8')),
            (9, Key::Layout('9')),
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
            (36, Key::F1),
            (37, Key::F2),
            (38, Key::F3),
            (39, Key::F4),
            (40, Key::F5),
            (41, Key::F6),
            (42, Key::F7),
            (43, Key::F8),
            (44, Key::F9),
            (45, Key::F10),
            (46, Key::F11),
            (47, Key::F12),
            //(48, Key::F13),
            //(49, Key::F14),
            //(50, Key::F15),
            (51, Key::DownArrow),
            (52, Key::LeftArrow),
            (53, Key::RightArrow),
            (54, Key::UpArrow),
            (55, Key::Layout('\'')),
            (56, Key::Layout('`')),
            (57, Key::Layout('\\')),
            (58, Key::Layout(',')),
            (59, Key::Layout('=')),
            (60, Key::Layout('[')),
            (61, Key::Layout('-')),
            (62, Key::Layout('.')),
            (63, Key::Layout(']')),
            (64, Key::Layout(';')),
            (65, Key::Layout('/')),
            (66, Key::Backspace),
            (67, Key::Delete),
            (68, Key::End),
            (69, Key::Return),
            (70, Key::Escape),
            (71, Key::Home),
            //(72, Key::Insert),
            //(73, Key::Menu),
            (74, Key::PageDown),
            (75, Key::PageUp),
            //(76, Key::Pause),
            (77, Key::Space),
            (78, Key::Tab),
            //(79, Key::NumLock),
            (80, Key::CapsLock),
            //(81, Key::ScrollLock),
            (82, Key::Shift),
            (83, Key::Shift),
            (84, Key::Control),
            (85, Key::Control),
            // (86, Key::NumPad0),
            // (87, Key::NumPad1),
            // (88, Key::NumPad2),
            // (89, Key::NumPad3),
            // (90, Key::NumPad4),
            // (91, Key::NumPad5),
            // (92, Key::NumPad6),
            // (93, Key::NumPad7),
            // (94, Key::NumPad8),
            // (95, Key::NumPad9),
            // (96, Key::NumPadDot),
            // (97, Key::NumPadSlash),
            // (98, Key::NumPadAsterisk),
            // (99, Key::NumPadMinus),
            // (100, Key::NumPadPlus),
            // (101, Key::NumPadEnter),
            (102, Key::Alt),
            (103, Key::Alt),
            (104, Key::Meta),
            (105, Key::Meta),

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
        if let Some(k) = self.keymap.get(&key) {
            self.enigo.as_mut().unwrap().key_down(*k);
        } else {
            println!("key not implemented: {}", key);
        }
    }
    pub fn key_up(&mut self, key: u8) {
        if let Some(k) = self.keymap.get(&key) {
            self.enigo.as_mut().unwrap().key_up(*k);
        } else {
            println!("key not implemented: {}", key);
        }
    }
}
