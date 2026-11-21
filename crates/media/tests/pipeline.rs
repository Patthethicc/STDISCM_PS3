//! cargo test -p media   (needs ffmpeg + ffprobe on PATH)
//! Each test makes its own fixture videos with ffmpeg's built-in test sources.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

use media::{process_video, MediaConfig, MediaError, OutputPaths};

fn workdir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("media-test-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// `ffmpeg -f lavfi` test pattern of `secs` seconds, with or without a tone.
fn make_video(path: &Path, secs: u32, size: &str, audio: bool) {
    let mut c = Command::new("ffmpeg");
    c.args(["-nostdin", "-hide_banner", "-loglevel", "error", "-y"]);
    c.args(["-f", "lavfi", "-i", &format!("testsrc2=size={size}:rate=25")]);
    if audio {
        c.args(["-f", "lavfi", "-i", "sine=frequency=440"]);
    }
    c.args(["-t", &secs.to_string(), "-c:v", "mpeg4", "-q:v", "3"]);
    if audio {
        c.args(["-c:a", "mp3"]);
    }
    assert!(c.arg(path).status().unwrap().success(), "fixture ffmpeg failed");
}

fn duration(path: &Path) -> f64 {
    let out = Command::new("ffprobe")
        .args(["-v", "error", "-show_entries", "format=duration", "-of", "default=nw=1:nk=1"])
        .arg(path)
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).trim().parse().unwrap()
}

#[test]
fn long_video_gets_10s_preview_thumbnail_and_compressed_copy() {
    let dir = workdir("long");
    let input = dir.join("long.avi");
    make_video(&input, 25, "1280x720", true);
    let out = OutputPaths::for_id(&dir, "long");

    let m = process_video(&MediaConfig::default(), &input, &out).unwrap();

    assert!((m.preview_secs - 10.0).abs() < 0.2, "preview was {}", m.preview_secs);
    assert!((duration(&out.preview) - 10.0).abs() < 0.2);
    assert!(m.thumbnail.is_some() && out.thumbnail.exists());
    let c = m.compression.expect("compression ran");
    assert!(c.original_bytes > 0 && c.compressed_bytes > 0);
    assert_eq!(m.playback, out.compressed);
    assert!(m.info.audio_codec.is_some());
}

#[test]
fn short_video_preview_is_the_whole_video() {
    let dir = workdir("short");
    let input = dir.join("short.avi");
    make_video(&input, 4, "640x360", false);
    let out = OutputPaths::for_id(&dir, "short");

    let m = process_video(&MediaConfig::default(), &input, &out).unwrap();

    assert!((m.preview_secs - 4.0).abs() < 0.2, "preview was {}", m.preview_secs);
    assert!(m.info.audio_codec.is_none()); // "-map 0:a:0?" must not fail without audio
}

#[test]
fn odd_frame_size_is_rounded_to_even() {
    let dir = workdir("odd");
    let input = dir.join("odd.mkv");
    let ok = Command::new("ffmpeg")
        .args(["-nostdin", "-loglevel", "error", "-y", "-f", "lavfi", "-i", "testsrc2=size=640x480:rate=25",
               "-t", "3", "-vf", "scale=321:241", "-pix_fmt", "yuv444p", "-c:v", "ffv1"])
        .arg(&input)
        .status()
        .unwrap()
        .success();
    assert!(ok);
    let out = OutputPaths::for_id(&dir, "odd");
    let m = process_video(&MediaConfig::default(), &input, &out).unwrap();
    assert_eq!((m.info.width, m.info.height), (321, 241));
    assert_eq!(m.playback, out.compressed);
}

