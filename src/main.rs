use std::{
    iter,
    sync::{Arc, Condvar, Mutex},
};

use crate::{
    body::{Body, BodyId},
    camera::Camera,
    drawing::DrawHandler,
    rendering::{GpuCamera, RenderData, RenderState},
    universe::Universe,
};
use cgmath::{InnerSpace, Vector2, Vector3, Zero};
use eframe::{
    egui::{self, response},
    wgpu,
};

pub mod body;
pub mod camera;
pub mod drawing;
pub mod rendering;
pub mod universe;

struct App {
    camera: Camera,
    last_time: Option<std::time::Instant>,
    states: Vec<Universe>,
    current_state: usize,
    gen_future: usize,
    step_size: f64,
    thread_state: Arc<ThreadState>,
    speed: f64,
    playing: bool,
    accumulated_time: f64,
    lagging: bool,
    stats_open: bool,
    selected: Option<BodyId>,
    show_future: f64,
}

struct ThreadState {
    generation_state: Mutex<GenerationState>,
    wakeup: Condvar,
}

struct GenerationState {
    initial_state: Option<Universe>,
    new_states: Vec<Universe>,
    states_buffer_size: usize,
    step_size: f64,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> anyhow::Result<Self> {
        let renderer = cc.wgpu_render_state.as_ref().unwrap();
        let state = RenderState::new(renderer.target_format, &renderer.device, &renderer.queue)?;
        renderer.renderer.write().callback_resources.insert(state);

        let mut inital_universe = Universe::new(1.0);

        inital_universe.bodies.insert(
            BodyId::next_id(),
            Body {
                pos: Vector2 { x: -5.0, y: 0.0 },
                vel: Vector2 { x: -0.4, y: 0.5 },
                radius: 1.0,
                density: 1.0,
                color: Vector3 {
                    x: 1.0,
                    y: 0.0,
                    z: 0.0,
                },
            },
        );
        inital_universe.bodies.insert(
            BodyId::next_id(),
            Body {
                pos: Vector2 { x: 5.0, y: 0.0 },
                vel: Vector2 { x: -0.8, y: 0.5 },
                radius: 1.0,
                density: 1.0,
                color: Vector3 {
                    x: 0.0,
                    y: 1.0,
                    z: 0.0,
                },
            },
        );
        inital_universe.bodies.insert(
            BodyId::next_id(),
            Body {
                pos: Vector2 { x: 0.0, y: 5.0 },
                vel: Vector2 { x: 0.8, y: 0.5 },
                radius: 1.3,
                density: 1.0,
                color: Vector3 {
                    x: 0.0,
                    y: 0.0,
                    z: 1.0,
                },
            },
        );

        let gen_future = 20000;
        let step_size = 1.0 / 128.0;
        let thread_state = Arc::new(ThreadState {
            generation_state: Mutex::new(GenerationState {
                initial_state: Some(inital_universe.clone()),
                new_states: vec![],
                states_buffer_size: gen_future,
                step_size,
            }),
            wakeup: Condvar::new(),
        });

        std::thread::spawn({
            let thread_state = thread_state.clone();
            move || {
                let mut state = None;
                let mut lock = thread_state.generation_state.lock().unwrap();
                loop {
                    if let Some(initial_state) = lock.initial_state.take() {
                        lock.new_states.clear();
                        state = Some(initial_state);
                    }

                    if lock.new_states.len() >= lock.states_buffer_size {
                        lock = thread_state.wakeup.wait(lock).unwrap();
                        continue;
                    }
                    let step_size = lock.step_size;

                    if let Some(old_state) = &state {
                        drop(lock);

                        let mut new_state = old_state.clone();
                        new_state.step(step_size);

                        lock = thread_state.generation_state.lock().unwrap();
                        if lock.new_states.len() >= lock.states_buffer_size {
                            lock = thread_state.wakeup.wait(lock).unwrap();
                            continue;
                        }
                        lock.new_states.push(new_state.clone());
                        state = Some(new_state);
                    } else {
                        lock = thread_state.wakeup.wait(lock).unwrap();
                    }
                }
            }
        });

        Ok(Self {
            camera: Camera {
                pos: Vector2 { x: 0.0, y: 0.0 },
                offset: Vector2 { x: 0.0, y: 0.0 },
                view_height: 10.0,
                width: 0.0,
                height: 0.0,
            },
            last_time: None,
            states: vec![inital_universe],
            current_state: 0,
            gen_future,
            step_size,
            thread_state,
            speed: 1.0,
            playing: false,
            accumulated_time: 0.0,
            lagging: false,
            stats_open: true,
            selected: None,
            show_future: 100.0,
        })
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let time = std::time::Instant::now();
        let dt = time - self.last_time.unwrap_or(time);
        self.last_time = Some(time);

        let dt = dt.as_secs_f64();

        egui::TopBottomPanel::top("Time").show(ctx, |ui| {
            ui.horizontal(|ui| {
                self.stats_open |= ui.button("Stats").clicked();
            });
            ui.label("Time");
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(&mut self.current_state));
                ui.label(format!(" /  {}", self.states.len()));
            });
            let default_slider_width = ui.spacing_mut().slider_width;
            ui.spacing_mut().slider_width = ui.available_width();
            ui.add(egui::Slider::new(
                &mut self.current_state,
                0..=self.states.len() - 1,
            ));
            ui.spacing_mut().slider_width = default_slider_width;
            ui.horizontal(|ui| {
                ui.label("Show Future");
                ui.add(egui::Slider::new(&mut self.show_future, 0.0..=750.0));
            });
            ui.horizontal(|ui| {
                ui.label("Speed: ");
                ui.add(egui::DragValue::new(&mut self.speed).speed(0.1));
                if ui.button("Play / Pause").clicked() {
                    self.playing = !self.playing
                }
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

        self.lagging = false;
        self.accumulated_time += (dt * self.playing as u8 as f64 * self.speed).max(0.0);
        while self.accumulated_time >= self.step_size {
            if self.current_state + 1 < self.states.len() {
                self.current_state += 1;
            } else {
                self.lagging = true;
                self.accumulated_time = 0.0;
                break;
            }
            self.accumulated_time -= self.step_size;
        }

        egui::Window::new("Stats")
            .open(&mut self.stats_open)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label(format!("Frame Time: {:.3}ms", 1000.0 * dt));
                ui.label(format!("FPS: {:.3}", 1.0 / dt));
                if self.lagging {
                    ui.label("The game is lagging!");
                }
            });

