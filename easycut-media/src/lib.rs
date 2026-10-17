use std::borrow::Cow;
use std::collections::VecDeque;
use std::fmt::{Display, Formatter};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender, SyncSender, TryRecvError, TrySendError};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use video_rs::decode::DecoderBuilder;
use video_rs::resize::Resize;
use video_rs::{Error as VideoError, Frame, Time};

mod audio;
pub use audio::AudioPlayback;
mod export;
pub use export::{ExportResult, export_clip, export_clip_with_progress};

const CACHE_LIMIT_BYTES: usize = 64 * 1024 * 1024;
const PREFETCH_MS: i64 = 250;
const EVENT_CAPACITY: usize = 4;

/// Source metadata, before the preview image is resized.
#[derive(Clone, Debug)]
pub struct MediaMeta {
    pub duration: f32,
    pub width: u32,
    pub height: u32,
    pub frame_rate: f32,
}

impl Display for MediaMeta {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Duration: {}s, Width: {}, Height: {}, Frame Rate: {}",
            self.duration, self.width, self.height, self.frame_rate
        )
    }
}

/// A preview frame with unpremultiplied RGBA pixels.
#[derive(Debug)]
pub struct FrameBuffer {
    pub width: u32,
    pub height: u32,
    pub ts: f32,
    pub rgba_data: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MediaState {
    Opening,
    Buffering,
    Ready,
    Ended,
    Failed,
}

enum Command {
    Seek {
        generation: u64,
        target_ms: i64,
        until_ms: i64,
    },
    Prefetch {
        generation: u64,
        until_ms: i64,
    },
    Stop,
}

enum Event {
    Opened(MediaMeta),
    Frame { generation: u64, frame: FrameBuffer },
    SeekReady { generation: u64 },
    EndOfStream { generation: u64 },
    Failed { generation: u64, message: String },
}

/// UI-side handle for a decoder owned by one background thread.
pub struct MediaSession {
    pub meta: Option<MediaMeta>,
    pub state: MediaState,
    pub error: Option<String>,
    frames: VecDeque<FrameBuffer>,
    cached_bytes: usize,
    generation: u64,
    requested_until_ms: i64,
    tx: Sender<Command>,
    rx: Receiver<Event>,
    worker: Option<JoinHandle<()>>,
}

impl MediaSession {
    pub fn open(path: PathBuf) -> std::io::Result<Self> {
        let (tx, commands) = mpsc::channel();
        let (events, rx) = mpsc::sync_channel(EVENT_CAPACITY);
        let worker = thread::Builder::new()
            .name("easycut-decoder".into())
            .spawn(move || run_worker(path, commands, events))?;
        let mut session = Self {
            meta: None,
            state: MediaState::Opening,
            error: None,
            frames: VecDeque::new(),
            cached_bytes: 0,
            generation: 0,
            requested_until_ms: 0,
            tx,
            rx,
            worker: Some(worker),
        };
        session.seek(0);
        if session.state != MediaState::Failed {
            session.state = MediaState::Opening;
        }
        Ok(session)
    }

    /// Request a new position and invalidate frames from the preceding request.
    pub fn seek(&mut self, target_ms: i64) {
        let target_ms = target_ms.max(0);
        self.generation = self.generation.wrapping_add(1);
        self.frames.clear();
        self.cached_bytes = 0;
        self.error = None;
        self.state = MediaState::Buffering;
        self.requested_until_ms = target_ms.saturating_add(PREFETCH_MS);
        if self
            .tx
            .send(Command::Seek {
                generation: self.generation,
                target_ms,
                until_ms: self.requested_until_ms,
            })
            .is_err()
        {
            self.fail("Decoder thread stopped".into());
        }
    }

    /// Extend the short decode window ahead of the current playback position.
    pub fn prefetch(&mut self, target_ms: i64) {
        if !matches!(self.state, MediaState::Ready | MediaState::Buffering) {
            return;
        }
        let until_ms = target_ms.saturating_add(PREFETCH_MS);
        if until_ms <= self.requested_until_ms {
            return;
        }
        self.requested_until_ms = until_ms;
        if self
            .tx
            .send(Command::Prefetch {
                generation: self.generation,
                until_ms,
            })
            .is_err()
        {
            self.fail("Decoder thread stopped".into());
        }
    }