#[test]
fn corrupt_and_non_video_files_are_rejected_and_leave_nothing_behind() {
    let dir = workdir("bad");
    let good = dir.join("good.avi");
    make_video(&good, 3, "320x240", true);
    let bytes = std::fs::read(&good).unwrap();

    let cases: Vec<(&str, Vec<u8>)> = vec![
        ("empty.mp4", vec![]),
        ("text.mp4", b"definitely not a video".to_vec()),
        ("random.mp4", (0..50_000u32).map(|i| (i.wrapping_mul(2654435761) >> 13) as u8).collect()),
        ("head_only.avi", bytes[..2_000].to_vec()),
    ];
    let cfg = MediaConfig::default();
    for (name, data) in cases {
        let input = dir.join(name);
        std::fs::write(&input, data).unwrap();
        let out = OutputPaths::for_id(&dir, name);
        let err = process_video(&cfg, &input, &out).expect_err(name);
        assert!(err.is_bad_input(), "{name}: {err}");
        assert!(!out.preview.exists() && !out.thumbnail.exists() && !out.compressed.exists(), "{name} left files");
    }
}

#[test]
fn missing_ffmpeg_is_reported_not_panicked() {
    let dir = workdir("missing");
    let input = dir.join("x.avi");
    make_video(&input, 1, "160x120", false);
    let cfg = MediaConfig { ffprobe: "/nope/ffprobe".into(), ..MediaConfig::default() };
    let err = process_video(&cfg, &input, &OutputPaths::for_id(&dir, "x")).unwrap_err();
    assert!(matches!(err, MediaError::ToolMissing(_)), "{err}");
}

#[test]
fn slow_encode_is_killed_by_the_timeout() {
    let dir = workdir("timeout");
    let input = dir.join("big.avi");
    make_video(&input, 20, "1920x1080", false);
    let cfg = MediaConfig {
        encode_timeout: Duration::from_millis(300),
        preset: "veryslow".into(),
        ..MediaConfig::default()
    };
    let out = OutputPaths::for_id(&dir, "big");
    let err = process_video(&cfg, &input, &out).unwrap_err();
    assert!(matches!(err, MediaError::Timeout { .. }), "{err}");
    assert!(!out.preview.exists(), "partial preview left behind");
}

#[test]
fn several_workers_can_share_one_config() {
    let dir = workdir("threads");
    let cfg = Arc::new(MediaConfig { threads: 1, ..MediaConfig::default() });
    let handles: Vec<_> = (0..4)
        .map(|i| {
            let input = dir.join(format!("v{i}.avi"));
            make_video(&input, 3, "320x240", true);
            let out = OutputPaths::for_id(&dir, &format!("v{i}"));
            let cfg = Arc::clone(&cfg);
            std::thread::spawn(move || process_video(&cfg, &input, &out))
        })
        .collect();
    for h in handles {
        h.join().expect("worker panicked").expect("processing failed");
    }
}

#[test]
fn transcode_always_lands_in_uploads_even_for_small_browser_ready_mp4() {
    let dir = workdir("small");
    let input = dir.join("small.mp4");
    let ok = Command::new("ffmpeg")
        .args(["-nostdin", "-hide_banner", "-loglevel", "error", "-y"])
        .args(["-f", "lavfi", "-i", "testsrc2=size=160x120:rate=10", "-f", "lavfi", "-i", "sine=frequency=440"])
        .args(["-t", "2", "-c:v", "libx264", "-crf", "35", "-preset", "veryfast", "-pix_fmt", "yuv420p", "-c:a", "aac", "-b:a", "32k"])
        .arg(&input)
        .status()
        .unwrap()
        .success();
    assert!(ok);
    let out = OutputPaths::for_id(&dir, "small");
    assert!(out.compressed.ends_with("uploads/small.mp4"));
    let m = process_video(&MediaConfig::default(), &input, &out).unwrap();
    assert_eq!(m.playback, out.compressed);
    assert!(out.compressed.exists());
}

#[test]
fn refuses_to_overwrite_the_input() {
    let dir = workdir("same");
    let out = OutputPaths::for_id(&dir, "x");
    std::fs::create_dir_all(out.compressed.parent().unwrap()).unwrap();
    make_video(&out.compressed, 2, "160x120", true); // the "original" already sits at uploads/x.mp4
    let before = std::fs::metadata(&out.compressed).unwrap().len();
    let err = process_video(&MediaConfig::default(), &out.compressed, &out).unwrap_err();
    assert!(matches!(err, MediaError::Io(_)));
    assert_eq!(std::fs::metadata(&out.compressed).unwrap().len(), before, "input was touched");
}
