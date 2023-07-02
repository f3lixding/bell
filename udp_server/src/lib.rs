use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Default, Debug, Clone)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}
