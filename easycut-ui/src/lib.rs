use easycut_media::MediaInstance;

use eframe::egui;
use egui::TextureHandle;

#[derive(Default)]
pub struct MyApp {
    pub media: Option<MediaInstance>,
    pub texture_handle: Option<TextureHandle>,
    pub file_path: String,
    pub current_time: f32,
}

impl MyApp {
    pub fn init() {
        let options = eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default().with_inner_size([720.0, 1080.0]),
            ..Default::default()
        };
        eframe::run_native(
            "EasyCut",
            options,
            Box::new(move |cc| {
                egui_extras::install_image_loaders(&cc.egui_ctx);

                Ok(Box::new(MyApp {
                    media: None,
                    texture_handle: None,
                    file_path: String::new(),
                    current_time: 0.0,
                }))
            }),
        )
        .unwrap();
    }
}

impl eframe::App for MyApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ui, |ui| {
            if ui.button("Open file").clicked() && let Some(path) = rfd::FileDialog::new().pick_file() {
                self.file_path = path.to_string_lossy().to_string();
                self.media = Some(MediaInstance::new(&self.file_path).unwrap());
            }
            ui.add(egui::Slider::new(&mut self.current_time, 0.0..=self.media.as_ref().map_or(0.0, |m| {m.get_meta().duration})).text("current_time"));

            if let Some(media) = self.media.as_mut() {
                let frame = media.frame_at(self.current_time).unwrap();
                let color_image = egui::ColorImage::from_rgba_unmultiplied([media.get_meta().width as usize, media.get_meta().height as usize], &frame.rgba_data);
                let texture_handle = ui.ctx().load_texture("frame", color_image, Default::default());
                ui.image((texture_handle.id(), texture_handle.size_vec2()));
            }
        });
    }

}
