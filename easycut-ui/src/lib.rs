use easycut_media::FrameBuffer;

use eframe::egui;
use egui::TextureHandle;

#[derive(Default)]
pub struct MyApp {
    pub frame: Option<FrameBuffer>,
    pub texture_handle: Option<TextureHandle>,
}

impl MyApp {
    pub fn init(self) {
        let frame = self.frame.expect("frame must be set before init");

        let width = frame.width as usize;
        let height = frame.height as usize;
        let options = eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default().with_inner_size([width as f32 * 2.0, height as f32 * 2.0]),
            ..Default::default()
        };
        eframe::run_native(
            "EasyCut",
            options,
            Box::new(move |cc| {
                egui_extras::install_image_loaders(&cc.egui_ctx);
                let color_image =
                    egui::ColorImage::from_rgba_premultiplied([width, height], &frame.rgba_data);
                let texture_handle =
                    cc.egui_ctx
                        .load_texture("frame", color_image, Default::default());

                Ok(Box::new(MyApp {
                    frame: Some(frame),
                    texture_handle: Some(texture_handle),
                }))
            }),
        )
        .unwrap();
    }
}

impl eframe::App for MyApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ui, |ui| {
            if let Some(texture_handle) = self.texture_handle.as_ref() {
                ui.image((texture_handle.id(), texture_handle.size_vec2()));
            }
        });
    }
}
