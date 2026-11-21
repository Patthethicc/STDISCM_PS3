use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tempfile::NamedTempFile;

pub struct Storage {
    uploads_dir: PathBuf,
    previews_dir: PathBuf,
    thumbnails_dir: PathBuf,
    reserved: Mutex<HashSet<String>>,
}

impl Storage {
    pub fn new(data_dir: impl Into<PathBuf>) -> std::io::Result<Self> {
        let data_dir = data_dir.into();
        let uploads_dir = data_dir.join("uploads");
        let previews_dir = data_dir.join("previews");
        let thumbnails_dir = data_dir.join("thumbnails");
        for d in [&uploads_dir, &previews_dir, &thumbnails_dir] {
            std::fs::create_dir_all(d)?;
        }
        Ok(Self {
            uploads_dir,
            previews_dir,
            thumbnails_dir,
            reserved: Mutex::new(HashSet::new()),
        })
    }

    /// Pre-load the in-flight reservation set from the persisted catalog.
    /// Call this right after constructing storage and opening the catalog.
    pub fn seed_reserved<I: IntoIterator<Item = String>>(&self, hashes: I) {
        let mut set = self.reserved.lock().unwrap();
        for h in hashes {
            set.insert(h.to_ascii_lowercase());
        }
    }

    pub fn id_from_sha(sha256: &str) -> String {
        sha256.to_ascii_lowercase().chars().take(16).collect()
    }

    pub fn try_reserve(&self, sha256: &str) -> bool {
        self.reserved.lock().unwrap().insert(sha256.to_ascii_lowercase())
    }
    pub fn release(&self, sha256: &str) {
        self.reserved.lock().unwrap().remove(&sha256.to_ascii_lowercase());
    }

    pub fn create_staging(&self) -> std::io::Result<NamedTempFile> {
        NamedTempFile::new_in(&self.uploads_dir)
    }

    /// Final file: `uploads/<id>.<ext>` — matches `VideoEntry.video_path`
    /// (see model.rs) so the GUI can serve it under `/media/`.
    pub fn persist(
        &self,
        staging: NamedTempFile,
        sha256: &str,
        filename: &str,
    ) -> Result<PathBuf, String> {
        let id = Self::id_from_sha(sha256);
        let ext = safe_extension(filename);
        let final_path = self.uploads_dir.join(format!("{id}.{ext}"));
        staging.persist(&final_path)
            .map_err(|e| format!("persist failed: {e}"))?;
        Ok(final_path)
    }

    /// Where the transcoded video goes: `uploads/<id>.mp4` (written by the media crate).
    pub fn video_abs(&self, id: &str) -> PathBuf { self.uploads_dir.join(format!("{id}.mp4")) }
    pub fn transcoded_rel(id: &str) -> String { format!("uploads/{id}.mp4") }
    pub fn preview_abs(&self, id: &str) -> PathBuf { self.previews_dir.join(format!("{id}.mp4")) }
    pub fn thumbnail_abs(&self, id: &str) -> PathBuf { self.thumbnails_dir.join(format!("{id}.jpg")) }
    pub fn preview_rel(id: &str) -> String { format!("previews/{id}.mp4") }
    pub fn thumbnail_rel(id: &str) -> String { format!("thumbnails/{id}.jpg") }
    pub fn video_rel(id: &str, original_filename: &str) -> String {
        format!("uploads/{id}.{}", safe_extension(original_filename))
    }
}

fn safe_extension(filename: &str) -> String {
    Path::new(filename)
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase())
        .filter(|s| matches!(s.as_str(),
            "mp4" | "mov" | "webm" | "mkv" | "avi" | "m4v" | "flv" | "wmv"))
        .unwrap_or_else(|| "mp4".into())
}