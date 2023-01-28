use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Copy, Clone)]
pub struct MouseEvent {
    pub typ: char,
    pub x: i32,
    pub y: i32,
    pub modifiers: u32,
}
