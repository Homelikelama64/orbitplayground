use crate::{
    body::{Body, BodyId},
    camera::Camera,
    drawing::DrawHandler,
    rendering::{GpuCamera, RenderData, RenderState},
    save::Save,
    universe::Universe,
};
use cgmath::{InnerSpace, Vector2, Vector3, Zero};
use eframe::{
    egui::{self},
    wgpu,
};
use egui_file_dialog::FileDialog;
use std::{
    path::PathBuf,
    sync::{Arc, Condvar, Mutex},
};

pub mod body;
pub mod camera;
pub mod drawing;
pub mod rendering;
pub mod save;
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
    focused: Option<BodyId>,
    show_future: f64,
    path_quality: usize,
    selected: Option<BodyId>,
    file_dialog: FileDialog,
    file_interaction: FileInteraction,
    save_path: Option<PathBuf>,
    help_open: bool,
}

enum FileInteraction {
    None,
    Save,
    Load,
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

        inital_universe.bodies.push(Body {
            name: "Red".into(),
            pos: Vector2 { x: -5.0, y: 0.0 },
            vel: Vector2 { x: -0.4, y: 0.5 },
            radius: 1.0,
            density: 1.0,
            color: Vector3 {
                x: 1.0,
                y: 0.0,
                z: 0.0,
            },
        });
        inital_universe.bodies.push(Body {
            name: "Green".into(),
            pos: Vector2 { x: 5.0, y: 0.0 },
            vel: Vector2 { x: -0.8, y: 0.5 },
            radius: 1.0,
            density: 1.0,
            color: Vector3 {
                x: 0.0,
                y: 1.0,
                z: 0.0,
            },
        });
        inital_universe.bodies.push(Body {
            name: "Blue".into(),
            pos: Vector2 { x: 0.0, y: 5.0 },
            vel: Vector2 { x: 0.8, y: 0.5 },
            radius: 1.3,
            density: 1.0,
            color: Vector3 {
                x: 0.0,
                y: 0.0,
                z: 1.0,
            },
        });

        let mut states = vec![inital_universe].into();

        let mut camera = Camera {
            pos: Vector2 { x: 0.0, y: 0.0 },
            offset: Vector2 { x: 0.0, y: 0.0 },
            view_height: 10.0,
            width: 0.0,
            height: 0.0,
        };
        let mut current_state = 0;
        let mut step_size = 1.0 / 512.0;

        if let Some(save) = cc
            .storage
            .unwrap()
            .get_string("Save")
            .and_then(|s| serde_json::from_str(&s).ok())
        {
            Save {
                current_state,
                step_size,
                camera,
                states,
            } = save;
        }
        let mut save_path: Option<PathBuf> = cc
            .storage
            .unwrap()
            .get_string("SavePath")
            .map(PathBuf::from);
        if save_path
            .as_ref()
            .is_some_and(|path| path.as_os_str().is_empty())
        {
            save_path = None;
        }

        let gen_future = 20000usize;
        let thread_state = Arc::new(ThreadState {
            generation_state: Mutex::new(GenerationState {
                initial_state: Some(states.last().unwrap().clone()),
                new_states: vec![],
                states_buffer_size: gen_future.saturating_sub(states.len() - current_state),
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
            camera,
            last_time: None,
            states: states.into_owned(),
            current_state,
            gen_future,
            step_size,
            thread_state,
            speed: 1.0,
            playing: false,
            accumulated_time: 0.0,
            lagging: false,
            stats_open: true,
            focused: None,
            show_future: 100.0,
            path_quality: 128,
            selected: None,
            file_dialog: FileDialog::new()
                .add_file_filter_extensions("Orbit Save", vec!["orbit"])
                .default_file_filter("Orbit Save")
                .add_save_extension("Orbit Save", "orbit")
                .default_save_extension("Orbit Save"),
            file_interaction: FileInteraction::None,
            save_path,
            help_open: true,
        })
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let time = std::time::Instant::now();
        let dt = time - self.last_time.unwrap_or(time);
        self.last_time = Some(time);

        let dt = dt.as_secs_f64();
        let mut current_state_modified = false;

        egui::TopBottomPanel::top("Menu").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("New").clicked() {
                        self.save_path = None;
                        self.current_state = 0;
                        self.states = vec![Universe::new(1.0)];
                        self.camera.pos = Vector2::zero();
                        self.camera.offset = Vector2::zero();
                        self.camera.view_height = 10.0;
                        self.selected = None;
                        self.focused = None;
                        self.speed = 1.0;
                        self.gen_future = (5.0 / self.step_size) as usize;
                        current_state_modified = true;
                    }
                    if ui.button("Save").clicked() {
                        match &self.save_path {
                            Some(save_path) => {
                                let save_string = serde_json::to_string(&Save {
                                    current_state: self.current_state,
                                    step_size: self.step_size,
                                    camera: self.camera,
                                    states: self.states.as_slice().into(),
                                })
                                .unwrap();
                                _ = std::fs::write(save_path, save_string);
                            }
                            None => {
                                self.file_interaction = FileInteraction::Save;
                                self.file_dialog.save_file();
                            }
                        }
                    }
                    if ui.button("Save As").clicked() {
                        self.file_interaction = FileInteraction::Save;
                        self.file_dialog.save_file();
                    }
                    if ui.button("Open").clicked() {
                        self.file_interaction = FileInteraction::Load;
                        self.file_dialog.pick_file();
                    }
                });
                ui.menu_button("Windows", |ui| {
                    self.stats_open |= ui.button("Stats").clicked();
                });

