use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use easycut_media::export_clip_with_progress;
use easycut_media::{AudioPlayback, MediaSession, MediaState};
use eframe::egui;
use egui::TextureHandle;

enum ExportEvent {
    Progress(f32),
    Finished {
        output: PathBuf,
        result: Result<(), String>,
    },
}

fn default_export_filename(input: &Path, duration: f32) -> String {
    let stem = input
        .file_stem()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("clip");
    format!("{stem}-{duration:.2}s.mp4")
}

fn playback_time(
    from: f32,
    elapsed: f32,
    duration: f32,
    latest_frame: f32,
    frame_interval: f32,
    ended: bool,
) -> (f32, bool) {
    let proposed = (from + elapsed).min(duration);
    let time = if ended {
        proposed
    } else {
        proposed.min(latest_frame + frame_interval)
    };
    (time, time < proposed)
}

fn audio_needs_buffering(audio_time: f32, latest_frame: f32, frame_interval: f32) -> bool {
    audio_time > latest_frame + frame_interval
}

fn audio_can_resume(audio_time: f32, latest_frame: f32, ended: bool) -> bool {
    ended || latest_frame >= audio_time + 0.1
}

pub struct MyApp {
    media: Option<MediaSession>,
    audio: Option<AudioPlayback>,
    audio_error: Option<String>,
    volume: f32,
    texture_handle: Option<TextureHandle>,
    file_path: Option<PathBuf>,
    current_time: f32,
    playback_start: Option<(Instant, f32)>,
    playing: bool,
    displayed_ts: Option<f32>,
    open_error: Option<String>,
    export_start: f32,
    export_end: Option<f32>,
    export_status: Option<String>,
    export_progress: Option<f32>,
    export_rx: Option<Receiver<ExportEvent>>,
}

impl Default for MyApp {
    fn default() -> Self {
        Self {
            media: None,
            audio: None,
            audio_error: None,
            volume: 1.0,
            texture_handle: None,
            file_path: None,
            current_time: 0.0,
            playback_start: None,
            playing: false,
            displayed_ts: None,
            open_error: None,
            export_start: 0.0,
            export_end: None,
            export_status: None,
            export_progress: None,
            export_rx: None,
        }
    }
}