    /// Process at most one channel's worth of events per UI pass.
    pub fn poll(&mut self) -> bool {
        let mut changed = false;
        for _ in 0..EVENT_CAPACITY {
            match self.rx.try_recv() {
                Ok(Event::Opened(meta)) => {
                    self.meta = Some(meta);
                    if self.state == MediaState::Opening {
                        self.state = MediaState::Buffering;
                    }
                    changed = true;
                }
                Ok(Event::Frame { generation, frame }) if generation == self.generation => {
                    self.cached_bytes += frame.rgba_data.len();
                    self.frames.push_back(frame);
                    self.trim_cache();
                    changed = true;
                }
                Ok(Event::SeekReady { generation }) if generation == self.generation => {
                    self.state = MediaState::Ready;
                    changed = true;
                }
                Ok(Event::EndOfStream { generation }) if generation == self.generation => {
                    self.state = MediaState::Ended;
                    changed = true;
                }
                Ok(Event::Failed {
                    generation,
                    message,
                }) if generation == self.generation => {
                    self.fail(message);
                    changed = true;
                }
                Ok(_) => {}
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    if !matches!(self.state, MediaState::Failed | MediaState::Ended) {
                        self.fail("Decoder thread stopped unexpectedly".into());
                        changed = true;
                    }
                    break;
                }
            }
        }
        changed
    }

    pub fn frame_at(&self, target_seconds: f32) -> Option<&FrameBuffer> {
        if !matches!(self.state, MediaState::Ready | MediaState::Ended) {
            return None;
        }
        self.frames
            .iter()
            .rev()
            .find(|frame| frame.ts <= target_seconds + 0.000_1)
            .or_else(|| self.frames.front())
    }

    /// PTS of the newest frame available for the active seek.
    pub fn latest_frame_time(&self) -> Option<f32> {
        self.frames.back().map(|frame| frame.ts)
    }

    pub fn contains_time(&self, target_seconds: f32) -> bool {
        let (Some(first), Some(last)) = (self.frames.front(), self.frames.back()) else {
            return false;
        };
        first.ts <= target_seconds && target_seconds <= last.ts + 1.0 / 15.0
    }

    fn trim_cache(&mut self) {
        trim_cache_to(&mut self.frames, &mut self.cached_bytes, CACHE_LIMIT_BYTES);
    }

    fn fail(&mut self, message: String) {
        self.error = Some(message);
        self.state = MediaState::Failed;
    }
}

fn trim_cache_to(frames: &mut VecDeque<FrameBuffer>, bytes: &mut usize, limit: usize) {
    while *bytes > limit && frames.len() > 1 {
        if let Some(frame) = frames.pop_front() {
            *bytes -= frame.rgba_data.len();
        }
    }
}

impl Drop for MediaSession {
    fn drop(&mut self) {
        let _ = self.tx.send(Command::Stop);
        // A running decode cannot be interrupted; joining here would block the UI.
        self.worker.take();
    }
}

struct Request {
    generation: u64,
    target_ms: i64,
    start_ms: i64,
    until_ms: i64,
    ready_sent: bool,
    paused: bool,
    previous: Option<(f32, Frame)>,
}

fn next_command(
    commands: &Receiver<Command>,
    pending: &mut Option<Command>,
    wait: bool,
) -> Option<Command> {
    let mut command = pending.take();
    if command.is_none() {
        command = match if wait {
            commands.recv().map_err(|_| TryRecvError::Disconnected)
        } else {
            commands.try_recv()
        } {
            Ok(command) => Some(command),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(Command::Stop),
        };
    }
    while let Ok(next) = commands.try_recv() {
        match next {
            Command::Stop => return Some(Command::Stop),
            Command::Seek { .. } => command = Some(next),
            Command::Prefetch { .. } if !matches!(command, Some(Command::Seek { .. })) => {
                command = Some(next);
            }
            Command::Prefetch { .. } => {}
        }
    }
    command
}

