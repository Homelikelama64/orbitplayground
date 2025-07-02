use crate::{
    drawing::DrawHandler,
    rendering::{GpuCamera, RenderData, RenderState},
    world::World,
};
use cgmath::{Vector2, Zero};
use eframe::{
    egui::{self},
    wgpu,
};
use egui_file_dialog::FileDialog;
use std::sync::Arc;

pub mod body;
pub mod camera;
pub mod drawing;
pub mod rendering;
pub mod save;
pub mod universe;
pub mod world;

struct App {
    last_time: Option<std::time::Instant>,
    lagging: bool,
    stats_open: bool,
    file_dialog: FileDialog,
    file_interaction: FileInteraction,
    help_open: bool,
    worlds: Vec<World>,
    selected_world: usize,
}

enum FileInteraction {
    None,
    Save,
    Load,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> anyhow::Result<Self> {
        let renderer = cc.wgpu_render_state.as_ref().unwrap();
        let state = RenderState::new(renderer.target_format, &renderer.device, &renderer.queue)?;
        renderer.renderer.write().callback_resources.insert(state);

        Ok(Self {
            last_time: None,
            lagging: false,
            stats_open: true,
            file_dialog: FileDialog::new()
                .add_file_filter_extensions("Orbit Save", vec!["orbit"])
                .default_file_filter("Orbit Save")
                .add_save_extension("Orbit Save", "orbit")
                .default_save_extension("Orbit Save"),
            file_interaction: FileInteraction::None,
            help_open: true,
            worlds: vec![World::new(1.0 / 512.0)],
            selected_world: 0,
        })
    }
    fn world(&mut self) -> &mut World {
        self.selected_world = self.selected_world.min(self.worlds.len() - 1);
        &mut self.worlds[self.selected_world]
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let time = std::time::Instant::now();
        let dt = time - self.last_time.unwrap_or(time);
        self.last_time = Some(time);

        let dt = dt.as_secs_f64();

        egui::TopBottomPanel::top("Menu").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.menu_button("File", |ui| {
                    ui.button("New").clicked();
                    ui.button("Save").clicked();
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
            ui.horizontal(|ui| {
                ui.label("Open Worlds: ");
                let mut remove = None;
                for (i, world) in self.worlds.iter().enumerate() {
                    let tab = ui.selectable_label(i == self.selected_world, world.name.as_str());
                    if tab.clicked() {
                        self.selected_world = i
                    }
                    if tab.clicked_by(egui::PointerButton::Middle) || ui.button("-").clicked() {
                        remove = Some(i)
                    }
                }
                if let Some(remove) = remove {
                    self.worlds.remove(remove);
                }
                if ui.button("+").clicked() {
                    self.worlds.push(World::new(1.0 / 512.0));
                }
            })
        });

        self.file_dialog.update(ctx);
        if let Some(_path) = self.file_dialog.take_picked() {
            match core::mem::replace(&mut self.file_interaction, FileInteraction::None) {
                FileInteraction::None => {}
                FileInteraction::Save => {}
                FileInteraction::Load => {}
            }
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

        if self.worlds.is_empty() {
            self.worlds.push(World::new(1.0 / 512.0));
        }

        self.world().ui(ctx, dt);

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(egui::Color32::from_rgb(50, 50, 50)))
            .show(ctx, |ui| {
                let (rect, response) =
                    ui.allocate_exact_size(ui.available_size(), egui::Sense::click_and_drag());
                let aspect = rect.width() / rect.height();

                self.world().world_input(&response, rect, ui);
                self.world().move_time(dt);
                self.world().gen_future();

                let mut d = DrawHandler::new();

                self.world().draw_states(&mut d);

                ui.painter()
                    .add(eframe::egui_wgpu::Callback::new_paint_callback(
                        rect,
                        RenderData {
                            camera: GpuCamera {
                                position: (self.world().camera.pos - self.world().camera.offset)
                                    .cast()
                                    .unwrap(),
                                vertical_height: self.world().camera.view_height as f32,
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