impl MyApp {
    pub fn init() {
        let options = eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default().with_inner_size([1080.0, 720.0]),
            ..Default::default()
        };
        eframe::run_native(
            "EasyCut",
            options,
            Box::new(|_| Ok(Box::new(Self::default()))),
        )
        .expect("Failed to start EasyCut");
    }

    fn open(&mut self, path: PathBuf) {
        self.media = None;
        self.audio = None;
        self.audio_error = None;
        self.texture_handle = None;
        self.displayed_ts = None;
        self.playback_start = None;
        self.playing = false;
        self.current_time = 0.0;
        self.export_start = 0.0;
        self.export_end = None;
        self.open_error = None;
        self.export_status = None;
        self.file_path = Some(path.clone());
        match MediaSession::open(path.clone()) {
            Ok(media) => {
                self.media = Some(media);
                match AudioPlayback::open(&path) {
                    Ok(audio) => {
                        if let Some(audio) = &audio {
                            audio.set_volume(self.volume);
                        }
                        self.audio = audio;
                    }
                    Err(error) => self.audio_error = Some(error),
                }
            }
            Err(error) => self.open_error = Some(error.to_string()),
        }
    }

    fn update_playback_clock(&mut self) {
        if !self.playing {
            return;
        }
        let Some(media) = self.media.as_ref() else {
            return;
        };
        if media.state == MediaState::Failed {
            if let Some(audio) = &self.audio {
                audio.pause();
            }
            self.playing = false;
            self.playback_start = None;
            return;
        }
        let Some(meta) = media.meta.as_ref() else {
            return;
        };
        if !matches!(media.state, MediaState::Ready | MediaState::Ended)
            || media.frame_at(self.current_time).is_none()
        {
            if let Some(audio) = &self.audio {
                audio.pause();
            }
            self.playback_start = None;
            return;
        }
        let now = Instant::now();
        let latest = media.latest_frame_time().unwrap_or(self.current_time);
        let frame_interval = 1.0 / meta.frame_rate.clamp(1.0, 120.0);
        if self.audio.as_ref().is_some_and(AudioPlayback::is_finished) {
            self.audio = None;
            self.playback_start = None;
        }
        if let Some(audio) = &self.audio {
            let audio_time = audio.position();
            let ended = media.state == MediaState::Ended;
            if !ended && audio_needs_buffering(audio_time, latest, frame_interval) {
                audio.pause();
            } else if audio.is_paused() && audio_can_resume(audio_time, latest, ended) {
                audio.play();
            }
            self.current_time = if ended {
                audio_time.min(meta.duration)
            } else {
                audio_time.min(meta.duration).min(latest + frame_interval)
            };
        } else {
            let (started, from) = *self.playback_start.get_or_insert((now, self.current_time));
            let (time, stalled) = playback_time(
                from,
                now.duration_since(started).as_secs_f32(),
                meta.duration,
                latest,
                frame_interval,
                media.state == MediaState::Ended,
            );
            self.current_time = time;
            if stalled {
                self.playback_start = Some((now, self.current_time));
            }
        }
        if self.current_time >= meta.duration {
            if let Some(audio) = &self.audio {
                audio.pause();
            }
            self.playback_start = None;
            self.playing = false;
        }
    }

    fn update_texture(&mut self, ctx: &egui::Context) {
        let Some(media) = self.media.as_ref() else {
            return;
        };
        let Some(frame) = media.frame_at(self.current_time) else {
            return;
        };
        if self.displayed_ts == Some(frame.ts) {
            return;
        }
        let image = egui::ColorImage::from_rgba_unmultiplied(
            [frame.width as usize, frame.height as usize],
            &frame.rgba_data,
        );
        if let Some(texture) = self.texture_handle.as_mut() {
            texture.set(image, egui::TextureOptions::LINEAR);
        } else {
            self.texture_handle =
                Some(ctx.load_texture("video-preview", image, egui::TextureOptions::LINEAR));
        }
        self.displayed_ts = Some(frame.ts);
    }

    fn poll_export(&mut self) {
        let Some(rx) = &self.export_rx else { return };
        let mut finished = None;
        while let Ok(event) = rx.try_recv() {
            match event {
                ExportEvent::Progress(progress) => {
                    self.export_progress = Some(progress.clamp(0.0, 1.0));
                }
                ExportEvent::Finished { output, result } => {
                    finished = Some((output, result));
                    break;
                }
            }
        }
        if let Some((output, result)) = finished {
            self.export_status = Some(match &result {
                Ok(()) => format!("Exported {}", output.display()),
                Err(error) => error.clone(),
            });
            self.export_progress = result.is_ok().then_some(1.0);
            self.export_rx = None;
        }
    }
}

