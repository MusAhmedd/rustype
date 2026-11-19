//! User settings, saved as settings.json next to the notes.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum SoundPack {
    Classic,
    Electric,
    Vintage,
    Mechanical,
}

impl SoundPack {
    pub const ALL: [SoundPack; 4] = [Self::Classic, Self::Electric, Self::Vintage, Self::Mechanical];

    pub fn name(self) -> &'static str {
        match self {
            Self::Classic => "Classic manual",
            Self::Electric => "Electric (IBM-style)",
            Self::Vintage => "Vintage portable",
            Self::Mechanical => "Mechanical keyboard",
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum TypeFont {
    Standard,
    SpecialElite,
    CourierPrime,
    CutiveMono,
    SpaceMono,
    PlexMono,
}

impl TypeFont {
    pub const ALL: [TypeFont; 6] =
        [Self::Standard, Self::SpecialElite, Self::CourierPrime, Self::CutiveMono, Self::SpaceMono, Self::PlexMono];

    pub fn name(self) -> &'static str {
        match self {
            Self::Standard => "Standard mono",
            Self::SpecialElite => "Special Elite",
            Self::CourierPrime => "Courier Prime",
            Self::CutiveMono => "Cutive Mono",
            Self::SpaceMono => "Space Mono",
            Self::PlexMono => "IBM Plex Mono",
        }
    }

    pub fn family(self) -> eframe::egui::FontFamily {
        use eframe::egui::FontFamily;
        match self {
            Self::Standard => FontFamily::Monospace,
            Self::SpecialElite => FontFamily::Name("SpecialElite".into()),
            Self::CourierPrime => FontFamily::Name("CourierPrime".into()),
            Self::CutiveMono => FontFamily::Name("CutiveMono".into()),
            Self::SpaceMono => FontFamily::Name("SpaceMono".into()),
            Self::PlexMono => FontFamily::Name("PlexMono".into()),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum ExportFormat {
    Txt,
    Md,
    Png,
    Jpeg,
    Pdf,
}

impl ExportFormat {
    pub const ALL: [ExportFormat; 5] = [Self::Txt, Self::Md, Self::Png, Self::Jpeg, Self::Pdf];

    pub fn label(self) -> &'static str {
        match self {
            Self::Txt => "TXT",
            Self::Md => "MD",
            Self::Png => "PNG",
            Self::Jpeg => "JPEG",
            Self::Pdf => "PDF",
        }
    }

    pub fn ext(self) -> &'static str {
        match self {
            Self::Txt => "txt",
            Self::Md => "md",
            Self::Png => "png",
            Self::Jpeg => "jpg",
            Self::Pdf => "pdf",
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum HomeView {
    Grid,
    List,
    Stack,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum SortBy {
    Edited,
    Created,
    Name,
    TagSize,
}

impl SortBy {
    pub const ALL: [SortBy; 4] = [Self::Edited, Self::Created, Self::Name, Self::TagSize];

    pub fn name(self) -> &'static str {
        match self {
            Self::Edited => "Date edited",
            Self::Created => "Date created",
            Self::Name => "Name",
            Self::TagSize => "Tag size",
        }
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[serde(default)]
pub struct Settings {
    pub font: TypeFont,
    pub paper: [u8; 3],
    pub sound_on: bool,
    pub volume: f32,
    pub pack: SoundPack,
    pub light_on: bool,
    pub light_strength: f32,
    pub open_tabs: Vec<PathBuf>,
    pub active_tab: Option<usize>,
    pub export_format: ExportFormat,
    pub export_dir: Option<PathBuf>,
    pub home_view: HomeView,
    pub sort_by: SortBy,
    /// Newest / Z-A / biggest first.
    pub sort_desc: bool,
    pub auto_update_check: bool,
    /// Unix seconds of the last automatic update check.
    pub last_update_check: u64,
    /// Ring the bell when today's words reach `goal_words`.
    pub goal_on: bool,
    pub goal_words: u32,
    /// Net words typed on `goal_day` (local days since 1970).
    pub words_today: i64,
    pub goal_day: u32,
    /// Last day the goal bell rang, so it rings once a day.
    pub goal_hit_day: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            font: TypeFont::SpecialElite,
            paper: [244, 238, 222],
            sound_on: true,
            volume: 0.8,
            pack: SoundPack::Classic,
            light_on: false,
            light_strength: 0.7,
            open_tabs: Vec::new(),
            active_tab: None,
            export_format: ExportFormat::Pdf,
            export_dir: None,
            home_view: HomeView::Grid,
            sort_by: SortBy::Edited,
            sort_desc: true,
            auto_update_check: true,
            last_update_check: 0,
            goal_on: false,
            goal_words: 500,
            words_today: 0,
            goal_day: 0,
            goal_hit_day: 0,
        }
    }
}

impl Settings {
    pub fn load(path: &Path) -> Self {
        fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(s.trim_start_matches('\u{feff}')).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) {
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = fs::write(path, json);
        }
    }
}