                self.help_open |= ui.button("Help").clicked();
            });
        });

        self.file_dialog.update(ctx);
        'file_loading: {
            if let Some(mut path) = self.file_dialog.take_picked() {
                match core::mem::replace(&mut self.file_interaction, FileInteraction::None) {
                    FileInteraction::None => {}
                    FileInteraction::Save => {
                        let save_string = serde_json::to_string(&Save {
                            current_state: self.current_state,
                            step_size: self.step_size,
                            camera: self.camera,
                            states: self.states.as_slice().into(),
                        })
                        .unwrap();
                        if path.extension().is_none() {
                            path.set_extension("orbit");
                        }
                        _ = std::fs::write(&path, save_string);
                        self.save_path = Some(path);
                    }
                    FileInteraction::Load => {
                        let Ok(string) = std::fs::read_to_string(path) else {
                            break 'file_loading;
                        };
                        let Ok(Save {
                            current_state,
                            step_size,
                            camera,
                            states,
                        }) = serde_json::from_str(&string)
                        else {
                            break 'file_loading;
                        };
                        self.current_state = current_state;
                        self.step_size = step_size;
                        self.camera = camera;
                        self.states = states.into_owned();
                        current_state_modified = true;
                    }
                }
            }
        }

        egui::TopBottomPanel::bottom("Time").show(ctx, |ui| {
            ui.heading("Time");
            ui.horizontal(|ui| {
                let mut seconds = self.current_state as f64 * self.step_size;
                if ui
                    .add(egui::DragValue::new(&mut seconds).suffix("s").speed(1.0))
                    .changed()
                {
                    self.current_state = (seconds / self.step_size) as usize;
                }
                ui.label(format!(
                    " /  {:.2}s",
                    self.states.len() as f64 * self.step_size
                ));
            });
            let default_slider_width = ui.spacing_mut().slider_width;
            ui.spacing_mut().slider_width = ui.available_width() - 75.0;
            ui.add(egui::Slider::new(
                &mut self.current_state,
                0..=self.states.len() - 1,
            ));
            ui.spacing_mut().slider_width = default_slider_width;
            ui.horizontal(|ui| {
                ui.label("Gen Future: ");
                let mut seconds = self.gen_future as f64 * self.step_size;
                if ui
                    .add(egui::DragValue::new(&mut seconds).suffix("s").speed(1.0))
                    .changed()
                {
                    self.gen_future = (seconds / self.step_size) as usize;
                }
            });
            ui.horizontal(|ui| {
                ui.label("Show Future: ");
                ui.spacing_mut().slider_width = ui.available_width() - 75.0;
                ui.add(
                    egui::Slider::new(&mut self.show_future, 0.0..=10000.0)
                        .suffix("s")
                        .step_by(1.0),
                );
                ui.spacing_mut().slider_width = default_slider_width;
            });
            ui.horizontal(|ui| {
                ui.label("Path Quality: ");
                ui.add(egui::Slider::new(&mut self.path_quality, 1..=128));
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
                self.speed = self.speed.max(0.0)
            });
            if ui.button("Delete Past").clicked() {
                self.states.drain(..self.current_state);
                self.current_state = 0;
                self.states.shrink_to_fit();
            }
            if ui.button("Delete Future").clicked() {
                current_state_modified = true;
            }
        });

        {
            let mut open = self.selected.is_some();
            let name = self.selected.and_then(|selected| {
                Some(
                    self.states[self.current_state]
                        .bodies
                        .get(selected)?
                        .name
                        .as_str(),
                )
            });
            egui::Window::new(name.unwrap_or("Selected Body"))
                .id("Selected Body".into())
                .open(&mut open)
                .show(ctx, |ui| {
                    let Some(body) = self.selected.and_then(|selected| {
                        self.states[self.current_state].bodies.get_mut(selected)
                    }) else {
                        ui.label("The selected body does not exist in this time :p");
                        return;
                    };
                    let mut delete = false;
                    ui.add_enabled_ui(!self.playing, |ui| {
                        ui.horizontal(|ui| {
                            ui.label("Name:");
                            current_state_modified |=
                                ui.text_edit_singleline(&mut body.name).changed();
                        });
                        ui.horizontal(|ui| {
                            ui.label("Position:");
                            current_state_modified |= ui
                                .add(
                                    egui::DragValue::new(&mut body.pos.x)
                                        .speed(1.0)
                                        .prefix("x:"),
                                )
                                .changed();
                            current_state_modified |= ui
                                .add(
                                    egui::DragValue::new(&mut body.pos.y)
                                        .speed(1.0)
                                        .prefix("y:"),
                                )
                                .changed();
                        });
                        ui.horizontal(|ui| {
                            ui.label("Velocity:");
                            current_state_modified |= ui
                                .add(
                                    egui::DragValue::new(&mut body.vel.x)
                                        .speed(0.1)
                                        .prefix("x:"),
                                )
                                .changed();
                            current_state_modified |= ui
                                .add(
                                    egui::DragValue::new(&mut body.vel.y)
                                        .speed(0.1)
                                        .prefix("y:"),
                                )
                                .changed();
                        });
                        ui.horizontal(|ui| {
                            ui.label("Radius:");
                            current_state_modified |= ui
                                .add(
                                    egui::DragValue::new(&mut body.radius)
                                        .speed(0.1)
                                        .suffix("m"),
                                )
                                .changed();
                        });
                        ui.horizontal(|ui| {
                            ui.label("Density:");
                            current_state_modified |= ui
                                .add(
                                    egui::DragValue::new(&mut body.density)
                                        .speed(0.1)
                                        .suffix("m^2/kg"),
                                )
                                .changed();
                        });
                        ui.horizontal(|ui| {
                            ui.label("Mass:");
                            ui.add_enabled(
                                false,
                                egui::DragValue::new(&mut body.mass()).suffix("kg"),
                            );
                        });
                        ui.horizontal(|ui| {
                            ui.label("Color:");
                            let color: Vector3<f32> = body.color.cast().unwrap();
                            let mut color: [f32; 3] = color.into();
                            if ui.color_edit_button_rgb(&mut color).changed() {
                                current_state_modified = true;
                                let color: Vector3<f32> = color.into();
                                body.color = color.cast().unwrap();
                            }
                        });
                        if ui.button("Delete").clicked() {
                            current_state_modified = true;
                            delete = true;
                        }
                    });
                    if delete {
                        self.states[self.current_state]
                            .bodies
                            .remove(self.selected.unwrap());
                    }
                });
            if self.selected.is_some() && !open {
                self.selected = None;
            }
        }

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

        egui::Window::new("Guide")
            .open(&mut self.help_open)
            .resizable(false)
            .show(ctx, |ui| {
                ui.heading("How to use:");
                ui.label(
                    "- Time (Bottom Bar)\n\
                        The First slider controls where you are in the simulation\n\n\
                        Gen Future is in seconds and controls how many seconds into the future it is allowed to simulate from the current time(controlled from the slider above)\n\n\
                        Show Future is the amount of seconds bodies paths are displayed into the future\n\n\
                        Path Quality controls how often a new line is drawn, eg:128 every 128t a line is drawn to show the path(This is only visual)\n\n\
                        Speed Controls how fast the simulation is played back, The simulation starts Paused\n\n\
                        Delete Past and Delete Future removes the past or future\n\n\n\
                        - Controls\n\
                        WASD to move around\n\n\
                        Right Click on a body to focus on it, making all orbit paths and bodys relative to it. Right Click again not on a body to unfocus\n\n\
                        Left Click on a body to select it, when a body is selected a window will appear with the body's components, When paused you can edit these components (NOTE: When editing components, from that point the simulation has to recompute. Do not have Gen Future too high to avoid lag)\n\
                        ",
                );
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

                if i.key_pressed(egui::Key::Delete)
                    && let Some(selected) = self.selected
                {
                    self.selected = None;
                    self.states[self.current_state].bodies.remove(selected);
                    current_state_modified = true
                }
            });
        }
        if !ctx.wants_pointer_input() {
            ctx.input(|i| {
                self.camera.view_height -=
                    i.raw_scroll_delta.y as f64 * self.camera.view_height * 0.005;
                self.camera.view_height = self.camera.view_height.max(0.1);
            });
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(egui::Color32::from_rgb(50, 50, 50)))
            .show(ctx, |ui| {
                let (rect, response) =
                    ui.allocate_exact_size(ui.available_size(), egui::Sense::click_and_drag());
                let aspect = rect.width() / rect.height();
                self.camera.width = rect.width() as f64;
                self.camera.height = rect.height() as f64;

                if let Some(focused) = self.focused
                    && let Some(body) = self.states[self.current_state].bodies.get(focused)
                {
                    self.camera.offset = -body.pos;
                } else {
                    self.camera.offset = Vector2::zero()
                };
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
                                if let Some(_focused) = self.focused {
                                    self.camera.pos -= self.camera.offset
                                }
                                self.focused = Some(key);
                                self.camera.pos -= body.pos;
                                self.camera.offset = -body.pos;
                                clicked_on_body = true
                            }
                        });
                    self.focused = if !clicked_on_body && let Some(_) = self.focused {
                        self.camera.pos -= self.camera.offset;
                        self.camera.offset = Vector2::zero();
                        None
                    } else {
                        self.focused
                    }
                }

                if response.clicked() {
                    self.states[self.current_state]
                        .bodies
                        .iter()
                        .for_each(|(key, body)| {
                            let mouse_to_body = body.pos - world_mouse_pos;
                            if mouse_to_body.magnitude() < body.radius {
                                self.selected = Some(key);
                            }
                        });
                }

                if response.clicked_by(egui::PointerButton::Middle) && !self.playing {
                    current_state_modified = true;
                    let new_body = self.states[self.current_state].bodies.push(Body {
                        name: "Unnamed".into(),
                        pos: world_mouse_pos,
                        vel: Vector2::zero(),
                        radius: 1.0,
                        density: 1.0,
                        color: Vector3 {
                            x: 1.0,
                            y: 1.0,
                            z: 1.0,
                        },
                    });
                    self.selected = Some(new_body)
                }

                {
                    let mut lock = self.thread_state.generation_state.lock().unwrap();
                    if current_state_modified {
                        self.states[self.current_state].changed = true;
                        self.states.truncate(self.current_state + 1);
                        self.states.shrink_to_fit();
                        lock.step_size = self.step_size;
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

                let mut d = DrawHandler::new();

                self.states[self.current_state].draw(&mut d);
                d.quads.reserve(
                    ((self.show_future / self.step_size) as usize).min(self.states.len() - 2)
                        * self.states[self.current_state].bodies.len()
                        / self.path_quality,
                );
                let mut old_index = self.current_state;
                for i in 0..(self.show_future / self.step_size) as usize {
                    let future_index = i + self.current_state;
                    if future_index + 2 > self.states.len() {
                        let universe = &self.states.last().unwrap();
                        universe.bodies.iter().for_each(|(_, body)| {
                            let offset = if let Some(focused) = self.focused
                                && let Some(body) = universe.bodies.get(focused)
                            {
                                body.pos + self.camera.offset
                            } else {
                                self.camera.offset
                            };
                            d.circle(
                                (body.pos - offset).cast().unwrap(),
                                0.005 * self.camera.view_height as f32,
                                Vector3 {
                                    x: 0.75,
                                    y: 0.75,
                                    z: 0.75,
                                },
                                0.2,
                            );
                        });
                        break;
                    }
                    let universe = &self.states[old_index];
                    let new_universe = &self.states[future_index + 1];
                    if (i + self.current_state) % self.path_quality == 0 {
                        universe.bodies.iter().for_each(|(id, _)| {
                            let Some(current) = universe.bodies.get(id) else {
                                return;
                            };
                            let Some(future) = new_universe.bodies.get(id) else {
                                return;
                            };
                            let current_offset = if let Some(focused) = self.focused
                                && let Some(body) = universe.bodies.get(focused)
                            {
                                body.pos + self.camera.offset
                            } else {
                                self.camera.offset
                            };
                            let future_offset = if let Some(focused) = self.focused
                                && let Some(body) = new_universe.bodies.get(focused)
                            {
                                body.pos + self.camera.offset
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
                        });
                        old_index = future_index
                    }
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

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        let save_string = serde_json::to_string(&Save {
            current_state: self.current_state,
            step_size: self.step_size,
            camera: self.camera,
            states: self.states.as_slice().into(),
        })
        .unwrap();
        storage.set_string("Save", save_string);

        let save_path = if let Some(save_path) = &self.save_path
            && let Some(save_path) = save_path.to_str()
        {
            save_path.to_string()
        } else {
            String::new()
        };
        storage.set_string("SavePath", save_path);
    }
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
                wgpu_setup: eframe::egui_wgpu::WgpuSetup::CreateNew(
                    eframe::egui_wgpu::WgpuSetupCreateNew {
                        device_descriptor: Arc::new(|adapter| wgpu::DeviceDescriptor {
                            label: Some("wgpu device"),
                            required_features: wgpu::Features::default(),
                            required_limits: adapter.limits(),
                            memory_hints: wgpu::MemoryHints::default(),
                        }),
                        ..Default::default()
                    },
                ),
                ..Default::default()
            },
            ..Default::default()
        },
        Box::new(|cc| Ok(Box::new(App::new(cc)?))),
    )
}
