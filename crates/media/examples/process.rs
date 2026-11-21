//! cargo run -p media --example process -- <data_dir> <video>...
//! Runs the full pipeline on each file and prints what a worker would log.

use std::path::{Path, PathBuf};
use std::time::Instant;

use media::{check_tools, process_video, MediaConfig, OutputPaths};

fn main() {
    let mut argv = std::env::args().skip(1);
    let data_dir = PathBuf::from(argv.next().expect("usage: process <data_dir> <video>..."));
    let cfg = MediaConfig::default();

    if let Err(e) = check_tools(&cfg.ffmpeg, &cfg.ffprobe) {
        eprintln!("startup check failed: {e}");
        std::process::exit(1);
    }

    for file in argv {
        let input = Path::new(&file);
        let id = input.file_stem().unwrap_or_default().to_string_lossy().into_owned();
        let out = OutputPaths::for_id(&data_dir, &id);
        let started = Instant::now();
        match process_video(&cfg, input, &out) {
            Ok(m) => {
                let dur = m.duration_secs.map_or("unknown".into(), |d| format!("{d:.1}s"));
                let size = match &m.compression {
                    Some(c) => format!("{} -> {} bytes ({:.1}% saved)", c.original_bytes, c.compressed_bytes, c.saved_percent),
                    None => "not compressed".into(),
                };
                println!(
                    "[media] OK     {file}: {}x{} {} {dur}, preview {:.1}s, thumb={}, {size}, play={} ({:.1}s)",
                    m.info.width, m.info.height, m.info.video_codec, m.preview_secs,
                    m.thumbnail.is_some(), m.playback.display(), started.elapsed().as_secs_f64()
                );
                for w in &m.warnings {
                    println!("[media]   warn {w}");
                }
            }
            Err(e) => {
                let kind = if e.is_bad_input() { "REJECT" } else { "ERROR " };
                println!("[media] {kind} {file}: {e}");
            }
        }
    }
}
