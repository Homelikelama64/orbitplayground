use cgmath::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Camera {
    pub pos: Vector2<f64>,
    pub offset: Vector2<f64>,
    pub view_height: f64,
    pub width: f64,
    pub height: f64,
}

impl Camera {
    pub fn new(pos: Vector2<f64>, offset: Vector2<f64>, view_height: f64) -> Camera {
        Self {
            pos,
            offset,
            view_height,
            width: 0.0,
            height: 0.0,
        }
    }

    pub fn screen_to_world(&self, pos: Vector2<f64>) -> Vector2<f64> {
        Vector2 {
            x: (pos.x - self.width * 0.5) / self.width
                * (self.view_height * (self.width / self.height))
                + self.pos.x
                - self.offset.x,
            y: -(pos.y - self.height * 0.5) / self.height * self.view_height + self.pos.y
                - self.offset.y,
        }
    }

    pub fn world_to_screen(&self, pos: Vector2<f64>) -> Vector2<f64> {
        Vector2 {
            x: (pos.x - self.pos.x + self.offset.x)
                * (self.width / (self.view_height * (self.width / self.height)))
                + self.width * 0.5,
            y: (pos.y - self.pos.y + self.offset.y) * (self.height / self.view_height)
                + self.height * 0.5,
        }
    }
}
