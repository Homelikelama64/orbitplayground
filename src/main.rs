use std::f32::consts::PI;

use crate::{
    drawing::DrawHandler,
    rendering::{GpuCamera, RenderData, RenderState},
};
use cgmath::{Vector2, Vector3, prelude::*};
use eframe::{
    egui::{self},
    wgpu,
};

pub mod drawing;
pub mod rendering;

struct App {
    camera: Camera,
    universe: Universe,
    last_time: Option<std::time::Instant>,
    look_ahead: f32,
    look_quality: usize,
    accumulated_time: f32,
    selected: Option<usize>,
    speed: f32,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> anyhow::Result<Self> {
        let renderer = cc.wgpu_render_state.as_ref().unwrap();
        let state = RenderState::new(renderer.target_format, &renderer.device, &renderer.queue)?;
        renderer.renderer.write().callback_resources.insert(state);

        let mut universe = Universe::new(1.0);
        universe.bodies.push(Body::new(
            Vector2 { x: -5.0, y: 0.0 },
            Vector2 { x: 0.0, y: 1.0 },
            1.2,
            Vector3 {
                x: 0.0,
                y: 1.0,
                z: 0.0,
            },
        ));
        universe.bodies.push(Body::new(
            Vector2 { x: 5.0, y: 0.0 },
            Vector2 { x: 0.0, y: -1.61 },
            1.1,
            Vector3 {
                x: 1.0,
                y: 0.0,
                z: 1.0,
            },
        ));
        universe.bodies.push(Body::new(
            Vector2 { x: 0.0, y: 4.0 },
            Vector2 { x: 0.0, y: -0.5 },
            1.3,
            Vector3 {
                x: 1.0,
                y: 1.0,
                z: 0.0,
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
            look_ahead: 20.0,
            look_quality: 8,
            last_time: None,
            accumulated_time: 0.0,
            selected: None,
            speed: 1.0,
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
            x: (pos.x - self.pos.x)
                * (self.width / (self.view_height * (self.width / self.height)))
                + self.width * 0.5,
            y: (pos.y - self.pos.y) * (self.height / self.view_height) + self.height * 0.5,
        }
    }
}

#[derive(Debug, Clone)]
struct Universe {
    bodies: Vec<Body>,
    gravity: f32,
    step_time: f32,
    time: f32,
}

impl Universe {
    fn new(gravity: f32) -> Self {
        Universe {
            bodies: vec![],
            gravity,
            time: 0.0,
            step_time: 0.01,
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
        for i in 0..universe.bodies.len() {
            for j in i + 1..universe.bodies.len() {
                let [a, b] = universe.bodies.get_disjoint_mut([i, j]).unwrap();
                let a_to_b = b.pos - a.pos;
                a.acc += a_to_b.normalize() * universe.gravity * b.mass / a_to_b.magnitude2() * dt;
                b.acc -= a_to_b.normalize() * universe.gravity * a.mass / a_to_b.magnitude2() * dt;
            }

            let body = &mut universe.bodies[i];
            body.vel += body.acc;
            body.acc = Vector2::zero();

            body.pos += body.vel * dt;
        }
        universe
    }
    fn draw(&self, d: &mut DrawHandler) {
        d.circles.reserve(self.bodies.len());
        for body in &self.bodies {
            d.circle(body.pos, body.radius, body.color, 0.0);
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Body {
    pos: Vector2<f32>,
    vel: Vector2<f32>,
    acc: Vector2<f32>,
    radius: f32,
    mass: f32,
    color: Vector3<f32>,
}

impl Body {
    fn new(pos: Vector2<f32>, vel: Vector2<f32>, radius: f32, color: Vector3<f32>) -> Self {
        Body {
            pos,
            vel,
            acc: Vector2::zero(),
            radius,
            mass: radius * radius * PI,
            color,
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let time = std::time::Instant::now();
        let dt = time - self.last_time.unwrap_or(time);
        self.last_time = Some(time);

        let dt = dt.as_secs_f32();

        egui::Window::new("Stats").resizable(false).show(ctx, |ui| {
            ui.label(format!("Frame Time: {:.3}ms", 1000.0 * dt));
            ui.label(format!("FPS: {:.3}", 1.0 / dt));
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

        self.accumulated_time += dt * self.speed;
        self.universe.update(&mut self.accumulated_time);

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(egui::Color32::from_rgb(50, 50, 50)))
            .show(ctx, |ui| {
                let (rect, response) =
                    ui.allocate_exact_size(ui.available_size(), egui::Sense::click_and_drag());
                let aspect = rect.width() / rect.height();
                self.camera.width = rect.width();
                self.camera.height = rect.height();

                if let Some(selected) = self.selected {
                    self.camera.offset = self.universe.bodies[selected].pos
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
                    for i in 0..self.universe.bodies.len() {
                        let body = self.universe.bodies[i];
                        let body_to_mouse = world_mouse_pos - body.pos;
                        if body_to_mouse.magnitude() < body.radius {
                            if let Some(selected) = self.selected {
                                self.camera.pos += self.universe.bodies[selected].pos
                            }
                            self.selected = Some(i);
                            self.camera.pos -= body.pos;
                            self.camera.offset = body.pos;
                            clicked_on_body = true;
                        }
                    }
                    self.selected = if !clicked_on_body && let Some(selected) = self.selected {
                        self.camera.pos += self.universe.bodies[selected].pos;
                        self.camera.offset = Vector2::zero();
                        None
                    } else {
                        self.selected
                    }
                }

                let mut d = DrawHandler::new();

                self.universe.draw(&mut d);

                let mut old_future = self.universe.clone();
                let mut future = old_future.step(self.universe.step_time);
                d.quads.reserve(
                    (self.look_ahead / self.universe.step_time) as usize * future.bodies.len(),
                );
                for step in 0..(self.look_ahead / self.universe.step_time) as usize {
                    if step % self.look_quality == 0 {
                        for i in 0..future.bodies.len() {
                            let mut pos = future.bodies[i].pos;
                            let mut old_pos = old_future.bodies[i].pos;
                            if let Some(selected) = self.selected {
                                pos -= future.bodies[selected].pos - self.camera.offset;
                                old_pos -= old_future.bodies[selected].pos - self.camera.offset;
                            }

                            d.line(
                                old_pos,
                                pos,
                                0.005 * self.camera.view_height,
                                future.bodies[i].color,
                                0.1,
                            );
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
