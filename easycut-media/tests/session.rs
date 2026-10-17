use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};

use easycut_media::{MediaSession, MediaState};

fn sample_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("sample_file.mp4")
}

fn wait_until(session: &mut MediaSession, condition: impl Fn(&MediaSession) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        session.poll();
        if condition(session) {
            return;
        }
        thread::sleep(Duration::from_millis(10));
    }
    panic!(
        "timed out waiting for decoder: state={:?}, error={:?}",
        session.state, session.error
    );
}

#[test]
fn opens_and_seeks_without_waiting_for_the_whole_video() {
    let mut session = MediaSession::open(sample_path()).unwrap();
    wait_until(&mut session, |session| session.state == MediaState::Ready);
    let meta = session.meta.as_ref().unwrap();
    assert_eq!((meta.width, meta.height), (640, 360));
    assert!(session.frame_at(0.0).is_some());

    session.seek(5_000);
    session.seek(18_000);
    wait_until(&mut session, |session| session.state == MediaState::Ready);
    let frame = session.frame_at(18.0).unwrap();
    assert!(
        (frame.ts - 18.0).abs() < 0.1,
        "unexpected PTS: {}",
        frame.ts
    );

    session.seek(30_000);
    wait_until(&mut session, |session| {
        matches!(session.state, MediaState::Ready | MediaState::Ended)
    });
    assert!(session.frame_at(30.0).is_some());
}

#[test]
fn reports_open_errors() {
    let mut session = MediaSession::open(sample_path().with_file_name("missing.mp4")).unwrap();
    wait_until(&mut session, |session| session.state == MediaState::Failed);
    assert!(
        session
            .error
            .as_ref()
            .is_some_and(|message| !message.is_empty())
    );
}

#[test]
fn prefetch_resumes_after_each_window_and_reaches_eof() {
    let mut session = MediaSession::open(sample_path()).unwrap();
    wait_until(&mut session, |session| session.state == MediaState::Ready);
    for second in 1..=3 {
        session.prefetch(second * 1_000);
        wait_until(&mut session, |session| session.contains_time(second as f32));
    }
    session.seek(30_000);
    wait_until(&mut session, |session| {
        matches!(session.state, MediaState::Ready | MediaState::Ended)
    });
    if session.state == MediaState::Ready {
        session.prefetch(31_000);
    }
    wait_until(&mut session, |session| session.state == MediaState::Ended);
    assert!(session.frame_at(30.0).is_some());
    assert!(!session.contains_time(0.0));
    let duration = session.meta.as_ref().unwrap().duration;
    let last_frame = session.latest_frame_time().unwrap();
    assert!(
        (0.0..=duration + 0.1).contains(&last_frame),
        "invalid final frame PTS: {last_frame}"
    );
    assert!(session.frame_at(duration).unwrap().ts >= 0.0);
}

#[test]
fn latest_seek_interrupts_a_full_result_queue() {
    let mut session = MediaSession::open(sample_path()).unwrap();
    thread::sleep(Duration::from_millis(200));
    session.seek(18_000);
    wait_until(&mut session, |session| session.state == MediaState::Ready);
    let frame = session.frame_at(18.0).unwrap();
    assert!((frame.ts - 18.0).abs() < 0.1);
}
