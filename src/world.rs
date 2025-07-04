use crate::{
    body::{Body, BodyId},
    camera::Camera,
    drawing::DrawHandler,
    save::{Data, Save},
    universe::Universe,
};
use cgmath::{InnerSpace, Vector2, Vector3, Zero};
use eframe::egui;
use std::sync::{Arc, Condvar, Mutex};

pub struct ThreadState {
    pub generation_state: Mutex<GenerationState>,
    pub wakeup: Condvar,
}

pub struct GenerationState {
    pub initial_state: Option<Universe>,
    pub new_states: Vec<Universe>,
    pub states_buffer_size: usize,
    pub step_size: f64,
}

pub struct World {
    pub name: String,
    pub camera: Camera,
    pub states: Vec<Universe>,
    pub gen_future: usize,
    pub show_future: f64,
    pub path_quality: usize,
    pub current_state: usize,
    pub thread_state: Arc<ThreadState>,
    pub step_size: f64,
    pub speed: f64,
    pub playing: bool,
    pub focused: Option<BodyId>,
    pub selected: Option<BodyId>,
    pub current_state_modified: bool,
    pub auto_orbit: bool,
    pub accumulated_time: f64,
    pub save_path: Option<String>,
    pub modified_since_save_to_file: bool,
}

impl World {
    pub fn new(step_size: f64) -> Self {
        let current_state = 0;
        let states = vec![Universe::new(1.0)];

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

        Self::spawn_update_thread(thread_state.clone());

        Self {
            name: "Unnamed".to_string(),
            camera: Camera::new(Vector2::zero(), Vector2::zero(), 10.0),
            states,
            gen_future,
            show_future: 100.0,
            path_quality: 128,
            current_state,
            thread_state,
            step_size,
            speed: 1.0,
            playing: false,
            focused: None,
            selected: None,
            current_state_modified: false,
            auto_orbit: false,
            accumulated_time: 0.0,
            save_path: None,
            modified_since_save_to_file: true,
        }
    }

    pub fn state(&self) -> &Universe {
        &self.states[self.current_state]
    }

    pub fn from_save(save: Save) -> World {
        let states: Vec<Universe> = save.states.into();

        let gen_future = 20000usize;
        let thread_state = Arc::new(ThreadState {
            generation_state: Mutex::new(GenerationState {
                initial_state: Some(states.last().unwrap().clone()),
                new_states: vec![],
                states_buffer_size: gen_future
                    .saturating_sub(states.len() - save.data.current_state),
                step_size: save.data.step_size,
            }),
            wakeup: Condvar::new(),
        });

        Self::spawn_update_thread(thread_state.clone());

        Self {
            name: save.data.name.clone(),
            camera: save.data.camera,
            states,
            gen_future,
            show_future: save.data.show_future,
            path_quality: save.data.path_quality,
            current_state: save.data.current_state,
            thread_state,
            step_size: save.data.step_size,
            speed: save.data.speed,
            playing: false,
            focused: None,
            selected: None,
            current_state_modified: false,
            auto_orbit: false,
            accumulated_time: 0.0,
            save_path: save.data.save_path,
            modified_since_save_to_file: false,
        }
    }

    pub fn to_save(&self) -> Save {
        Save {
            data: Data {
                name: self.name.clone(),
                camera: self.camera,
                gen_future: self.gen_future,
                show_future: self.show_future,
                path_quality: self.path_quality,
                current_state: self.current_state,
                step_size: self.step_size,
                speed: self.speed,
                save_path: self.save_path.clone(),
            },
            states: self.states.as_slice().into(),
        }
    }

    fn spawn_update_thread(thread_state: Arc<ThreadState>) {
        std::thread::spawn(move || {
            let mut state: Option<Universe> = None;
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
        });
    }

