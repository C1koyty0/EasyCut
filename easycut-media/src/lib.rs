use std::fmt::{Display, Formatter};
use std::path::Path;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use video_rs::decode::Decoder;
use video_rs::{Frame, Time};
pub struct MediaMeta {
    /// 视频时长, 单位s
    pub duration: f32,
    /// 视频宽度
    pub width: u32,
    /// 视频高度
    pub height: u32,
    /// 视频帧率
    pub frame_rate: f32,
}

/// 保证输出的是rgba格式
pub struct FrameBuffer {
    pub width: u32,
    pub height: u32,
    pub ts: f32,
    pub rgba_data: Vec<u8>,
}

impl Display for MediaMeta {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(
            f,
            "Duration: {}s, Width: {}, Height: {}, Frame Rate: {}",
            self.duration, self.width, self.height, self.frame_rate
        )
    }
}

pub struct MediaInstance {
    pub file_path: String,
    pub frames: Vec<FrameBuffer>,
    pub meta: MediaMeta,
    #[deprecated]
    pub load_finished: bool,
    rx: Receiver<FrameBuffer>,
    decoder: Decoder,
}

impl MediaInstance {
    pub fn new(file_path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let mut decoder = Decoder::new(Path::new(file_path))?;
        let meta = Self::get_meta(&decoder);
        todo!()
    }

    pub fn load_frames(&mut self, start_ts_ms: i64, end_ts_ms: i64) {
        self.decoder.seek(start_ts_ms);
    }

    /// 获取视频元数据
    fn get_meta(decoder: &Decoder) -> MediaMeta {
        MediaMeta {
            duration: decoder.duration().unwrap_or(Time::from_secs(0.0)).as_secs(),
            width: decoder.size().0,
            height: decoder.size().1,
            frame_rate: decoder.frame_rate(),
        }
    }

    fn frame_from_raw_frame(ts: f32, raw_frame: Frame) -> FrameBuffer {
        let height = raw_frame.shape()[0];
        let width = raw_frame.shape()[1];
        let mut rgba_data = Vec::with_capacity(height * width * 4);
        let rgb = raw_frame.as_slice().unwrap();
        for pix in rgb.chunks_exact(3) {
            rgba_data.extend_from_slice(&[pix[0], pix[1], pix[2], 255]);
        }
        FrameBuffer {
            height: height as u32,
            width: width as u32,
            ts: ts,
            rgba_data: rgba_data,
        }
    }
}
