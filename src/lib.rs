use serde::{Deserialize, Serialize};
use enigo::{Enigo, MouseButton, MouseControllable, Key, KeyboardControllable};

#[derive(Serialize, Deserialize, Debug, Copy, Clone)]
pub struct MouseEvent {
    pub typ: char,
    pub x: i32,
    pub y: i32,
    pub modifiers: u32,
}

pub struct Input {
    enigo: Option<Enigo>,
}

#[allow(dead_code)]
impl Input {
    pub fn new() -> Self {
        Self { 
            enigo: Some(Enigo::new()),
        }
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
    pub fn mouse_click(&mut self, event: MouseEvent) {
        let x = event.x;
        let y = event.y;
        let button = MouseButton::Left;
        self.enigo.as_mut().unwrap().mouse_move_to(x, y);
        self.enigo.as_mut().unwrap().mouse_click(button);
    }
    pub fn key(&mut self, key: &str) {
        println!(" key to match: {:?}", key);
        let k = match key {
            "Return" => Key::Return,
            "BackSpace" => Key::Backspace,
            "Delete" => Key::Delete,
            "Page_Up" => Key::PageUp,
            "Page_Down" => Key::PageDown,
            "Up" => Key::UpArrow,
            "Down" => Key::DownArrow,
            "Left" => Key::LeftArrow,
            "Right" => Key::RightArrow,
            "End" => Key::End,
            "Home" => Key::Home,
            "Tab" => Key::Tab,
            "Escape" => Key::Escape,
            c => Key::Layout(c.chars().next().unwrap()),
        };
        println!(" key detected: {:?}", k);
        self.enigo.as_mut().unwrap().key_click(k);

    }
}
