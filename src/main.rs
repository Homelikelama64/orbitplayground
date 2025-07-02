use crate::{
    drawing::DrawHandler,
    rendering::{GpuCamera, RenderData, RenderState},
    universe::Universe,
};
use cgmath::{Vector2, Zero};
use eframe::{
    egui::{self},
    wgpu,
};
use egui_file_dialog::FileDialog;
use std::sync::{Arc, Condvar, Mutex};

pub mod body;
pub mod camera;
pub mod drawing;
pub mod rendering;
pub mod save;
pub mod universe;

struct App {
    last_time: Option<std::time::Instant>,
    lagging: bool,
    stats_open: bool,
    file_dialog: FileDialog,
    file_interaction: FileInteraction,
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
        })
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let time = std::time::Instant::now();
        let dt = time - self.last_time.unwrap_or(time);
        self.last_time = Some(time);

        let dt = dt.as_secs_f64();
        let mut _current_state_modified = false;

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

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(egui::Color32::from_rgb(50, 50, 50)))
            .show(ctx, |ui| {
                let (rect, _response) =
                    ui.allocate_exact_size(ui.available_size(), egui::Sense::click_and_drag());
                let aspect = rect.width() / rect.height();

                let d = DrawHandler::new();

                ui.painter()
                    .add(eframe::egui_wgpu::Callback::new_paint_callback(
                        rect,
                        RenderData {
                            camera: GpuCamera {
                                position: Vector2::zero(),
                                vertical_height: 1.0,
                                aspect,
                            },
                            quads: d.quads,
                            circles: d.circles,
                        },
                    ));
            });

        ctx.request_repaint();
    }

    fn save(&mut self, _storage: &mut dyn eframe::Storage) {
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