// A full result queue must not prevent the worker from observing a new Seek or Stop.
fn send_event(
    mut event: Event,
    events: &SyncSender<Event>,
    commands: &Receiver<Command>,
    pending: &mut Option<Command>,
) -> bool {
    loop {
        match events.try_send(event) {
            Ok(()) => return true,
            Err(TrySendError::Disconnected(_)) => {
                *pending = Some(Command::Stop);
                return false;
            }
            Err(TrySendError::Full(unsent)) => event = unsent,
        }
        match commands.try_recv() {
            Ok(command @ (Command::Seek { .. } | Command::Stop)) => {
                *pending = Some(command);
                return false;
            }
            Ok(command @ Command::Prefetch { .. }) => *pending = Some(command),
            Err(TryRecvError::Disconnected) => {
                *pending = Some(Command::Stop);
                return false;
            }
            Err(TryRecvError::Empty) => thread::sleep(Duration::from_millis(2)),
        }
    }
}

fn run_worker(path: PathBuf, commands: Receiver<Command>, events: SyncSender<Event>) {
    let mut decoder = match DecoderBuilder::new(path.as_path())
        .with_resize(Resize::Fit(960, 540))
        .build()
    {
        Ok(decoder) => decoder,
        Err(error) => {
            let _ = events.send(Event::Failed {
                generation: 1,
                message: error.to_string(),
            });
            return;
        }
    };
    let meta = MediaMeta {
        duration: decoder.duration().unwrap_or(Time::from_secs(0.0)).as_secs(),
        width: decoder.size().0,
        height: decoder.size().1,
        frame_rate: decoder.frame_rate(),
    };
    if events.send(Event::Opened(meta)).is_err() {
        return;
    }
    let mut request: Option<Request> = None;
    let mut pending = None;
    loop {
        let wait = request.as_ref().is_none_or(|active| active.paused);
        if let Some(command) = next_command(&commands, &mut pending, wait) {
            match command {
                Command::Stop => return,
                Command::Seek {
                    generation,
                    target_ms,
                    until_ms,
                } => {
                    // FFmpeg seeks to nearby keyframes. Decode forward to the target PTS.
                    let start_ms = target_ms.saturating_sub(1_000);
                    let result = if start_ms == 0 {
                        decoder.seek_to_start()
                    } else {
                        decoder.seek(start_ms)
                    };
                    if let Err(error) = result {
                        let _ = send_event(
                            Event::Failed {
                                generation,
                                message: error.to_string(),
                            },
                            &events,
                            &commands,
                            &mut pending,
                        );
                        request = None;
                        continue;
                    }
                    request = Some(Request {
                        generation,
                        target_ms,
                        start_ms,
                        until_ms,
                        ready_sent: false,
                        paused: false,
                        previous: None,
                    });
                }
                Command::Prefetch {
                    generation,
                    until_ms,
                } => {
                    if let Some(active) = request.as_mut()
                        && active.generation == generation
                    {
                        active.until_ms = active.until_ms.max(until_ms);
                        active.paused = false;
                    }
                }
            }
        }
        let Some(active) = request.as_mut() else {
            continue;
        };
        if active.paused {
            continue;
        }
        match decoder.decode() {
            Ok((time, raw)) => {
                if !time.has_value() || time.has_no_pts() {
                    continue;
                }
                let ts_ms = (time.as_secs() * 1_000.0).round() as i64;
                if ts_ms < active.target_ms {
                    active.previous = Some((time.as_secs(), raw));
                    continue;
                }
                let frame = match frame_from_raw_frame(time.as_secs(), raw) {
                    Ok(frame) => frame,
                    Err(message) => {
                        let _ = send_event(
                            Event::Failed {
                                generation: active.generation,
                                message,
                            },
                            &events,
                            &commands,
                            &mut pending,
                        );
                        request = None;
                        continue;
                    }
                };
                if !active.ready_sent
                    && let Some((previous_ts, previous_raw)) = active.previous.take()
                {
                    let previous = match frame_from_raw_frame(previous_ts, previous_raw) {
                        Ok(frame) => frame,
                        Err(message) => {
                            let _ = send_event(
                                Event::Failed {
                                    generation: active.generation,
                                    message,
                                },
                                &events,
                                &commands,
                                &mut pending,
                            );
                            request = None;
                            continue;
                        }
                    };
                    if !send_event(
                        Event::Frame {
                            generation: active.generation,
                            frame: previous,
                        },
                        &events,
                        &commands,
                        &mut pending,
                    ) {
                        continue;
                    }
                }
                if !send_event(
                    Event::Frame {
                        generation: active.generation,
                        frame,
                    },
                    &events,
                    &commands,
                    &mut pending,
                ) {
                    continue;
                }
                if !active.ready_sent {
                    active.ready_sent = true;
                    if !send_event(
                        Event::SeekReady {
                            generation: active.generation,
                        },
                        &events,
                        &commands,
                        &mut pending,
                    ) {
                        continue;
                    }
                }
                if ts_ms >= active.until_ms {
                    active.paused = true;
                }
            }
            Err(VideoError::DecodeExhausted) => {
                if !active.ready_sent && active.previous.is_none() && active.start_ms > 0 {
                    // A seek near EOF may land on frames whose packet DTS is missing.
                    active.start_ms = active.start_ms.saturating_sub(5_000);
                    let result = if active.start_ms == 0 {
                        decoder.seek_to_start()
                    } else {
                        decoder.seek(active.start_ms)
                    };
                    if let Err(error) = result {
                        let _ = send_event(
                            Event::Failed {
                                generation: active.generation,
                                message: error.to_string(),
                            },
                            &events,
                            &commands,
                            &mut pending,
                        );
                        request = None;
                    }
                    continue;
                }
                if let Some((previous_ts, previous_raw)) = active.previous.take() {
                    let previous = match frame_from_raw_frame(previous_ts, previous_raw) {
                        Ok(frame) => frame,
                        Err(message) => {
                            let _ = send_event(
                                Event::Failed {
                                    generation: active.generation,
                                    message,
                                },
                                &events,
                                &commands,
                                &mut pending,
                            );
                            request = None;
                            continue;
                        }
                    };
                    if !send_event(
                        Event::Frame {
                            generation: active.generation,
                            frame: previous,
                        },
                        &events,
                        &commands,
                        &mut pending,
                    ) {
                        continue;
                    }
                }
                if !active.ready_sent
                    && !send_event(
                        Event::SeekReady {
                            generation: active.generation,
                        },
                        &events,
                        &commands,
                        &mut pending,
                    )
                {
                    continue;
                }
                let _ = send_event(
                    Event::EndOfStream {
                        generation: active.generation,
                    },
                    &events,
                    &commands,
                    &mut pending,
                );
                request = None;
            }
            Err(error) => {
                let _ = send_event(
                    Event::Failed {
                        generation: active.generation,
                        message: error.to_string(),
                    },
                    &events,
                    &commands,
                    &mut pending,
                );
                request = None;
            }
        }
    }
}

