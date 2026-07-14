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
    pub load_finished: bool,
    rx: Receiver<FrameBuffer>,
}

impl MediaInstance {
    pub fn new(file_path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let mut decoder = Decoder::new(Path::new(file_path))?;
        let meta = Self::get_meta(&decoder);
        let frames = Vec::new();
        let (tx, rx) = mpsc::channel::<FrameBuffer>();
        std::thread::spawn(move || {
            while let Ok((time, raw_frame)) = decoder.decode() {
                let _ = tx.send(Self::frame_from_raw_frame(time.as_secs(), raw_frame));
            }
            println!("Decoder finished ");
        });
        println!("Decoder thread spawned, init success. {}", file_path);
        Ok(Self {
            file_path: file_path.to_string(),
            frames,
            meta,
            load_finished: false,
            rx,
        })
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

    /// 获取指定时间点的帧数据, 保证输出的格式为RGBA
    pub fn frame_at(&mut self, timestamp_second: f32) -> Option<&FrameBuffer> {
        while let Ok(frame) = self.rx.try_recv() {
            self.frames.push(frame);
        }
        if let Err(e) = self.rx.try_recv() {
            if e == TryRecvError::Disconnected {
                self.load_finished = true;
            }
        }
        for _frame in &self.frames {
            if _frame.ts > timestamp_second {
                return Some(_frame);
            }
        }
        None
    }

    fn frame_from_raw_frame(ts: f32, raw_frame: Frame) -> FrameBuffer {
        let height = raw_frame.shape()[0];
        let width = raw_frame.shape()[1];
        let channels = raw_frame.shape()[2];
        let mut rgba_data: Vec<u8>;
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
        FrameBuffer {
            height: height as u32,
            width: width as u32,
            ts: ts,
            rgba_data: rgba_data,
        }
    }
}
