//! Colour tags for sheets. One tag per sheet, saved in tags.json next to
//! the notes so the .txt files stay plain.

use crate::theme;
use eframe::egui::{self, pos2, vec2, Align2, Color32, FontId, RichText, Sense, Stroke, Vec2};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

pub const PALETTE: [[u8; 3]; 10] = [
    [214, 84, 76],   // red
    [230, 140, 60],  // orange
    [226, 192, 70],  // yellow
    [110, 176, 96],  // green
    [70, 168, 160],  // teal
    [80, 140, 220],  // blue
    [130, 110, 214], // violet
    [206, 104, 168], // pink
    [150, 120, 90],  // brown
    [140, 144, 152], // grey
];

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct Tag {
    pub id: u32,
    pub name: String,
    pub color: [u8; 3],
}

impl Tag {
    pub fn color32(&self) -> Color32 {
        Color32::from_rgb(self.color[0], self.color[1], self.color[2])
    }
}

#[derive(Serialize, Deserialize, Default, Debug)]
#[serde(default)]
pub struct Tags {
    pub tags: Vec<Tag>,
    /// Sheet file name -> tag id.
    pub sheets: BTreeMap<String, u32>,
    next_id: u32,
    #[serde(skip)]
    path: PathBuf,
}

fn key(sheet: &Path) -> String {
    sheet.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

impl Tags {
    pub fn load(path: PathBuf) -> Self {
        let mut t: Tags = fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(s.trim_start_matches('\u{feff}')).ok())
            .unwrap_or_default();
        t.path = path;
        t
    }

    pub fn save(&self) {
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = fs::write(&self.path, json);
        }
    }

    pub fn get(&self, id: u32) -> Option<&Tag> {
        self.tags.iter().find(|t| t.id == id)
    }

    pub fn of(&self, sheet: &Path) -> Option<&Tag> {
        self.sheets.get(&key(sheet)).and_then(|&id| self.get(id))
    }

    pub fn set(&mut self, sheet: &Path, tag: Option<u32>) {
        match tag {
            Some(id) => self.sheets.insert(key(sheet), id),
            None => self.sheets.remove(&key(sheet)),
        };
        self.save();
    }

    pub fn create(&mut self, name: &str, color: [u8; 3]) -> u32 {
        self.next_id = self.next_id.max(self.tags.iter().map(|t| t.id + 1).max().unwrap_or(0));
        let id = self.next_id;
        self.next_id += 1;
        self.tags.push(Tag { id, name: clean_name(name), color });
        self.save();
        id
    }

    pub fn update(&mut self, id: u32, name: &str, color: [u8; 3]) {
        if let Some(t) = self.tags.iter_mut().find(|t| t.id == id) {
            t.name = clean_name(name);
            t.color = color;
        }
        self.save();
    }

    /// Removes a tag; its sheets become untagged (their text is untouched).
    pub fn delete(&mut self, id: u32) {
        self.tags.retain(|t| t.id != id);
        self.sheets.retain(|_, v| *v != id);
        self.save();
    }

    /// Forgets sheets that no longer exist.
    pub fn prune(&mut self, existing: &[PathBuf]) {
        let names: Vec<String> = existing.iter().map(|p| key(p)).collect();
        let before = self.sheets.len();
        self.sheets.retain(|k, _| names.contains(k));
        if self.sheets.len() != before {
            self.save();
        }
    }

    /// A palette colour no tag uses yet, for new tags.
    pub fn next_color(&self) -> [u8; 3] {
        PALETTE.iter().copied().find(|c| !self.tags.iter().any(|t| t.color == *c)).unwrap_or(PALETTE[self.tags.len() % PALETTE.len()])
    }
}

fn clean_name(name: &str) -> String {
    let n: String = name.trim().chars().take(24).collect();
    if n.is_empty() {
        "Tag".into()
    } else {
        n
    }
}

/// Colour swatches plus a custom colour button.
pub fn color_row(ui: &mut egui::Ui, color: &mut [u8; 3]) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
        for c in PALETTE {
            let (r, resp) = ui.allocate_exact_size(Vec2::splat(22.0), Sense::click());
            let t = ui.ctx().animate_bool_with_time(resp.id, resp.hovered(), 0.1);
            let p = ui.painter();
            p.circle_filled(r.center(), 8.5 + t, Color32::from_rgb(c[0], c[1], c[2]));
            if *color == c {
                p.circle_stroke(r.center(), 11.0, Stroke::new(1.8f32, theme::text()));
            }
            if resp.clicked() {
                *color = c;
            }
        }
        ui.color_edit_button_srgb(color).on_hover_text("Custom colour");
    });
}

/// In-progress new-tag form, kept in egui temp memory per picker.
#[derive(Clone, Default)]
struct NewTag {
    open: bool,
    name: String,
    color: [u8; 3],
}

