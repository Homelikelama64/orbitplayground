use crate::{body::BodyList, drawing::DrawHandler};
use cgmath::InnerSpace;

#[derive(Debug)]
pub struct Universe {
    pub bodies: BodyList,
    pub gravity: f64,
    pub changed: bool,
}

impl Clone for Universe {
    fn clone(&self) -> Self {
        Self {
            bodies: self.bodies.clone(),
            gravity: self.gravity,
            changed: false,
        }
    }
}

impl Universe {
    pub fn new(gravity: f64) -> Self {
        Self {
            bodies: BodyList::new(),
            gravity,
            changed: true,
        }
    }

    pub fn step(&mut self, dt: f64) {
        self.bodies.iter_mut_pairs(|_, a, _, b| {
            let a_to_b = b.pos - a.pos;
            let dist2 = a_to_b.magnitude2();
            let _dist = a_to_b.magnitude();

            a.vel += a_to_b.normalize() * (self.gravity * b.mass() / dist2) * dt;
            b.vel -= a_to_b.normalize() * (self.gravity * a.mass() / dist2) * dt;
        });
        self.bodies.iter_mut().for_each(|(_, body)| {
            body.pos += body.vel * dt;
        });
    }

    pub fn draw(&self, d: &mut DrawHandler) {
        self.bodies.iter().for_each(|(_, body)| {
            d.circle(
                body.pos.cast().unwrap(),
                body.radius as f32,
                body.color.cast().unwrap(),
                0.0,
            );
        });
    }
}
