use easycut_media::MediaInstance;

use eframe::egui;
use egui::TextureHandle;

#[derive(Default)]
pub struct MyApp {
    pub media: Option<MediaInstance>,
    pub texture_handle: Option<TextureHandle>,
    pub file_path: String,
    pub current_time: f32,
    pub is_playing: bool,
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
                    is_playing: false,
                }))
            }),
        )
        .unwrap();
    }
}

impl eframe::App for MyApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ui, |ui| {
            if ui.button("Open file").clicked()
                && let Some(path) = rfd::FileDialog::new().pick_file()
            {
                println!("select file: {}", path.to_string_lossy());
                self.file_path = path.to_string_lossy().to_string();
                self.media = Some(MediaInstance::new(&self.file_path).unwrap());
            }
            if let None = self.media {
                return
            }
            self.media.as_mut().unwrap().frame_at(0.0);
            if self.media.as_ref().unwrap().load_finished{
                if ui.button("Play").clicked() && !self.file_path.is_empty() {
                    self.is_playing = !self.is_playing;
                }
                ui.add(
                    egui::Slider::new(
                        &mut self.current_time,
                        0.0..=self.media.as_ref().map_or(0.0, |m| m.meta.duration),
                    )
                    .text("current_time"),
                );
    
                if let Some(media) = self.media.as_mut() {
                    match media.frame_at(self.current_time) {
                        Some(frame) => {
                            let color_image = egui::ColorImage::from_rgba_unmultiplied(
                                [
                                    frame.width as usize,
                                    frame.height as usize,
                                ],
                                &frame.rgba_data,
                            );
                            let texture_handle =
                                ui.ctx()
                                    .load_texture("frame", color_image, Default::default());
                            ui.image((texture_handle.id(), texture_handle.size_vec2()));
                        },
                        None => {
                            println!("No frame found");
                        }
                    }
                }
            } else {
                ui.add(egui::Spinner::new());
                return;
            }
        });
    }
}
