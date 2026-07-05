use std::fmt::{Display, Formatter};

use video_rs::decode::Decoder;
use video_rs::{Time, Url};

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
    file_path: String,
    decoder: Decoder,
}

impl MediaInstance {
    /// TODO: 动态改成Any是不是更好?
    pub fn new(file_path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let url = file_path.parse::<Url>()?;
        let decoder = Decoder::new(url)?;
        Ok(Self {
            file_path: file_path.to_string(),
            decoder,
        })
    }

    /// 获取视频元数据
    pub fn get_meta(&self) -> MediaMeta {
        let meta = MediaMeta {
            duration: self
                .decoder
                .duration()
                .unwrap_or(Time::from_secs(0.0))
                .as_secs(),
            width: self.decoder.size().0,
            height: self.decoder.size().1,
            frame_rate: self.decoder.frame_rate(),
        };
        meta
    }
}
