use crate::{
    drawing::DrawHandler,
    rendering::{GpuCamera, RenderData, RenderState},
};
use cgmath::Vector2;
use eframe::{
    egui::{self},
    wgpu,
};

pub mod drawing;
pub mod rendering;
pub mod body;

struct App {
    camera: Camera,
    last_time: Option<std::time::Instant>,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> anyhow::Result<Self> {
        let renderer = cc.wgpu_render_state.as_ref().unwrap();
        let state = RenderState::new(renderer.target_format, &renderer.device, &renderer.queue)?;
        renderer.renderer.write().callback_resources.insert(state);

        Ok(Self {
            camera: Camera {
                pos: Vector2 { x: 0.0, y: 0.0 },
                offset: Vector2 { x: 0.0, y: 0.0 },
                view_height: 10.0,
                width: 0.0,
                height: 0.0,
            },
            last_time: None,
        })
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Camera {
    pos: Vector2<f64>,
    offset: Vector2<f64>,
    view_height: f64,
    width: f64,
    height: f64,
}

impl Camera {
    pub fn screen_to_world(&self, pos: Vector2<f64>) -> Vector2<f64> {
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
    pub fn world_to_screen(&self, pos: Vector2<f64>) -> Vector2<f64> {
        Vector2 {
            x: (pos.x - self.pos.x - self.offset.x)
                * (self.width / (self.view_height * (self.width / self.height)))
                + self.width * 0.5,
            y: (pos.y - self.pos.y - self.offset.y) * (self.height / self.view_height)
                + self.height * 0.5,
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let time = std::time::Instant::now();
        let dt = time - self.last_time.unwrap_or(time);
        self.last_time = Some(time);

        let dt = dt.as_secs_f64();

        egui::Window::new("Stats").resizable(false).show(ctx, |ui| {
            ui.label(format!("Frame Time: {:.3}ms", 1000.0 * dt));
            ui.label(format!("FPS: {:.3}", 1.0 / dt));
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
                self.camera.view_height -= i.raw_scroll_delta.y as f64 * self.camera.view_height * 0.005;
                self.camera.view_height = self.camera.view_height.max(0.1);
            });
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(egui::Color32::from_rgb(50, 50, 50)))
            .show(ctx, |ui| {
                let (rect, _response) =
                    ui.allocate_exact_size(ui.available_size(), egui::Sense::click_and_drag());
                let aspect = rect.width() / rect.height();
                self.camera.width = rect.width() as f64;
                self.camera.height = rect.height() as f64;

                let d = DrawHandler::new();

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