fn frame_from_raw_frame(ts: f32, raw: Frame) -> Result<FrameBuffer, String> {
    let shape = raw.shape();
    let (height, width, channels) = (shape[0], shape[1], shape[2]);
    if !matches!(channels, 3 | 4) {
        return Err(format!("Unsupported channel count: {channels}"));
    }
    let pixels: Cow<'_, [u8]> = match raw.as_slice() {
        Some(slice) => Cow::Borrowed(slice),
        None => Cow::Owned(raw.iter().copied().collect()),
    };
    let rgba_data = if channels == 4 {
        pixels.into_owned()
    } else {
        let mut rgba = Vec::with_capacity(height * width * 4);
        for pixel in pixels.as_chunks::<3>().0 {
            rgba.extend_from_slice(&[pixel[0], pixel[1], pixel[2], 255]);
        }
        rgba
    };
    Ok(FrameBuffer {
        width: width as u32,
        height: height as u32,
        ts,
        rgba_data,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_evicts_old_frames_and_keeps_the_newest() {
        let mut frames = VecDeque::from([
            FrameBuffer {
                width: 1,
                height: 1,
                ts: 0.0,
                rgba_data: vec![0; 4],
            },
            FrameBuffer {
                width: 1,
                height: 1,
                ts: 1.0,
                rgba_data: vec![0; 4],
            },
            FrameBuffer {
                width: 1,
                height: 1,
                ts: 2.0,
                rgba_data: vec![0; 4],
            },
        ]);
        let mut bytes = 12;
        trim_cache_to(&mut frames, &mut bytes, 7);
        assert_eq!(bytes, 4);
        assert_eq!(frames.front().unwrap().ts, 2.0);
    }
}
