use std::fs;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

/// Export one range as an H.264/AAC MP4. The caller should run this on a worker thread.
/// Requires an `ffmpeg` executable with libx264 and AAC encoders on PATH.
pub fn export_clip(input: &Path, output: &Path, start: f32, end: f32) -> Result<(), String> {
    export_clip_with_progress(input, output, start, end, |_| {})
}

/// Export one range and report the encoded fraction through `on_progress`.
///
/// FFmpeg reports `out_time_us` while encoding. The callback is invoked with values in
/// `[0.0, 1.0]`; it is called at least once with `0.0` and, after a successful export, with
/// `1.0`.
pub fn export_clip_with_progress<F>(
    input: &Path,
    output: &Path,
    start: f32,
    end: f32,
    mut on_progress: F,
) -> Result<(), String>
where
    F: FnMut(f32),
{
    if !start.is_finite() || !end.is_finite() || start < 0.0 || end <= start {
        return Err("Select a valid range with the end after the start".into());
    }
    if input == output {
        return Err("Output must differ from the source video".into());
    }
    if output.exists() {
        return Err(format!("Output already exists: {}", output.display()));
    }
    let parent = output
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_nanos();
    let temporary = parent.join(format!(".easycut-{}-{stamp}.mp4", std::process::id()));
    let duration = end - start;
    on_progress(0.0);
    let result = Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-nostdin",
            "-n",
            "-progress",
            "pipe:1",
            "-nostats",
        ])
        .arg("-i")
        .arg(input)
        .arg("-ss")
        .arg(format!("{start:.6}"))
        .arg("-t")
        .arg(format!("{duration:.6}"))
        .args([
            "-map",
            "0:v:0",
            "-map",
            "0:a:0?",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
            "-movflags",
            "+faststart",
        ])
        .arg(&temporary)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();

    let result = match result {
        Ok(mut child) => {
            let Some(stdout) = child.stdout.take() else {
                let _ = child.kill();
                let _ = child.wait();
                let _ = fs::remove_file(&temporary);
                return Err("FFmpeg did not provide a progress stream".into());
            };
            let Some(mut stderr) = child.stderr.take() else {
                let _ = child.kill();
                let _ = child.wait();
                let _ = fs::remove_file(&temporary);
                return Err("FFmpeg did not provide an error stream".into());
            };
            let stderr_reader = std::thread::spawn(move || {
                let mut output = Vec::new();
                let result = stderr.read_to_end(&mut output);
                (result, output)
            });
            let progress_result = read_progress(BufReader::new(stdout), duration, &mut on_progress);
            if let Err(error) = progress_result {
                let _ = child.kill();
                let _ = child.wait();
                let _ = stderr_reader.join();
                let _ = fs::remove_file(&temporary);
                return Err(error);
            }
            let status = child.wait();
            let (stderr_result, stderr) = match stderr_reader.join() {
                Ok(result) => result,
                Err(_) => {
                    let _ = fs::remove_file(&temporary);
                    return Err("FFmpeg stderr reader stopped unexpectedly".into());
                }
            };
            if let Err(error) = stderr_result {
                let _ = fs::remove_file(&temporary);
                return Err(format!("Could not read FFmpeg error output: {error}"));
            }
            status.map(|status| std::process::Output {
                status,
                stdout: Vec::new(),
                stderr,
            })
        }
        Err(error) => Err(error),
    };

    let export_result = match result {
        Ok(result) if result.status.success() => fs::hard_link(&temporary, output)
            .map_err(|error| format!("Could not save output: {error}"))
            .map(|()| on_progress(1.0)),
        Ok(result) => Err(format!(
            "FFmpeg export failed: {}",
            String::from_utf8_lossy(&result.stderr).trim()
        )),
        Err(error) => Err(format!("Could not start or wait for ffmpeg: {error}")),
    };
    let _ = fs::remove_file(&temporary);
    export_result
}

fn read_progress<R, F>(reader: R, duration: f32, on_progress: &mut F) -> Result<(), String>
where
    R: BufRead,
    F: FnMut(f32),
{
    for line in reader.lines() {
        let line = line.map_err(|error| format!("Could not read FFmpeg progress: {error}"))?;
        let Some(value) = line.strip_prefix("out_time_us=") else {
            continue;
        };
        let Ok(microseconds) = value.parse::<f64>() else {
            continue;
        };
        let fraction = (microseconds / f64::from(duration) / 1_000_000.0).clamp(0.0, 1.0) as f32;
        on_progress(fraction);
    }
    Ok(())
}

/// The selected source and destination for a completed export.
#[derive(Debug)]
pub struct ExportResult {
    pub output: PathBuf,
    pub result: Result<(), String>,
}

#[cfg(test)]
mod tests {
    use super::{export_clip, export_clip_with_progress, read_progress};
    use std::io::Cursor;
    use std::path::Path;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn rejects_invalid_ranges_before_starting_ffmpeg() {
        let source = Path::new("source.mp4");
        let output = Path::new("output.mp4");
        assert!(export_clip(source, output, 2.0, 2.0).is_err());
        assert!(export_clip(source, output, -1.0, 2.0).is_err());
        assert!(export_clip(source, output, f32::NAN, 2.0).is_err());
        assert!(export_clip(source, source, 0.0, 1.0).is_err());
    }

    #[test]
    fn parses_ffmpeg_progress_as_a_bounded_fraction() {
        let mut values = Vec::new();
        read_progress(
            Cursor::new("out_time_us=250000\nprogress=continue\nout_time_us=1200000\n"),
            1.0,
            &mut |value| values.push(value),
        )
        .unwrap();
        assert_eq!(values, vec![0.25, 1.0]);
    }

    #[test]
    fn exports_a_short_clip_and_reports_completion() {
        if std::process::Command::new("ffmpeg")
            .arg("-version")
            .output()
            .is_err()
        {
            return;
        }
        let input = Path::new(env!("CARGO_MANIFEST_DIR")).join("../sample_file.mp4");
        if !input.exists() {
            return;
        }
        let output = std::env::temp_dir().join(format!(
            "easycut-export-test-{}-{}.mp4",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut progress = Vec::new();
        export_clip_with_progress(&input, &output, 1.0, 1.5, |value| progress.push(value)).unwrap();
        assert!(output.exists());
        assert_eq!(progress.first(), Some(&0.0));
        assert_eq!(progress.last(), Some(&1.0));
        assert!(progress.windows(2).all(|pair| pair[0] <= pair[1]));
        std::fs::remove_file(output).unwrap();
    }
}