    pub fn ui(&mut self, ctx: &egui::Context, dt: f64) {
        self.current_state_modified = false;
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
                    self.modified_since_save_to_file = true;
                    self.gen_future = (seconds / self.step_size) as usize;
                }
            });
            ui.horizontal(|ui| {
                ui.label("Show Future: ");
                ui.spacing_mut().slider_width = ui.available_width() - 75.0;
                if ui
                    .add(
                        egui::Slider::new(&mut self.show_future, 0.0..=10000.0)
                            .suffix("s")
                            .step_by(1.0),
                    )
                    .changed()
                {
                    self.modified_since_save_to_file = true;
                }
                ui.spacing_mut().slider_width = default_slider_width;
            });
            ui.horizontal(|ui| {
                ui.label("Path Quality: ");
                if ui
                    .add(egui::Slider::new(&mut self.path_quality, 1..=128))
                    .changed()
                {
                    self.modified_since_save_to_file = true;
                };
            });
            ui.horizontal(|ui| {
                ui.label("Speed: ");
                if ui
                    .add(egui::DragValue::new(&mut self.speed).speed(0.1))
                    .changed()
                {
                    self.modified_since_save_to_file = true;
                }
                if ui.button("Play / Pause").clicked() {
                    self.playing = !self.playing;
                }
                if ui.button("0.1x").clicked() {
                    self.speed = 0.1;
                    self.modified_since_save_to_file = true;
                }
                if ui.button("0.5x").clicked() {
                    self.speed = 0.5;
                    self.modified_since_save_to_file = true;
                }
                if ui.button("1x").clicked() {
                    self.speed = 1.0;
                    self.modified_since_save_to_file = true;
                }
                if ui.button("5x").clicked() {
                    self.speed = 5.0;
                    self.modified_since_save_to_file = true;
                }
                if ui.button("10x").clicked() {
                    self.speed = 10.0;
                    self.modified_since_save_to_file = true;
                }
                if ui.button("50x").clicked() {
                    self.speed = 50.0;
                    self.modified_since_save_to_file = true;
                }
                if ui.button("100x").clicked() {
                    self.speed = 100.0;
                    self.modified_since_save_to_file = true;
                }
                self.speed = self.speed.max(0.0)
            });
            if ui.button("Delete Past").clicked() {
                self.states.drain(..self.current_state);
                self.current_state = 0;
                self.states.shrink_to_fit();
                self.modified_since_save_to_file = true;
            }
            if ui.button("Delete Future").clicked() {
                self.current_state_modified = true;
                self.modified_since_save_to_file = true;
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
                    let [selected, focused] = self.states[self.current_state]
                        .bodies
                        .maybe_get_disjoint_mut([self.selected, self.focused]);
                    let Some(body) = selected else {
                        ui.label("The selected body does not exist in this time :p");
                        return;
                    };
                    let mut delete = false;
                    ui.add_enabled_ui(!self.playing, |ui| {
                        ui.horizontal(|ui| {
                            ui.label("Name:");
                            self.current_state_modified |=
                                ui.text_edit_singleline(&mut body.name).changed();
                        });
                        ui.horizontal(|ui| {
                            ui.label("Position:");
                            self.current_state_modified |= ui
                                .add(
                                    egui::DragValue::new(&mut body.pos.x)
                                        .speed(1.0)
                                        .prefix("x:"),
                                )
                                .changed();
                            self.current_state_modified |= ui
                                .add(
                                    egui::DragValue::new(&mut body.pos.y)
                                        .speed(1.0)
                                        .prefix("y:"),
                                )
                                .changed();
                        });
                        ui.horizontal(|ui| {
                            ui.label("Velocity:");
                            self.current_state_modified |= ui
                                .add(
                                    egui::DragValue::new(&mut body.vel.x)
                                        .speed(0.1)
                                        .prefix("x:"),
                                )
                                .changed();
                            self.current_state_modified |= ui
                                .add(
                                    egui::DragValue::new(&mut body.vel.y)
                                        .speed(0.1)
                                        .prefix("y:"),
                                )
                                .changed();
                        });
                        ui.horizontal(|ui| {
                            ui.label("Radius:");
                            self.current_state_modified |= ui
                                .add(
                                    egui::DragValue::new(&mut body.radius)
                                        .speed(0.1)
                                        .suffix("m"),
                                )
                                .changed();
                        });
                        ui.horizontal(|ui| {
                            ui.label("Density:");
                            self.current_state_modified |= ui
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
                                self.current_state_modified = true;
                                let color: Vector3<f32> = color.into();
                                body.color = color.cast().unwrap();
                            }
                        });
                        if ui.button("Delete").clicked() {
                            self.current_state_modified = true;
                            delete = true;
                        }
                        ui.checkbox(&mut self.auto_orbit, "Auto Orbit");
                        if self.focused.is_none() && self.auto_orbit && !self.playing {
                            ui.label("Focus a body for auto orbit");
                        }
                        if let Some(focus) = focused
                            && self.auto_orbit
                            && !self.playing
                        {
                            let focused_to_body = body.pos - focus.pos;
                            let mut current_height = focused_to_body.magnitude();
                            ui.horizontal(|ui| {
                                ui.label("Current Height:");
                                if ui
                                    .add(egui::DragValue::new(&mut current_height).speed(0.1))
                                    .changed()
                                {
                                    let new_focused_to_body =
                                        focused_to_body.normalize_to(current_height);
                                    body.pos = new_focused_to_body + focus.pos;
                                    self.current_state_modified = true;
                                }
                            });
                            ui.label("Not Finished");
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
                    self.current_state_modified = true
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
        self.modified_since_save_to_file |= self.current_state_modified;
    }

    pub fn world_input(&mut self, response: &egui::Response, rect: egui::Rect, ui: &mut egui::Ui) {
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
            self.current_state_modified = true;
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
    }

    pub fn move_time(&mut self, dt: f64) {
        self.accumulated_time += (dt * self.playing as u8 as f64 * self.speed).max(0.0);
        while self.accumulated_time >= self.step_size {
            if self.current_state + 1 < self.states.len() {
                self.current_state += 1;
            } else {
                break;
            }
            self.accumulated_time -= self.step_size;
        }
    }

    pub fn gen_future(&mut self) {
        let mut lock = self.thread_state.generation_state.lock().unwrap();
        if self.current_state_modified {
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

    pub fn draw_states(&self, d: &mut DrawHandler) {
        self.state().draw(d);
        d.quads.reserve(
            ((self.show_future / self.step_size) as usize)
                .min((self.states.len() as i32 - 2_i32).max(0) as usize)
                * self.state().bodies.len()
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
    }
}
