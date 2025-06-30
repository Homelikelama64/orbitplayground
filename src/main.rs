use std::sync::{Arc, Condvar, Mutex};

use crate::{
    body::{Body, BodyId},
    camera::Camera,
    drawing::DrawHandler,
    rendering::{GpuCamera, RenderData, RenderState},
    universe::Universe,
};
use cgmath::{Vector2, Vector3};
use eframe::{
    egui::{self},
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
                vel: Vector2 { x: 0.0, y: 0.5 },
                radius: 1.0,
                density: 1.0,
                color: Vector3 {
                    x: 1.0,
                    y: 1.0,
                    z: 1.0,
                },
            },
        );
        inital_universe.bodies.insert(
            BodyId::next_id(),
            Body {
                pos: Vector2 { x: 5.0, y: 0.0 },
                vel: Vector2 { x: 0.0, y: -0.5 },
                radius: 1.0,
                density: 1.0,
                color: Vector3 {
                    x: 1.0,
                    y: 1.0,
                    z: 1.0,
                },
            },
        );

        let gen_future = 100;
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
            ui.spacing_mut().slider_width = ui.available_width();
            ui.add(egui::Slider::new(
                &mut self.current_state,
                0..=self.states.len() - 1,
            ));
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
                    .saturating_sub(self.states.len() - self.current_state);
                lock.initial_state = Some(self.states.last().unwrap().clone());
            } else {
                self.states.append(&mut lock.new_states);
                lock.states_buffer_size = self
                    .gen_future
                    .saturating_sub(self.states.len() - self.current_state);
            }
            self.thread_state.wakeup.notify_one();
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(egui::Color32::from_rgb(50, 50, 50)))
            .show(ctx, |ui| {
                let (rect, _response) =
                    ui.allocate_exact_size(ui.available_size(), egui::Sense::click_and_drag());
                let aspect = rect.width() / rect.height();
                self.camera.width = rect.width() as f64;
                self.camera.height = rect.height() as f64;

                let mut d = DrawHandler::new();

                self.states[self.current_state].draw(&mut d);

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
