use std::fs::File;
use std::path::Path;
use std::time::Duration;

use rodio::{Decoder, DeviceSinkBuilder, MixerDeviceSink, Player};
use video_rs::ffmpeg;

/// Audio output for one source file. The player's position is the preview clock.
pub struct AudioPlayback {
    player: Player,
    _device: MixerDeviceSink,
}

impl AudioPlayback {
    /// Returns `None` when the file has no audio stream.
    pub fn open(path: &Path) -> Result<Option<Self>, String> {
        let input = ffmpeg::format::input(path).map_err(|error| error.to_string())?;
        if input.streams().best(ffmpeg::media::Type::Audio).is_none() {
            return Ok(None);
        }
        let file = File::open(path).map_err(|error| error.to_string())?;
        let decoder = Decoder::try_from(file).map_err(|error| error.to_string())?;
        let mut device =
            DeviceSinkBuilder::open_default_sink().map_err(|error| error.to_string())?;
        device.log_on_drop(false);
        let player = Player::connect_new(device.mixer());
        player.pause();
        player.append(decoder);
        Ok(Some(Self {
            player,
            _device: device,
        }))
    }

    pub fn seek(&self, seconds: f32) -> Result<(), String> {
        self.player
            .try_seek(Duration::from_secs_f32(seconds.max(0.0)))
            .map_err(|error| error.to_string())
    }

    pub fn play(&self) {
        self.player.play();
    }

    pub fn pause(&self) {
        self.player.pause();
    }

    pub fn is_paused(&self) -> bool {
        self.player.is_paused()
    }

    pub fn is_finished(&self) -> bool {
        self.player.empty()
    }

    pub fn position(&self) -> f32 {
        self.player.get_pos().as_secs_f32()
    }

    pub fn set_volume(&self, volume: f32) {
        self.player.set_volume(volume);
    }
}

#[cfg(test)]
mod tests {
    use std::fs::File;
    use std::path::PathBuf;
    use std::time::Duration;

    use rodio::{Decoder, Source};

    use super::AudioPlayback;

    fn sample_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("sample_file.mp4")
    }

    #[test]
    fn sample_mp4_audio_decodes_and_seeks() {
        let mut decoder = Decoder::try_from(File::open(sample_path()).unwrap()).unwrap();
        assert!(decoder.sample_rate().get() > 0);
        assert!(decoder.next().is_some());
        decoder.try_seek(Duration::from_secs(18)).unwrap();
        assert!(decoder.next().is_some());
    }

    #[test]
    #[ignore = "requires a local audio output device"]
    fn output_clock_tracks_play_pause_and_seek() {
        let audio = AudioPlayback::open(&sample_path()).unwrap().unwrap();
        audio.play();
        std::thread::sleep(Duration::from_millis(200));
        assert!(audio.position() > 0.05);

        audio.pause();
        std::thread::sleep(Duration::from_millis(100));
        let paused_at = audio.position();
        std::thread::sleep(Duration::from_millis(100));
        assert!((audio.position() - paused_at).abs() < 0.02);

        audio.seek(18.0).unwrap();
        assert!((audio.position() - 18.0).abs() < 0.1);
    }
}
