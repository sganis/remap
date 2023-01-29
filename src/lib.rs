use serde::{Deserialize, Serialize};

#[cfg(linux)]
mod linux;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Event {
    KeyPress {
        key: String, 
        modifier: i32, 
    },
    Click { 
        x: i32, 
        y: i32,
        modifier: i32,
    },
    Scroll {
        value: i32,
        modifier: i32,
    },
}
impl Event {
    pub fn as_bytes(&mut self) -> Vec<u8> {
        bincode::serialize(self).unwrap()
    }
}