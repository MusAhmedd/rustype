//! Plain .txt note storage, one file per note.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct Note {
    pub path: PathBuf,
    pub title: String,
    pub body: String,
    pub modified: SystemTime,
    pub created: SystemTime,
}

pub struct Store {
    dir: PathBuf,
}

/// First non-empty line, trimmed and shortened.
pub fn title_of(text: &str) -> String {
    let line = text.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("");
    line.chars().take(40).collect()
}

/// Human-friendly "time since" label.
pub fn ago(t: SystemTime) -> String {
    let s = t.elapsed().map_or(0, |d| d.as_secs());
    match s {
        0..=59 => "just now".into(),
        60..=3599 => format!("{} min ago", s / 60),
        3600..=86_399 => format!("{} h ago", s / 3600),
        _ => format!("{} d ago", s / 86_400),
    }
}

impl Store {
    /// Notes live in `Documents/Rustype Notes`.
    pub fn open_default() -> Self {
        let base = dirs::document_dir().unwrap_or_else(|| PathBuf::from("."));
        let dir = base.join("Rustype Notes");
        let _ = fs::create_dir_all(&dir);
        Self { dir }
    }

    pub fn tags_path(&self) -> PathBuf {
        self.dir.join("tags.json")
    }

    pub fn settings_path(&self) -> PathBuf {
        self.dir.join("settings.json")
    }

    /// All saved notes, newest first.
    pub fn list(&self) -> Vec<Note> {
        let mut notes: Vec<Note> = fs::read_dir(&self.dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "txt"))
            .map(|path| {
                let meta = fs::metadata(&path).ok();
                let modified = meta.as_ref().and_then(|m| m.modified().ok()).unwrap_or(UNIX_EPOCH);
                let created = created_from_name(&path).or_else(|| meta.as_ref().and_then(|m| m.created().ok())).unwrap_or(modified);
                let body = load(&path);
                Note { title: title_of(&body), body, path, modified, created }
            })
            .collect();
        notes.sort_by(|a, b| b.modified.cmp(&a.modified));
        notes
    }

    /// Path for a new note. The file is only written once something is typed.
    pub fn new_path(&self) -> PathBuf {
        let ms = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis());
        self.dir.join(format!("sheet-{ms}.txt"))
    }
}

pub fn load(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_default().replace("\r\n", "\n")
}

pub fn save(path: &Path, text: &str) {
    if let Err(e) = fs::write(path, text) {
        eprintln!("failed to save {}: {e}", path.display());
    }
}

pub fn delete(path: &Path) {
    let _ = fs::remove_file(path);
}

/// New sheets are named `sheet-<unix ms>.txt`; that is their creation time.
fn created_from_name(path: &Path) -> Option<SystemTime> {
    let stem = path.file_stem()?.to_str()?;
    let ms: u64 = stem.strip_prefix("sheet-")?.parse().ok()?;
    Some(UNIX_EPOCH + std::time::Duration::from_millis(ms))
}
