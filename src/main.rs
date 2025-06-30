use std::f64::consts::{PI, TAU};

use crate::{
    drawing::DrawHandler,
    rendering::{GpuCamera, RenderData, RenderState},
};
use cgmath::{Vector2, Vector3, num_traits::clamp, prelude::*};
use eframe::{
    egui::{self},
    wgpu,
};
use slotmap::{SlotMap, new_key_type};

pub mod drawing;
pub mod rendering;

struct App {
    camera: Camera,
    universe: Universe,
    last_time: Option<std::time::Instant>,
    look_ahead: f32,
    look_quality: usize,
    accumulated_time: f32,
    selected: Option<BodyId>,
    speed: f32,
    warp_to: f32,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> anyhow::Result<Self> {
        let renderer = cc.wgpu_render_state.as_ref().unwrap();
        let state = RenderState::new(renderer.target_format, &renderer.device, &renderer.queue)?;
        renderer.renderer.write().callback_resources.insert(state);

        let mut universe = Universe::new(1.0);
        //universe.bodies.push(Body::new(
        //    Vector2 { x: 5.0, y: 0.0 },
        //    Vector2 { x: 0.0, y: 1.0 },
        //    1.0,
        //    Vector3 {
        //        x: 0.0,
        //        y: 1.0,
        //        z: 0.0,
        //    },
        //));
        //universe.bodies.push(Body::new(
        //    Vector2 { x: 0.0, y: 0.0 },
        //    Vector2 { x: 0.0, y: 0.0 },
        //    1.0,
        //    Vector3 {
        //        x: 1.0,
        //        y: 0.0,
        //        z: 1.0,
        //    },
        //));
        //universe.bodies.push(Body::new(
        //    Vector2 { x: -5.0, y: 0.0 },
        //    Vector2 { x: 0.0, y: -1.0 },
        //    1.001,
        //    Vector3 {
        //        x: 1.0,
        //        y: 1.0,
        //        z: 0.0,
        //    },
        //));

        //universe.bodies.push(Body::new(
        //    Vector2 { x: -5.0, y: 0.0 },
        //    Vector2 { x: 0.0, y: 0.0 },
        //    1.0,
        //    Vector3 {
        //        x: 1.0,
        //        y: 1.0,
        //        z: 1.0,
        //    },
        //));
        //universe.bodies.push(Body::new(
        //    Vector2 { x: 5.0, y: 0.0 },
        //    Vector2 { x: 0.0, y: 0.0 },
        //    1.0,
        //    Vector3 {
        //        x: 1.0,
        //        y: 1.0,
        //        z: 1.0,
        //    },
        //));

        universe.bodies.insert(Body::new(
            Vector2 { x: 0.0, y: 0.0 },
            Vector2 { x: 0.0, y: 0.0 },
            5.0,
            Vector3 {
                x: 1.0,
                y: 1.0,
                z: 1.0,
            },
        ));
        universe.bodies.insert(Body::new(
            Vector2 { x: 20.0, y: 0.0 },
            Vector2 { x: 0.0, y: 0.1 },
            1.0,
            Vector3 {
                x: 1.0,
                y: 1.0,
                z: 1.0,
            },
        ));

        Ok(Self {
            camera: Camera {
                pos: Vector2 { x: 0.0, y: 0.0 },
                offset: Vector2 { x: 0.0, y: 0.0 },
                view_height: 10.0,
                width: 0.0,
                height: 0.0,
            },
            universe,
            look_ahead: 0.0,
            look_quality: 8,
            last_time: None,
            accumulated_time: 0.0,
            selected: None,
            speed: 1.0,
            warp_to: 0.0,
        })
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Camera {
    pos: Vector2<f32>,
    offset: Vector2<f32>,
    view_height: f32,
    width: f32,
    height: f32,
}

impl Camera {
    pub fn screen_to_world(&self, pos: Vector2<f32>) -> Vector2<f32> {
        Vector2 {
            x: (pos.x - self.width * 0.5) / self.width
                * (self.view_height * (self.width / self.height))
                + self.pos.x
                + self.offset.x,
            y: -(pos.y - self.height * 0.5) / self.height * self.view_height
                + self.pos.y
                + self.offset.y,
        }
    }
    pub fn world_to_screen(&self, pos: Vector2<f32>) -> Vector2<f32> {
        Vector2 {
            x: (pos.x - self.pos.x - self.offset.x)
                * (self.width / (self.view_height * (self.width / self.height)))
                + self.width * 0.5,
            y: (pos.y - self.pos.y - self.offset.y) * (self.height / self.view_height)
                + self.height * 0.5,
        }
    }
}

#[derive(Debug, Clone)]
struct Universe {
    bodies: SlotMap<BodyId, Body>,
    gravity: f32,
    step_time: f32,
    time: f32,
    removing: Vec<BodyId>,
    adding: Vec<Body>,
}

impl Universe {
    fn new(gravity: f32) -> Self {
        Universe {
            bodies: SlotMap::<BodyId, Body>::with_key(),
            gravity,
            time: 0.0,
            step_time: 1.0 / 128.0,
            removing: vec![],
            adding: vec![],
        }
    }
    fn update(&mut self, time: &mut f32) {
        while *time > self.step_time {
            *self = self.step(self.step_time);
            *time -= self.step_time;
        }
    }
    fn step(&self, dt: f32) -> Self {
        let mut universe = self.clone();
        universe.time += dt;
        universe.remove_queued();
        universe.add_bodies();
        let keys = universe.bodies.keys().collect::<Vec<_>>();
        for (index, &i) in keys.iter().enumerate() {
            universe.bodies[i].last_vel = universe.bodies[i].vel;
            for j in index + 1..universe.bodies.len() {
                let j = keys[j];
                let [a, b] = universe.bodies.get_disjoint_mut([i, j]).unwrap();
                let a_to_b = b.pos - a.pos;
                let dist_sqr = a_to_b.magnitude2();
                let dist = dist_sqr.sqrt();
                a.vel += a_to_b.normalize() * universe.gravity as f64 * b.mass
                    / a_to_b.magnitude2()
                    * dt as f64;
                b.vel -= a_to_b.normalize() * universe.gravity as f64 * a.mass
                    / a_to_b.magnitude2()
                    * dt as f64;
                let distance_edge = dist - a.radius as f64 - b.radius as f64;
                if distance_edge < 0.0 {
                    let new_a_vel = a.vel
                        - -a_to_b
                            * ((2.0 * b.mass) / (a.mass + b.mass))
                            * ((a.vel - b.vel).dot(-a_to_b) / dist_sqr);
                    let new_b_vel = b.vel
                        - a_to_b
                            * ((2.0 * a.mass) / (a.mass + b.mass))
                            * ((b.vel - a.vel).dot(a_to_b) / dist_sqr);

                    let a_collision_acc = a.vel - new_a_vel;
                    let b_collision_acc = b.vel - new_b_vel;
                    let a_collision_force = 0.5 * a.mass * a_collision_acc.magnitude2();
                    let b_collision_force = 0.5 * b.mass * b_collision_acc.magnitude2();
                    if a_collision_force > a.gravitational_binding_energy(universe.gravity) {
                        dbg!(a_collision_force);
                        dbg!(a.gravitational_binding_energy(universe.gravity));
                        println!("A broke Apart");
                        universe.break_body(i);
                    }
                    let b = &mut universe.bodies[j];
                    if b_collision_force > b.gravitational_binding_energy(universe.gravity) {
                        dbg!(b_collision_force);
                        dbg!(b.gravitational_binding_energy(universe.gravity));
                        universe.break_body(j);
                    }
                    let [a, b] = universe.bodies.get_disjoint_mut([i, j]).unwrap();

                    a.vel = new_a_vel;
                    b.vel = new_b_vel;

                    a.pos -=
                        (a_to_b.normalize() * -distance_edge) * (2.0 * a.mass) / (a.mass + b.mass);
                    b.pos +=
                        (a_to_b.normalize() * -distance_edge) * (2.0 * b.mass) / (a.mass + b.mass);
                }
            }

            let body = &mut universe.bodies[i];

            body.pos += body.vel * dt as f64;
        }
        universe
    }
    fn remove_queued(&mut self) {
        self.removing.sort_by_key(|&x| core::cmp::Reverse(x));
        for i in self.removing.drain(..) {
            self.bodies.remove(i);
        }
    }
    fn add_bodies(&mut self) {
        for new in &self.adding {
            self.bodies.insert(*new);
        }
        self.adding.clear();
    }
    fn draw(&self, d: &mut DrawHandler) {
        d.circles.reserve(self.bodies.len());
        for (key, body) in &self.bodies {
            d.circle(body.pos.cast().unwrap(), body.radius, body.color, 0.0);
        }
    }
    fn get_center_of_mass(&self) -> Vector2<f64> {
        let mut total_mass = 0.0;
        let mut total_position = Vector2::zero();
        for (key, body) in &self.bodies {
            total_mass += body.mass;
            total_position += body.pos * body.mass;
        }
        total_position / total_mass
    }
    fn break_body(&mut self, id: BodyId) {
        let body = &self.bodies[id];
        if body.radius < 0.3 {
            self.removing.push(id);
            return;
        }

        let radius = body.radius / 3.0;
        for i in 0..5 {
            let angle = (TAU / 6.0) * i as f64;
            let dir = Vector2 {
                x: angle.sin(),
                y: angle.cos(),
            };
            self.adding.push(Body::new(
                body.pos + dir * (body.radius - radius) as f64,
                body.vel * 1.0,
                radius,
                body.color,
            ));
        }
        self.adding
            .push(Body::new(body.pos, body.vel * 1.0, radius, body.color));
        self.removing.push(id);
    }
}

#[derive(Debug, Clone, Copy)]
struct Body {
    pos: Vector2<f64>,
    vel: Vector2<f64>,
    last_vel: Vector2<f64>,
    radius: f32,
    mass: f64,
    color: Vector3<f32>,
}

new_key_type! {
    struct BodyId;
}

impl Body {
    fn new(pos: Vector2<f64>, vel: Vector2<f64>, radius: f32, color: Vector3<f32>) -> Self {
        Body {
            pos,
            vel,
            last_vel: vel,
            radius,
            mass: radius as f64 * radius as f64 * PI,
            color,
        }
    }
    fn acc(&self) -> Vector2<f64> {
        self.vel - self.last_vel
    }
    fn gravitational_binding_energy(&self, gravity: f32) -> f64 {
        // https://physics.stackexchange.com/questions/216377/gravitational-binding-energy-of-2d-circle
        (2.0 * gravity as f64 * self.mass.powi(2)) / (3.0 * self.radius as f64)
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let time = std::time::Instant::now();
        let dt = time - self.last_time.unwrap_or(time);
        self.last_time = Some(time);

        let dt = dt.as_secs_f32();

        let universe_center_of_mass = self.universe.get_center_of_mass();
        egui::Window::new("Stats").resizable(false).show(ctx, |ui| {
            ui.label(format!("Frame Time: {:.3}ms", 1000.0 * dt));
            ui.label(format!("FPS: {:.3}", 1.0 / dt));
            ui.label(format!(
                "Center: x:{:.2}, y:{:.2}, ({:.2})",
                universe_center_of_mass.x,
                universe_center_of_mass.y,
                universe_center_of_mass.magnitude()
            ));
        });

        egui::Window::new("Physics")
            .resizable(false)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Gravity Strength: ");
                    ui.add(egui::DragValue::new(&mut self.universe.gravity).speed(0.1));
                });
            });

        egui::Window::new("Display")
            .resizable(false)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Look Ahead: ");
                    //ui.add(egui::DragValue::new(&mut self.look_ahead).speed(0.1));
                    ui.add(egui::Slider::new(&mut self.look_ahead, 0.0..=2000.0).step_by(1.0));
                });
                ui.horizontal(|ui| {
                    ui.label("Look Ahead Quality(Higher = Worse): ");
                    //ui.add(egui::DragValue::new(&mut self.look_ahead).speed(0.1));
                    ui.add(egui::Slider::new(&mut self.look_quality, 1..=32).step_by(1.0));
                });
                ui.horizontal(|ui| {
                    ui.label("Speed: ");
                    ui.add(egui::DragValue::new(&mut self.speed).speed(0.1));
                    if ui.button("0.1x").clicked() {
                        self.speed = 0.1
                    }
                    if ui.button("0.5x").clicked() {
                        self.speed = 0.5
                    }
                    if ui.button("1x").clicked() {
                        self.speed = 1.0
                    }
                    if ui.button("5x").clicked() {
                        self.speed = 5.0
                    }
                    if ui.button("10x").clicked() {
                        self.speed = 10.0
                    }
                    if ui.button("50x").clicked() {
                        self.speed = 50.0
                    }
                    if ui.button("100x").clicked() {
                        self.speed = 100.0
                    }
                });
            });

        if !ctx.wants_keyboard_input() {
            ctx.input(|i| {
                let move_speed = 1.0;
                self.camera.pos.y += i.key_down(egui::Key::W) as u8 as f32
                    * dt
                    * move_speed
                    * self.camera.view_height;
                self.camera.pos.y -= i.key_down(egui::Key::S) as u8 as f32
                    * dt
                    * move_speed
                    * self.camera.view_height;
                self.camera.pos.x += i.key_down(egui::Key::D) as u8 as f32
                    * dt
                    * move_speed
                    * self.camera.view_height;
                self.camera.pos.x -= i.key_down(egui::Key::A) as u8 as f32
                    * dt
                    * move_speed
                    * self.camera.view_height;
            });
        }
        if !ctx.wants_pointer_input() {
            ctx.input(|i| {
                self.camera.view_height -= i.raw_scroll_delta.y * self.camera.view_height * 0.005;
                self.camera.view_height = self.camera.view_height.max(0.1);
            });
        }
        let warp_time = clamp(self.warp_to - self.universe.time, 0.0, 5.0);

        self.accumulated_time += dt * self.speed + warp_time;
        self.universe.update(&mut self.accumulated_time);

        let universe_center_of_mass = self.universe.get_center_of_mass();
        if universe_center_of_mass.magnitude() > 100.0 {
            for (_, body) in &mut self.universe.bodies {
                body.pos -= universe_center_of_mass;
            }
            if self.selected.is_none() {
                self.camera.pos -= universe_center_of_mass.cast().unwrap();
            }
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(egui::Color32::from_rgb(50, 50, 50)))
            .show(ctx, |ui| {
                let (rect, response) =
                    ui.allocate_exact_size(ui.available_size(), egui::Sense::click_and_drag());
                let aspect = rect.width() / rect.height();
                self.camera.width = rect.width();
                self.camera.height = rect.height();

                if let Some(selected) = self.selected {
                    self.camera.offset = self.universe.bodies[selected].pos.cast().unwrap()
                } else {
                    self.camera.offset = Vector2::zero()
                }

                let mouse_pos = if let Some(hover_pos) = response.hover_pos() {
                    Vector2 {
                        x: hover_pos.x,
                        y: hover_pos.y,
                    }
                } else {
                    Vector2::zero()
                };
                let world_mouse_pos = self.camera.screen_to_world(mouse_pos);

                if response.clicked_by(egui::PointerButton::Secondary) {
                    let mut clicked_on_body = false;
                    for (key, body) in &self.universe.bodies {
                        let body_to_mouse = world_mouse_pos - body.pos.cast().unwrap();
                        if body_to_mouse.magnitude() < body.radius {
                            if let Some(selected) = self.selected {
                                self.camera.pos +=
                                    self.universe.bodies[selected].pos.cast().unwrap()
                            }
                            self.selected = Some(key);
                            self.camera.pos -= body.pos.cast().unwrap();
                            self.camera.offset = body.pos.cast().unwrap();
                            clicked_on_body = true;
                        }
                    }
                    self.selected = if !clicked_on_body && let Some(selected) = self.selected {
                        self.camera.pos += self.universe.bodies[selected].pos.cast().unwrap();
                        self.camera.offset = Vector2::zero();
                        None
                    } else {
                        self.selected
                    }
                }

                let mut d = DrawHandler::new();
                d.circle(
                    universe_center_of_mass.cast().unwrap(),
                    1.0,
                    Vector3 {
                        x: 0.25,
                        y: 0.25,
                        z: 0.25,
                    },
                    0.0,
                );

                self.universe.draw(&mut d);

                let mut old_future = self.universe.clone();
                let mut future = old_future.step(self.universe.step_time);
                d.quads.reserve(
                    (self.look_ahead / self.universe.step_time) as usize * future.bodies.len(),
                );
                let mut will_time_warp = false;
                for step in 0..(self.look_ahead / self.universe.step_time) as usize {
                    if step % self.look_quality == 0 {
                        for (key, body) in &future.bodies {
                            let mut pos = body.pos;
                            let mut old_pos = body.pos - body.vel * self.universe.step_time as f64 * self.look_quality as f64;
                            if let Some(selected) = self.selected {
                                pos -= future.bodies[selected].pos
                                    - self.camera.offset.cast().unwrap();
                                old_pos -= old_future.bodies[selected].pos
                                    - self.camera.offset.cast().unwrap();
                            }

                            d.line(
                                old_pos.cast().unwrap(),
                                pos.cast().unwrap(),
                                0.002 * self.camera.view_height,
                                future.bodies[key].color,
                                0.1,
                            );

                            if response.clicked() {
                                let pos = pos;
                                let old = old_pos;
                                let ed = pos - old;
                                let e = ed.x;
                                let d_ = -ed.y;
                                let ba = world_mouse_pos.cast().unwrap() - old;
                                let a = ba.x;
                                let b = ba.y;
                                let projected = Vector2 {
                                    x: -(b * d_ * e - a * e * e) / (d_ * d_ + e * e),
                                    y: (b * d_ * d_ - a * d_ * e) / (d_ * d_ + e * e),
                                };
                                if (ba - projected).magnitude()
                                    <= 0.01 * self.camera.view_height as f64
                                {
                                    let length =
                                        (Vector2 { x: e, y: -d_ }).normalize().dot(projected);
                                    if (length < ed.magnitude()) && length > 0.0 && !will_time_warp
                                    {
                                        self.warp_to = future.time;
                                        will_time_warp = true
                                    }
                                }
                            }
                        }
                        old_future = future.clone();
                    }
                    future = future.step(self.universe.step_time);
                }

                ui.painter()
                    .add(eframe::egui_wgpu::Callback::new_paint_callback(
                        rect,
                        RenderData {
                            camera: GpuCamera {
                                position: self.camera.pos + self.camera.offset,
                                vertical_height: self.camera.view_height,
                                aspect,
                            },
                            quads: d.quads,
                            circles: d.circles,
                        },
                    ));
            });

        ctx.request_repaint();
    }

    fn save(&mut self, _storage: &mut dyn eframe::Storage) {}
}

fn main() -> eframe::Result<()> {
    eframe::run_native(
        "Orbit Playground",
        eframe::NativeOptions {
            renderer: eframe::Renderer::Wgpu,
            vsync: false,
            depth_buffer: 24,
            wgpu_options: eframe::egui_wgpu::WgpuConfiguration {
                present_mode: wgpu::PresentMode::AutoNoVsync,
                ..Default::default()
            },
            ..Default::default()
        },
        Box::new(|cc| Ok(Box::new(App::new(cc)?))),
    )
}