        if !ctx.wants_keyboard_input() {
            ctx.input(|i| {
                let move_speed = 1.0;
                self.camera.pos.y += i.key_down(egui::Key::W) as u8 as f64
                    * dt
                    * move_speed
                    * self.camera.view_height;
                self.camera.pos.y -= i.key_down(egui::Key::S) as u8 as f64
                    * dt
                    * move_speed
                    * self.camera.view_height;
                self.camera.pos.x += i.key_down(egui::Key::D) as u8 as f64
                    * dt
                    * move_speed
                    * self.camera.view_height;
                self.camera.pos.x -= i.key_down(egui::Key::A) as u8 as f64
                    * dt
                    * move_speed
                    * self.camera.view_height;
            });
        }
        if !ctx.wants_pointer_input() {
            ctx.input(|i| {
                self.camera.view_height -=
                    i.raw_scroll_delta.y as f64 * self.camera.view_height * 0.005;
                self.camera.view_height = self.camera.view_height.max(0.1);
            });
        }

        let current_state_modified = false;

        {
            let mut lock = self.thread_state.generation_state.lock().unwrap();
            if current_state_modified {
                self.states.truncate(self.current_state + 1);
                lock.states_buffer_size = self
                    .gen_future
                    .saturating_sub((self.states.len()) - self.current_state);
                lock.initial_state = Some(self.states.last().unwrap().clone());
            } else {
                self.states.append(&mut lock.new_states);
                lock.states_buffer_size = self
                    .gen_future
                    .saturating_sub((self.states.len()) - self.current_state);
            }
            self.thread_state.wakeup.notify_one();
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(egui::Color32::from_rgb(50, 50, 50)))
            .show(ctx, |ui| {
                let (rect, response) =
                    ui.allocate_exact_size(ui.available_size(), egui::Sense::click_and_drag());
                let aspect = rect.width() / rect.height();
                self.camera.width = rect.width() as f64;
                self.camera.height = rect.height() as f64;

                if let Some(selected) = self.selected {
                    self.camera.offset = -self.states[self.current_state]
                        .bodies
                        .get(selected)
                        .unwrap()
                        .pos
                } else {
                    self.camera.offset = Vector2::zero()
                }
                let mouse_pos = if let Some(hover_pos) = ui.ctx().pointer_hover_pos() {
                    Vector2 {
                        x: hover_pos.x - rect.left_top().x,
                        y: hover_pos.y - rect.left_top().y,
                    }
                } else {
                    Vector2::zero()
                }
                .cast()
                .unwrap();

                let world_mouse_pos = self.camera.screen_to_world(mouse_pos);

                if response.clicked_by(egui::PointerButton::Secondary) {
                    let mut clicked_on_body = false;
                    self.states[self.current_state]
                        .bodies
                        .iter()
                        .for_each(|(key, body)| {
                            let mouse_to_body = body.pos - world_mouse_pos;
                            if mouse_to_body.magnitude() < body.radius {
                                if let Some(_selected) = self.selected {
                                    self.camera.pos -= self.camera.offset
                                }
                                self.selected = Some(key);
                                self.camera.pos -= body.pos;
                                self.camera.offset = -body.pos;
                                clicked_on_body = true
                            }
                        });
                    self.selected = if !clicked_on_body && let Some(_) = self.selected {
                        self.camera.pos -= self.camera.offset;
                        self.camera.offset = Vector2::zero();
                        None
                    } else {
                        self.selected
                    }
                }

                let mut d = DrawHandler::new();

                self.states[self.current_state].draw(&mut d);
                d.quads.reserve(
                    ((self.show_future / self.step_size) as usize).min(self.states.len() - 2)
                        * self.states[self.current_state].bodies.len(),
                );
                for i in 0..(self.show_future / self.step_size) as usize {
                    let i = i + self.current_state;
                    if i + 2 > self.states.len() {
                        break;
                    }
                    let universe = &self.states[i];
                    let new_universe = &self.states[i + 1];
                    universe.bodies.iter().for_each(|(id, _)| {
                        let Some(current) = universe.bodies.get(id) else {
                            return;
                        };
                        let Some(future) = new_universe.bodies.get(id) else {
                            return;
                        };
                        let current_offset = if let Some(selected) = self.selected {
                            if let Some(body) = universe.bodies.get(selected) {
                                body.pos + self.camera.offset
                            } else {
                                self.camera.offset
                            }
                        } else {
                            self.camera.offset
                        };
                        let future_offset = if let Some(selected) = self.selected {
                            if let Some(body) = new_universe.bodies.get(selected) {
                                body.pos + self.camera.offset
                            } else {
                                self.camera.offset
                            }
                        } else {
                            self.camera.offset
                        };

                        d.line(
                            (current.pos - current_offset).cast().unwrap(),
                            (future.pos - future_offset).cast().unwrap(),
                            0.005 * self.camera.view_height as f32,
                            current.color.cast().unwrap(),
                            0.1,
                        );

                        if i + 3 > self.states.len() {
                            d.circle(
                                (future.pos - future_offset).cast().unwrap(),
                                0.005 * self.camera.view_height as f32,
                                Vector3 {
                                    x: 0.25,
                                    y: 0.25,
                                    z: 0.25,
                                },
                                0.2,
                            );
                        }
                    });
                }

                ui.painter()
                    .add(eframe::egui_wgpu::Callback::new_paint_callback(
                        rect,
                        RenderData {
                            camera: GpuCamera {
                                position: (self.camera.pos - self.camera.offset).cast().unwrap(),
                                vertical_height: self.camera.view_height as f32,
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
