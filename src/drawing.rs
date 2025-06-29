use crate::rendering::{GpuCircle, GpuQuad};
use cgmath::{Vector2, Vector3, prelude::*};

pub struct DrawHandler {
    pub quads: Vec<GpuQuad>,
    pub circles: Vec<GpuCircle>,
}

impl DrawHandler {
    pub fn new() -> DrawHandler {
        DrawHandler {
            quads: vec![],
            circles: vec![],
        }
    }
    pub fn circle(&mut self, pos: Vector2<f32>, radius: f32, color: Vector3<f32>, depth: f32) {
        self.circles.push(GpuCircle {
            position: Vector3 {
                x: pos.x,
                y: pos.y,
                z: depth,
            },
            color,
            radius,
        });
    }
    pub fn rect(
        &mut self,
        pos: Vector2<f32>,
        size: Vector2<f32>,
        angle: f32,
        color: Vector3<f32>,
        depth: f32,
    ) {
        self.quads.push(GpuQuad {
            position: Vector3 {
                x: pos.x,
                y: pos.y,
                z: depth,
            },
            rotation: angle.to_radians(),
            color,
            size,
        });
    }
    pub fn line(
        &mut self,
        start_pos: Vector2<f32>,
        end_pos: Vector2<f32>,
        thickness: f32,
        color: Vector3<f32>,
        depth: f32,
    ) {
        let start_to_end = end_pos - start_pos;
        let middle = start_pos + start_to_end * 0.5;
        let rotation = start_to_end.angle(Vector2 { x: 0.0, y: 1.0 }).0;
        let length = start_to_end.magnitude();
        self.quads.push(GpuQuad {
            position: Vector3 {
                x: middle.x,
                y: middle.y,
                z: depth,
            },
            rotation,
            color,
            size: Vector2 {
                x: length,
                y: thickness,
            },
        });
    }
}

impl Default for DrawHandler {
    fn default() -> Self {
        Self::new()
    }
}