impl eframe::App for MyApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        if let Some(media) = self.media.as_mut()
            && media.poll()
        {
            ui.ctx().request_repaint();
        }
        self.update_playback_clock();
        self.poll_export();
        if self.playing
            && let Some(media) = self.media.as_mut()
        {
            media.prefetch((self.current_time * 1_000.0) as i64);
        }
        self.update_texture(ui.ctx());

        egui::CentralPanel::default().show(ui, |ui| {
            ui.horizontal(|ui| {
                if ui.button("Open video").clicked()
                    && let Some(path) = rfd::FileDialog::new()
                        .add_filter("Video", &["mp4", "mov", "mkv", "webm"])
                        .pick_file()
                {
                    self.open(path);
                }
                if let Some(path) = &self.file_path {
                    ui.label(path.file_name().map_or_else(
                        || path.display().to_string(),
                        |name| name.to_string_lossy().into_owned(),
                    ));
                }
            });
            ui.separator();

            let Some(media) = self.media.as_mut() else {
                if let Some(error) = &self.open_error {
                    ui.colored_label(egui::Color32::LIGHT_RED, error);
                }
                return;
            };

            let duration = media.meta.as_ref().map_or(0.0, |meta| meta.duration);
            ui.horizontal(|ui| {
                let playing = self.playing;
                let button = ui
                    .add_enabled(
                        media.meta.is_some() && media.state != MediaState::Failed,
                        egui::Button::new(if playing { "⏸" } else { "▶" })
                            .min_size([32.0, 28.0].into()),
                    )
                    .on_hover_text(if playing { "Pause" } else { "Play" });
                if button.clicked() {
                    if playing {
                        self.playing = false;
                        self.playback_start = None;
                        if let Some(audio) = &self.audio {
                            audio.pause();
                        }
                    } else {
                        if self.current_time >= duration {
                            self.current_time = 0.0;
                            media.seek(0);
                            if self.audio.is_none()
                                && self.audio_error.is_none()
                                && let Some(path) = &self.file_path
                            {
                                match AudioPlayback::open(path) {
                                    Ok(audio) => {
                                        if let Some(audio) = &audio {
                                            audio.set_volume(self.volume);
                                        }
                                        self.audio = audio;
                                    }
                                    Err(error) => self.audio_error = Some(error),
                                }
                            }
                            if let Some(audio) = &self.audio
                                && let Err(error) = audio.seek(0.0)
                            {
                                self.audio_error = Some(error);
                                self.audio = None;
                            }
                        }
                        self.playing = true;
                        self.playback_start = None;
                    }
                }
                ui.label(format!(
                    "{:.2} / {:.2} s",
                    self.displayed_ts.unwrap_or(self.current_time),
                    duration
                ));
                let slider = ui.add_enabled(
                    duration > 0.0,
                    egui::Slider::new(&mut self.current_time, 0.0..=duration).show_value(false),
                );
                if slider.changed() {
                    self.playback_start = None;
                    media.seek((self.current_time * 1_000.0) as i64);
                    if let Some(audio) = &self.audio {
                        audio.pause();
                        if let Err(error) = audio.seek(self.current_time) {
                            self.audio_error = Some(error);
                            self.audio = None;
                        }
                    }
                }
            });

            if let Some(audio) = &self.audio {
                ui.horizontal(|ui| {
                    ui.label("Volume");
                    if ui
                        .add(egui::Slider::new(&mut self.volume, 0.0..=1.0).show_value(false))
                        .changed()
                    {
                        audio.set_volume(self.volume);
                    }
                });
            }
            if let Some(error) = &self.audio_error {
                ui.colored_label(egui::Color32::YELLOW, format!("Audio unavailable: {error}"));
            }

            ui.horizontal(|ui| {
                if ui.button("Set in").clicked() {
                    self.export_start = self.current_time.min(duration);
                    self.export_end = self.export_end.map(|end| end.max(self.export_start));
                }
                if ui.button("Set out").clicked() {
                    self.export_end = Some(self.current_time.min(duration));
                }
                let end = self.export_end.unwrap_or(duration);
                ui.label(format!(
                    "Export range {:.2} - {:.2} s",
                    self.export_start, end
                ));
                let exporting = self.export_rx.is_some();
                if ui
                    .add_enabled(
                        !exporting && end > self.export_start,
                        egui::Button::new("Export MP4"),
                    )
                    .clicked()
                    && let Some(input) = self.file_path.clone()
                    && let Some(mut output) = rfd::FileDialog::new()
                        .add_filter("MP4 video", &["mp4"])
                        .set_file_name(default_export_filename(&input, end - self.export_start))
                        .save_file()
                {
                    output.set_extension("mp4");
                    let start = self.export_start;
                    let (tx, rx) = mpsc::channel();
                    self.export_rx = Some(rx);
                    self.export_progress = Some(0.0);
                    self.export_status = Some("Exporting...".into());
                    std::thread::spawn(move || {
                        let progress_tx = tx.clone();
                        let result = export_clip_with_progress(
                            &input,
                            &output,
                            start,
                            end,
                            move |progress| {
                                let _ = progress_tx.send(ExportEvent::Progress(progress));
                            },
                        );
                        let _ = tx.send(ExportEvent::Finished { output, result });
                    });
                }
            });
            if let Some(progress) = self.export_progress {
                ui.add(egui::ProgressBar::new(progress).show_percentage());
            }
            if let Some(status) = &self.export_status {
                ui.label(status);
            }

            ui.horizontal(|ui| {
                ui.set_min_height(20.0);
                match media.state {
                    MediaState::Opening | MediaState::Buffering => {
                        ui.add(egui::Spinner::new());
                        ui.label("Loading preview");
                    }
                    MediaState::Failed => {
                        ui.colored_label(
                            egui::Color32::LIGHT_RED,
                            media.error.as_deref().unwrap_or("Could not decode video"),
                        );
                    }
                    MediaState::Ready | MediaState::Ended => {}
                }
            });

            if let Some(texture) = &self.texture_handle {
                let available = ui.available_size();
                let source = texture.size_vec2();
                let scale = (available.x / source.x)
                    .min(available.y / source.y)
                    .clamp(0.0, 1.0);
                let size = source * scale;
                ui.vertical_centered(|ui| {
                    ui.image((texture.id(), size));
                });
            }
        });

        let interval = if self.playing
            || self.media.as_ref().is_some_and(|media| {
                matches!(media.state, MediaState::Opening | MediaState::Buffering)
            }) {
            Duration::from_millis(16)
        } else if self.export_rx.is_some() {
            Duration::from_millis(50)
        } else {
            Duration::from_millis(100)
        };
        ui.ctx().request_repaint_after(interval);
    }
}

