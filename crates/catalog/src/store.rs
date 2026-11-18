use crate::VideoEntry;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

pub const CATALOG_FILE: &str = "catalog.json";

/// Writer handle, shared by the consumer's worker threads (wrap it in an `Arc`).
pub struct Catalog {
    path: PathBuf,
    entries: Mutex<Vec<VideoEntry>>,
}

impl Catalog {
    /// Opens `<data_dir>/catalog.json`, or starts empty if it doesn't exist yet.
    pub fn open(data_dir: impl AsRef<Path>) -> io::Result<Self> {
        let path = data_dir.as_ref().join(CATALOG_FILE);
        let entries = read_catalog(data_dir)?;
        Ok(Self { path, entries: Mutex::new(entries) })
    }

    /// Adds a processed video and saves the catalog to disk.
    /// The mutex is held for the whole write, so concurrent workers can't
    /// interleave writes or lose each other's entries.
    pub fn add(&self, entry: VideoEntry) -> io::Result<()> {
        let mut entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        entries.retain(|e| e.id != entry.id);
        entries.push(entry);
        write_atomic(&self.path, &entries)
    }

    /// Snapshot of all entries, newest first.
    pub fn list(&self) -> Vec<VideoEntry> {
        let entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        newest_first(entries.clone())
    }
}

/// Reads the catalog from disk, newest first. Used by the GUI, which runs as a
/// separate process and re-reads the file to pick up new uploads.
pub fn read_catalog(data_dir: impl AsRef<Path>) -> io::Result<Vec<VideoEntry>> {
    let path = data_dir.as_ref().join(CATALOG_FILE);
    match fs::read(&path) {
        Ok(bytes) if bytes.is_empty() => Ok(Vec::new()),
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map(newest_first)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(e),
    }
}

fn newest_first(mut v: Vec<VideoEntry>) -> Vec<VideoEntry> {
    v.sort_by(|a, b| b.added_at_ms.cmp(&a.added_at_ms));
    v
}

fn write_atomic(path: &Path, entries: &[VideoEntry]) -> io::Result<()> {
    let tmp = path.with_extension("json.tmp");
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(&serde_json::to_vec_pretty(entries)?)?;
        f.sync_all()?;
    }
    fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    fn entry(id: &str, t: u64) -> VideoEntry {
        VideoEntry {
            id: id.into(),
            filename: format!("{id}.mp4"),
            sha256: id.into(),
            duration_secs: 12.0,
            original_size: 100,
            compressed_size: Some(50),
            video_path: format!("uploads/{id}.mp4"),
            preview_path: Some(format!("previews/{id}.mp4")),
            thumbnail_path: Some(format!("thumbnails/{id}.jpg")),
            added_at_ms: t,
        }
    }

    #[test]
    fn concurrent_adds_are_all_saved() {
        let dir = std::env::temp_dir().join(format!("catalog-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let cat = Arc::new(Catalog::open(&dir).unwrap());

        let handles: Vec<_> = (0..8)
            .map(|i| {
                let cat = Arc::clone(&cat);
                thread::spawn(move || cat.add(entry(&format!("v{i}"), i)).unwrap())
            })
            .collect();
        for h in handles {
            h.join().unwrap();
        }

        let on_disk = read_catalog(&dir).unwrap();
        assert_eq!(on_disk.len(), 8);
        assert_eq!(on_disk[0].id, "v7"); // newest first
        fs::remove_dir_all(&dir).unwrap();
    }
}
