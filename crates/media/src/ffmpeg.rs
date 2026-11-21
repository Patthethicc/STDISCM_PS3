//! Starts ffmpeg + ffprobe processes, waits for them to finish, and returns their output/error. 
//! Everything else builds an argument list and calls `run`.

use std::ffi::OsString;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use crate::error::MediaError;

/// "-nosdin" : never read keyboard (prevents Y/N waiting prompts)
/// "-hide_banner": don't print ffmpeg version + build info at the top of every run
/// "-loglevel" + error: suppress all output except real errors
/// "error": only print real errors on stderr
/// "-y": automatically overwrite output files
pub(crate) const QUIET: [&str; 5] = ["-nostdin", "-hide_banner", "-loglevel", "error", "-y"];

pub(crate) struct ToolOutput {
    pub stdout: Vec<u8>,
}

/// Run `program args...` with a hard time limit.
/// stdout/stder are read concurrently so they do not fill process pipes
/// and block the child process. Child is killed if timeout exceeded.
pub(crate) fn run(program: &Path, args: &[OsString], timeout: Duration) -> Result<ToolOutput, MediaError> {
    let tool = program.file_name().unwrap_or(program.as_os_str()).to_string_lossy().into_owned();

    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => MediaError::ToolMissing(tool.clone()),
            _ => MediaError::Io(e),
        })?;

    let mut out_pipe = child.stdout.take().expect("stdout is piped");
    let mut err_pipe = child.stderr.take().expect("stderr is piped");
    let out_reader = thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = out_pipe.read_to_end(&mut buf);
        buf
    });
    let err_reader = thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = err_pipe.read_to_end(&mut buf);
        buf
    });

    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if started.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait(); // reap it so no zombie is left behind
            return Err(MediaError::Timeout { tool, secs: timeout.as_secs() });
        }
        thread::sleep(Duration::from_millis(50));
    };

    let stdout = out_reader.join().unwrap_or_default();
    let stderr = String::from_utf8_lossy(&err_reader.join().unwrap_or_default()).into_owned();

    if !status.success() {
        return Err(MediaError::ToolFailed { tool, code: status.code(), stderr: tail(&stderr, 1500) });
    }
    Ok(ToolOutput { stdout })
}

/// Fail fast at startup if ffmpeg or ffprobe is missing.
pub fn check_tools(ffmpeg: &Path, ffprobe: &Path) -> Result<(), MediaError> {
    for tool in [ffmpeg, ffprobe] {
        run(tool, &["-version".into()], Duration::from_secs(10))?;
    }
    Ok(())
}

/// Write to `<final>.part`, then rename into place, so readers (the GUI,
/// the catalog) never see a half-written file.
pub(crate) fn part_path(final_path: &Path) -> PathBuf {
    let mut name = final_path.as_os_str().to_owned();
    name.push(".part");
    PathBuf::from(name)
}

pub(crate) fn finish(part: &Path, final_path: &Path) -> Result<(), MediaError> {
    let len = std::fs::metadata(part).map(|m| m.len()).unwrap_or(0);
    if len == 0 {
        let _ = std::fs::remove_file(part);
        return Err(MediaError::EmptyOutput(final_path.to_path_buf()));
    }
    std::fs::rename(part, final_path)?;
    Ok(())
}

/// Run one ffmpeg encode into `final_path` via a `.part` file; the `.part`
/// file is removed if anything goes wrong.
pub(crate) fn encode(ffmpeg: &Path, mut args: Vec<OsString>, final_path: &Path, timeout: Duration) -> Result<(), MediaError> {
    if let Some(dir) = final_path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let part = part_path(final_path);
    args.push(part.clone().into());
    let result = run(ffmpeg, &args, timeout).and_then(|_| finish(&part, final_path));
    if result.is_err() {
        let _ = std::fs::remove_file(&part);
    }
    result
}

fn tail(s: &str, max: usize) -> String {
    let s = s.trim();
    if s.len() <= max {
        return s.to_string();
    }
    let mut start = s.len() - max;
    while !s.is_char_boundary(start) {
        start += 1;
    }
    format!("…{}", &s[start..])
}

/// Small helper so argument lists read like the command line.
pub(crate) fn args<I, S>(items: I) -> Vec<OsString>
where
    I: IntoIterator<Item = S>,
    S: Into<OsString>,
{
    items.into_iter().map(Into::into).collect()
}