#[cfg(test)]
mod tests {
    use super::{audio_can_resume, audio_needs_buffering, default_export_filename, playback_time};
    use std::path::Path;

    #[test]
    fn playback_clock_waits_for_a_late_frame_and_resumes_from_it() {
        let (stalled_at, stalled) = playback_time(5.0, 0.5, 10.0, 5.0, 0.04, false);
        assert!(stalled);
        assert!((stalled_at - 5.04).abs() < 0.001);

        let (resumed_at, stalled) = playback_time(stalled_at, 0.03, 10.0, 5.12, 0.04, false);
        assert!(!stalled);
        assert!((resumed_at - 5.07).abs() < 0.001);
    }

    #[test]
    fn sixty_fps_playback_clock_advances_one_second_per_second() {
        let (time, stalled) = playback_time(0.0, 1.0, 10.0, 1.5, 1.0 / 60.0, false);
        assert!(!stalled);
        assert_eq!(time, 1.0);
    }

    #[test]
    fn ended_stream_can_advance_to_duration() {
        let (time, stalled) = playback_time(9.8, 0.5, 10.0, 9.8, 0.04, true);
        assert_eq!(time, 10.0);
        assert!(!stalled);
    }

    #[test]
    fn audio_waits_for_video_buffer_before_resuming() {
        assert!(audio_needs_buffering(5.2, 5.1, 1.0 / 30.0));
        assert!(!audio_can_resume(5.2, 5.25, false));
        assert!(audio_can_resume(5.2, 5.31, false));
        assert!(audio_can_resume(5.2, 5.2, true));
    }

    #[test]
    fn export_name_uses_source_stem_and_duration_in_seconds() {
        assert_eq!(
            default_export_filename(Path::new("/tmp/my-video.mov"), 12.5),
            "my-video-12.50s.mp4"
        );
    }
}
