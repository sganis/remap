use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct MouseEvent {
    pub typ: String,
    pub x: f64,
    pub y: f64,
    pub modifiers: u32,
}