/// Tag chooser for one sheet. Returns true once a choice was made.
pub fn picker(ui: &mut egui::Ui, tags: &mut Tags, sheet: &Path) -> bool {
    let mut done = false;
    let current = tags.of(sheet).map(|t| t.id);
    ui.set_width(230.0);
    ui.label(RichText::new("TAG").size(12.5).strong().color(theme::muted()));
    ui.add_space(2.0);

    let mut choice: Option<Option<u32>> = None;
    let row = |ui: &mut egui::Ui, color: Option<Color32>, name: &str, selected: bool| -> bool {
        let (r, resp) = ui.allocate_exact_size(vec2(230.0, 30.0), Sense::click());
        let t = ui.ctx().animate_bool_with_time(resp.id, resp.hovered(), 0.1);
        let p = ui.painter();
        if selected || t > 0.0 {
            p.rect_filled(r, 7.0, theme::tab_active().gamma_multiply(if selected { 1.0 } else { t }));
        }
        let c = pos2(r.left() + 16.0, r.center().y);
        match color {
            Some(col) => {
                p.circle_filled(c, 6.0, col);
            }
            None => {
                p.circle_stroke(c, 5.5, Stroke::new(1.3f32, theme::muted()));
            }
        }
        p.text(pos2(r.left() + 32.0, r.center().y), Align2::LEFT_CENTER, name, FontId::proportional(14.0), theme::text());
        if selected {
            let c = pos2(r.right() - 18.0, r.center().y);
            let s = Stroke::new(1.8f32, theme::accent());
            p.line_segment([c + vec2(-5.0, 0.0), c + vec2(-1.5, 3.5)], s);
            p.line_segment([c + vec2(-1.5, 3.5), c + vec2(5.0, -4.0)], s);
        }
        resp.clicked()
    };
    if row(ui, None, "No tag", current.is_none()) {
        choice = Some(None);
    }
    for t in tags.tags.clone() {
        if row(ui, Some(t.color32()), &t.name, current == Some(t.id)) {
            choice = Some(Some(t.id));
        }
    }

    ui.add_space(4.0);
    let id = ui.id().with("new_tag");
    let mut form: NewTag = ui.data(|d| d.get_temp(id)).unwrap_or_default();
    if !form.open {
        if ui.add(egui::Button::new(RichText::new("+  New tag").color(theme::muted())).frame(false)).clicked() {
            form = NewTag { open: true, name: String::new(), color: tags.next_color() };
        }
    } else {
        ui.separator();
        let r = ui.add(egui::TextEdit::singleline(&mut form.name).hint_text("Tag name").desired_width(230.0).margin(vec2(8.0, 5.0)));
        if form.name.is_empty() && !r.has_focus() && !r.lost_focus() {
            r.request_focus();
        }
        color_row(ui, &mut form.color);
        let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        ui.horizontal(|ui| {
            let add = egui::Button::new(RichText::new("Create & apply").color(Color32::WHITE)).fill(theme::accent()).rounding(7.0);
            if (ui.add_enabled(!form.name.trim().is_empty(), add).clicked() || enter) && !form.name.trim().is_empty() {
                let new = tags.create(&form.name, form.color);
                choice = Some(Some(new));
                form = NewTag::default();
            }
            if ui.button("Cancel").clicked() {
                form = NewTag::default();
            }
        });
    }
    ui.data_mut(|d| d.insert_temp(id, form));

    if let Some(c) = choice {
        tags.set(sheet, c);
        done = true;
    }
    done
}

/// Editing an existing tag (or creating one with `id: None`).
#[derive(Clone)]
pub struct Edit {
    pub id: Option<u32>,
    pub name: String,
    pub color: [u8; 3],
    pub confirm_delete: bool,
}

pub enum EditResult {
    Open,
    Close,
    /// Tag removed: clear any filter on it.
    Deleted(u32),
    Created(u32),
}

pub fn editor(ui: &mut egui::Ui, tags: &mut Tags, e: &mut Edit, sheet_count: usize) -> EditResult {
    ui.set_width(250.0);
    let title = if e.id.is_some() { "EDIT TAG" } else { "NEW TAG" };
    ui.label(RichText::new(title).size(12.5).strong().color(theme::muted()));
    ui.add_space(2.0);
    let r = ui.add(egui::TextEdit::singleline(&mut e.name).hint_text("Tag name").desired_width(250.0).margin(vec2(8.0, 6.0)));
    let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
    ui.add_space(4.0);
    color_row(ui, &mut e.color);
    ui.add_space(8.0);
    let mut out = EditResult::Open;
    ui.horizontal(|ui| {
        let save = egui::Button::new(RichText::new(if e.id.is_some() { "Save" } else { "Create" }).color(Color32::WHITE))
            .fill(theme::accent())
            .rounding(7.0)
            .min_size(vec2(72.0, 28.0));
        let valid = !e.name.trim().is_empty();
        if (ui.add_enabled(valid, save).clicked() || (enter && valid)) && valid {
            out = match e.id {
                Some(id) => {
                    tags.update(id, &e.name, e.color);
                    EditResult::Close
                }
                None => EditResult::Created(tags.create(&e.name, e.color)),
            };
        }
        if ui.add(egui::Button::new("Cancel").rounding(7.0).min_size(vec2(0.0, 28.0))).clicked() {
            out = EditResult::Close;
        }
        if let Some(id) = e.id {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let red = Color32::from_rgb(214, 80, 72);
                if e.confirm_delete {
                    let b = egui::Button::new(RichText::new("Confirm").color(Color32::WHITE)).fill(red).rounding(7.0).min_size(vec2(0.0, 28.0));
                    if ui.add(b).clicked() {
                        tags.delete(id);
                        out = EditResult::Deleted(id);
                    }
                } else if ui.add(egui::Button::new(RichText::new("Delete").color(red)).frame(false)).clicked() {
                    e.confirm_delete = true;
                }
            });
        }
    });
    if e.confirm_delete {
        let msg = match sheet_count {
            0 => "No sheets use this tag.".to_string(),
            1 => "1 sheet will become untagged. Its text is kept.".to_string(),
            n => format!("{n} sheets will become untagged. Their text is kept."),
        };
        ui.label(RichText::new(msg).size(12.0).color(theme::muted()));
    }
    out
}
