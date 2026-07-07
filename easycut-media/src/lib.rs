use std::fmt::{Display, Formatter};
use std::path::Path;

use video_rs::decode::Decoder;
use video_rs::{Time};

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
    decoder: Decoder,
}

impl MediaInstance {
    /// TODO: 动态改成Any是不是更好?
    pub fn new(file_path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let decoder = Decoder::new(Path::new(file_path))?;
        Ok(Self {
            file_path: file_path.to_string(),
            decoder,
        })
    }

    /// 获取视频元数据
    pub fn get_meta(&self) -> MediaMeta {
        MediaMeta {
            duration: self
                .decoder
                .duration()
                .unwrap_or(Time::from_secs(0.0))
                .as_secs(),
            width: self.decoder.size().0,
            height: self.decoder.size().1,
            frame_rate: self.decoder.frame_rate(),
        }
    }

    /// 获取指定时间点的帧数据, 保证输出的格式为RGBA
    pub fn frame_at(&mut self, timestamp_second: f64) -> Result<FrameBuffer, video_rs::Error> {
        let timestamp_millisecond = (timestamp_second * 1000.0) as i64;
        self.decoder.seek(timestamp_millisecond)?;
        // HWC
        let (_, raw_frame) = self.decoder.decode()?;
        let height = raw_frame.shape()[0];
        let width = raw_frame.shape()[1];
        let channels = raw_frame.shape()[2];
        let mut rgba_data: Vec<u8>;
        println!("raw_frame.shape(): {:?}", raw_frame.shape());
        match channels {
            3 => {
                rgba_data = Vec::with_capacity(height * width * 4);
                for y in 0..height {
                    for x in 0..width {
                        rgba_data.push(*raw_frame.get((y, x, 0)).unwrap());
                        rgba_data.push(*raw_frame.get((y, x, 1)).unwrap());
                        rgba_data.push(*raw_frame.get((y, x, 2)).unwrap());
                        rgba_data.push(255);
                    }
                }
            }
            4 => {
                rgba_data = raw_frame.into_raw_vec_and_offset().0;
            }
            _ => {
                panic!("Unsupported channel count: {}", channels);
            }
        }
        Ok(FrameBuffer {
            height: height as u32,
            width: width as u32,
            rgba_data: rgba_data,
        })
    }
}
