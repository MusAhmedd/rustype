//! Floating control bar at the bottom centre: paper, typeface, sound,
//! light, emoticons and export. Fades back while you type.

use crate::icons;
use crate::export;
use crate::settings::{ExportFormat, Settings, SoundPack, TypeFont};
use crate::tags::{self, Tags};
use crate::theme;
use crate::update::{self, Status};
use std::path::Path;
use eframe::egui::{
    self, epaint::Shadow, pos2, vec2, Align2, Color32, FontId, Frame, Id, Margin, Order, Rect, RichText, Sense, Stroke, Vec2,
};

const PAPER_PRESETS: [(&str, [u8; 3]); 6] = [
    ("Cream", [244, 238, 222]),
    ("White", [250, 250, 247]),
    ("Aged", [226, 212, 180]),
    ("Mint", [220, 234, 222]),
    ("Slate", [205, 212, 222]),
    ("Night", [34, 34, 38]),
];

const EMOTICONS: [&str; 24] = [
    ":)", ":D", ";)", ":(", ":P", ":O", ":/", ":|", "<3", "B)", "XD", "^_^", "-_-", "o_O", "T_T", ">_<", "(:",
    "\\o/", ":-)", ";-)", "=)", ":'(", "<(^_^)>", "(>'.')>",
];

const BAR_GAP: f32 = 18.0;
const BTN: f32 = 38.0;

#[derive(Clone, Copy, PartialEq)]
pub enum Popup {
    Paper,
    Font,
    Sound,
    Light,
    Emoji,
    Export,
    Tag,
    About,
    Goal,
}

/// Word counts for the open sheet, shown by the goal button.
pub struct Stats {
    pub sheet_words: usize,
    pub sheet_chars: usize,
    pub today: i64,
    pub session_words: i64,
    pub session_secs: f64,
}

#[derive(Default)]
pub struct State {
    pub popup: Option<Popup>,
    pub last_typed: f64,
    bar_rect: Option<Rect>,
    popup_rect: Option<Rect>,
    pub export_name: String,
    /// Result of the last export: (message, is_error).
    pub export_status: Option<(String, bool)>,
}

pub enum Action {
    Insert(&'static str),
    TestSound,
    Export(ExportFormat, String),
    CopyText,
    CopyImage,
    CheckUpdate,
    InstallUpdate,
    OpenUrl(String),
    ResetToday,
}

fn panel_frame() -> Frame {
    Frame::none()
        .fill(theme::bar())
        .stroke(Stroke::new(1.0f32, theme::border()))
        .shadow(Shadow { offset: vec2(0.0, 8.0), blur: 24.0, spread: 0.0, color: Color32::from_black_alpha(110) })
}

/// `editor_title` is the open sheet's title, `None` on the home screen.
#[allow(clippy::too_many_arguments)]
pub fn show(
    ctx: &egui::Context,
    s: &mut Settings,
    st: &mut State,
    editor_title: Option<&str>,
    sheet: Option<&Path>,
    tags: &mut Tags,
    upd: &Status,
    stats: Option<&Stats>,
    now: f64,
) -> Option<Action> {
    let in_editor = editor_title.is_some();
    let mut action = None;
    if !in_editor && matches!(st.popup, Some(Popup::Emoji | Popup::Export | Popup::Tag | Popup::Goal)) {
        st.popup = None;
    }

    // Close the popup when clicking anywhere else.
    if ctx.input(|i| i.pointer.any_pressed()) {
        if let Some(p) = ctx.input(|i| i.pointer.interact_pos()) {
            let inside = st.bar_rect.is_some_and(|r| r.contains(p)) || st.popup_rect.is_some_and(|r| r.contains(p));
            if !inside && !ctx.memory(|m| m.any_popup_open()) {
                st.popup = None;
            }
        }
    }

    let hovered = ctx.input(|i| i.pointer.hover_pos()).is_some_and(|p| st.bar_rect.is_some_and(|r| r.expand(20.0).contains(p)));
    let typing = now - st.last_typed < 1.6;
    let target = if hovered || st.popup.is_some() || !typing { 1.0 } else { 0.22 };
    let opacity = ctx.animate_value_with_time(Id::new("bar_opacity"), target, 0.35);
    if (opacity - target).abs() > 0.01 || typing {
        ctx.request_repaint();
    }

    let resp = egui::Area::new(Id::new("floating_bar"))
        .order(Order::Foreground)
        .anchor(Align2::CENTER_BOTTOM, [0.0, -BAR_GAP])
        .show(ctx, |ui| {
            ui.multiply_opacity(opacity);
            panel_frame().rounding(BTN / 2.0 + 6.0).inner_margin(Margin::same(6.0)).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    let size = Vec2::splat(BTN);
                    let toggle_popup = |p: Popup, st: &mut State| {
                        st.popup = if st.popup == Some(p) { None } else { Some(p) };
                    };

                    let paper = Color32::from_rgb(s.paper[0], s.paper[1], s.paper[2]);
                    let r = icons::icon_button(ui, size, st.popup == Some(Popup::Paper), |p, r, _| {
                        p.circle(r.center(), r.width() * 0.55, paper, Stroke::new(1.5f32, theme::border()));
                    });
                    if r.on_hover_text("Paper").clicked() {
                        toggle_popup(Popup::Paper, st);
                    }

                    let fam = s.font.family();
                    let r = icons::icon_button(ui, size, st.popup == Some(Popup::Font), |p, r, c| {
                        p.text(r.center(), Align2::CENTER_CENTER, "Aa", FontId::new(17.0, fam), c);
                    });
                    if r.on_hover_text("Typeface").clicked() {
                        toggle_popup(Popup::Font, st);
                    }

                    let muted = !s.sound_on;
                    let r = icons::icon_button(ui, size, st.popup == Some(Popup::Sound), |p, r, c| icons::speaker(p, r, c, muted));
                    if r.on_hover_text("Typing sound").clicked() {
                        toggle_popup(Popup::Sound, st);
                    }

                    let lit = s.light_on;
                    let r = icons::icon_button(ui, size, lit || st.popup == Some(Popup::Light), |p, r, c| icons::bulb(p, r, c, lit));
                    if r.on_hover_text("Focus light").clicked() {
                        toggle_popup(Popup::Light, st);
                    }

                    let has_update = matches!(upd, Status::Available(_) | Status::Downloading(..));
                    let r = icons::icon_button(ui, size, st.popup == Some(Popup::About), |p, r, c| {
                        icons::info(p, r, c);
                        if has_update {
                            p.circle_filled(pos2(r.right() + 1.0, r.top() + 1.0), 4.0, theme::accent());
                        }
                    });
                    let tip = if has_update { "About & updates \u{00b7} update available" } else { "About & updates" };
                    if r.on_hover_text(tip).clicked() {
                        toggle_popup(Popup::About, st);
                    }

                    if in_editor {
                        let (sep, _) = ui.allocate_exact_size(vec2(9.0, BTN), Sense::hover());
                        ui.painter().line_segment(
                            [pos2(sep.center().x, sep.top() + 9.0), pos2(sep.center().x, sep.bottom() - 9.0)],
                            Stroke::new(1.0f32, theme::border()),
                        );
                        let r = icons::icon_button(ui, size, st.popup == Some(Popup::Emoji), |p, r, c| {
                            p.text(r.center(), Align2::CENTER_CENTER, ":)", FontId::monospace(16.0), c);
                        });
                        if r.on_hover_text("Emoticons").clicked() {
                            toggle_popup(Popup::Emoji, st);
                        }
                        let current = sheet.and_then(|p| tags.of(p)).cloned();
                        let r = icons::icon_button(ui, size, st.popup == Some(Popup::Tag), |p, r, c| {
                            icons::tag(p, r, c);
                            if let Some(t) = &current {
                                p.circle_filled(pos2(r.right() - 1.0, r.top() + 2.0), 4.5, t.color32());
                            }
                        });
                        let tip = current.as_ref().map_or("Tag".to_string(), |t| format!("Tag: {}", t.name));
                        if r.on_hover_text(tip).clicked() {
                            toggle_popup(Popup::Tag, st);
                        }
                        if let Some(stats) = stats {
                            let r = goal_button(ui, s, stats, st.popup == Some(Popup::Goal));
                            let tip = if s.goal_on {
                                format!("{} / {} words today", stats.today, s.goal_words)
                            } else {
                                "Word count & goal".to_string()
                            };
                            if r.on_hover_text(tip).clicked() {
                                toggle_popup(Popup::Goal, st);
                            }
                        }
                        let r = icons::icon_button(ui, size, st.popup == Some(Popup::Export), icons::export);
                        if r.on_hover_text("Export").clicked() {
                            toggle_popup(Popup::Export, st);
                            if st.popup == Some(Popup::Export) {
                                st.export_name = export::sanitize(editor_title.unwrap_or_default());
                                st.export_status = None;
                            }
                        }
                    }
                });
            });
        });
    st.bar_rect = Some(resp.response.rect);

    st.popup_rect = None;
    let mut close = false;
    if let Some(popup) = st.popup {
        let lift = BAR_GAP + resp.response.rect.height() + 10.0;
        let pr = egui::Area::new(Id::new("bar_popup"))
            .order(Order::Foreground)
            .anchor(Align2::CENTER_BOTTOM, [0.0, -lift])
            .show(ctx, |ui| {
                panel_frame().rounding(14.0).inner_margin(Margin::same(16.0)).show(ui, |ui| match popup {
                    Popup::Paper => paper_popup(ui, s),
                    Popup::Font => font_popup(ui, s),
                    Popup::Sound => {
                        if sound_popup(ui, s) {
                            action = Some(Action::TestSound);
                        }
                    }
                    Popup::Light => light_popup(ui, s),
                    Popup::Emoji => {
                        if let Some(e) = emoji_popup(ui) {
                            action = Some(Action::Insert(e));
                        }
                    }
                    Popup::Export => {
                        if let Some(a) = export_popup(ui, s, st) {
                            action = Some(a);
                        }
                    }
                    Popup::About => {
                        if let Some(a) = about_popup(ui, s, upd) {
                            action = Some(a);
                        }
                    }
                    Popup::Goal => {
                        if let Some(stats) = stats {
                            if goal_popup(ui, s, stats) {
                                action = Some(Action::ResetToday);
                            }
                        }
                    }
                    Popup::Tag => {
                        if let Some(p) = sheet {
                            if tags::picker(ui, tags, p) {
                                close = true;
                            }
                        }
                    }
                });
            });
        st.popup_rect = Some(pr.response.rect);
        if close {
            st.popup = None;
        }
    }
    action
}

fn heading(ui: &mut egui::Ui, t: &str) {
    ui.label(RichText::new(t).size(12.5).strong().color(theme::muted()));
    ui.add_space(4.0);
}

fn paper_popup(ui: &mut egui::Ui, s: &mut Settings) {
    heading(ui, "PAPER");
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 10.0;
        for (name, c) in PAPER_PRESETS {
            let (r, resp) = ui.allocate_exact_size(Vec2::splat(34.0), Sense::click());
            let t = ui.ctx().animate_bool_with_time(resp.id, resp.hovered(), 0.1);
            let col = Color32::from_rgb(c[0], c[1], c[2]);
            let sel = s.paper == c;
            let p = ui.painter();
            p.circle_filled(r.center(), 14.0 + t, col);
            p.circle_stroke(r.center(), 14.0 + t, Stroke::new(1.0f32, Color32::from_black_alpha(80)));
            if sel {
                p.circle_stroke(r.center(), 17.5, Stroke::new(2.0f32, theme::accent()));
            }
            if resp.on_hover_text(name).clicked() {
                s.paper = c;
            }
        }
        ui.add_space(4.0);
        ui.color_edit_button_srgb(&mut s.paper).on_hover_text("Custom colour");
    });
}

fn font_popup(ui: &mut egui::Ui, s: &mut Settings) {
    heading(ui, "TYPEFACE");
    for f in TypeFont::ALL {
        let (r, resp) = ui.allocate_exact_size(vec2(300.0, 38.0), Sense::click());
        let t = ui.ctx().animate_bool_with_time(resp.id, resp.hovered(), 0.1);
        let sel = s.font == f;
        let p = ui.painter();
        if sel || t > 0.0 {
            p.rect_filled(r, 8.0, theme::tab_active().gamma_multiply(if sel { 1.0 } else { t }));
        }
        p.text(pos2(r.left() + 12.0, r.center().y), Align2::LEFT_CENTER, "The quick fox", FontId::new(18.0, f.family()), theme::text());
        p.text(pos2(r.right() - 12.0, r.center().y), Align2::RIGHT_CENTER, f.name(), FontId::proportional(11.5), if sel { theme::accent() } else { theme::muted() });
        if resp.clicked() {
            s.font = f;
        }
    }
}

/// Returns true when "Test" was clicked.
fn sound_popup(ui: &mut egui::Ui, s: &mut Settings) -> bool {
    heading(ui, "TYPING SOUND");
    icons::toggle(ui, &mut s.sound_on, "Sounds on");
    ui.add_space(6.0);
    let mut test = false;
    ui.add_enabled_ui(s.sound_on, |ui| {
        for p in SoundPack::ALL {
            let (r, resp) = ui.allocate_exact_size(vec2(260.0, 32.0), Sense::click());
            let t = ui.ctx().animate_bool_with_time(resp.id, resp.hovered(), 0.1);
            let sel = s.pack == p;
            if sel || t > 0.0 {
                ui.painter().rect_filled(r, 8.0, theme::tab_active().gamma_multiply(if sel { 1.0 } else { t }));
            }
            let dot = pos2(r.left() + 16.0, r.center().y);
            ui.painter().circle_stroke(dot, 6.0, Stroke::new(1.5f32, if sel { theme::accent() } else { theme::muted() }));
            if sel {
                ui.painter().circle_filled(dot, 3.0, theme::accent());
            }
            ui.painter().text(pos2(r.left() + 32.0, r.center().y), Align2::LEFT_CENTER, p.name(), FontId::proportional(14.0), theme::text());
            if resp.clicked() {
                s.pack = p;
                test = true;
            }
        }
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("Volume").color(theme::muted()));
            ui.add(egui::Slider::new(&mut s.volume, 0.0..=1.5).show_value(false).trailing_fill(true));
            if ui.button("Test").clicked() {
                test = true;
            }
        });
    });
    test
}

fn light_popup(ui: &mut egui::Ui, s: &mut Settings) {
    heading(ui, "FOCUS LIGHT");
    icons::toggle(ui, &mut s.light_on, "Light on (dims the room)");
    ui.add_space(6.0);
    ui.add_enabled_ui(s.light_on, |ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Strength").color(theme::muted()));
            ui.add(egui::Slider::new(&mut s.light_strength, 0.2..=1.0).show_value(false).trailing_fill(true));
        });
    });
}

/// Pill with a progress ring (when a goal is set) and the sheet's word count.
fn goal_button(ui: &mut egui::Ui, s: &Settings, stats: &Stats, active: bool) -> egui::Response {
    let label = short_count(stats.sheet_words);
    let w = (BTN + label.len() as f32 * 7.5).max(BTN + 14.0);
    icons::icon_button(ui, vec2(w, BTN), active, |p, r, c| {
        let ring = pos2(r.center().x - w / 2.0 + BTN / 2.0 - 2.0, r.center().y);
        let rad = 8.0;
        p.circle_stroke(ring, rad, Stroke::new(2.0f32, c.gamma_multiply(0.35)));
        if s.goal_on && s.goal_words > 0 {
            let frac = (stats.today as f32 / s.goal_words as f32).clamp(0.0, 1.0);
            if frac >= 1.0 {
                p.circle_stroke(ring, rad, Stroke::new(2.0f32, theme::accent()));
                p.circle_filled(ring, 3.0, theme::accent());
            } else if frac > 0.0 {
                let n = (frac * 40.0).ceil() as usize + 1;
                let pts: Vec<_> = (0..n)
                    .map(|k| {
                        let a = -std::f32::consts::FRAC_PI_2 + std::f32::consts::TAU * frac * k as f32 / (n - 1) as f32;
                        ring + vec2(a.cos(), a.sin()) * rad
                    })
                    .collect();
                p.add(egui::Shape::line(pts, Stroke::new(2.0f32, theme::accent())));
            }
        } else {
            p.text(ring, Align2::CENTER_CENTER, "#", FontId::proportional(11.0), c);
        }
        p.text(pos2(ring.x + rad + 7.0, ring.y), Align2::LEFT_CENTER, label, FontId::proportional(13.5), c);
    })
}

/// 1234 -> "1.2k" so the button stays narrow.
fn short_count(n: usize) -> String {
    if n < 1000 {
        n.to_string()
    } else if n < 100_000 {
        format!("{:.1}k", n as f32 / 1000.0)
    } else {
        format!("{}k", n / 1000)
    }
}

/// Returns true when "Reset today" was clicked.
fn goal_popup(ui: &mut egui::Ui, s: &mut Settings, stats: &Stats) -> bool {
    ui.set_width(280.0);
    heading(ui, "WRITING");
    let row = |ui: &mut egui::Ui, k: &str, v: String| {
        ui.horizontal(|ui| {
            ui.label(RichText::new(k).color(theme::muted()));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(RichText::new(v).color(theme::text()));
            });
        });
    };
    use crate::goal::{duration, n_words};
    row(ui, "This sheet", format!("{} \u{00b7} {} chars", n_words(stats.sheet_words as i64), stats.sheet_chars));
    row(ui, "This session", format!("{} \u{00b7} {}", n_words(stats.session_words.max(0)), duration(stats.session_secs)));
    row(ui, "Today", n_words(stats.today));

    ui.add_space(8.0);
    ui.separator();
    icons::toggle(ui, &mut s.goal_on, "Daily goal");
    let mut reset = false;
    ui.add_enabled_ui(s.goal_on, |ui| {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("Words per day").color(theme::muted()));
            ui.add(egui::DragValue::new(&mut s.goal_words).range(50..=20_000).speed(10.0));
        });
        ui.add_space(6.0);
        let frac = if s.goal_words > 0 { (stats.today as f32 / s.goal_words as f32).clamp(0.0, 1.0) } else { 0.0 };
        let text = if frac >= 1.0 {
            "Goal reached".to_string()
        } else {
            format!("{} to go", s.goal_words as i64 - stats.today)
        };
        ui.add(egui::ProgressBar::new(frac).desired_width(280.0).fill(theme::accent()));
        ui.label(RichText::new(text).size(12.5).color(if frac >= 1.0 { theme::accent() } else { theme::muted() }));
        ui.add_space(4.0);
        if ui.add(egui::Button::new("Reset today").rounding(8.0)).on_hover_text("Start today's count from zero").clicked() {
            reset = true;
        }
    });
    reset
}

fn emoji_popup(ui: &mut egui::Ui) -> Option<&'static str> {
    heading(ui, "EMOTICONS");
    let mut out = None;
    egui::Grid::new("emoticons").spacing(vec2(4.0, 4.0)).show(ui, |ui| {
        for (i, e) in EMOTICONS.iter().enumerate() {
            let (r, resp) = ui.allocate_exact_size(vec2(64.0, 34.0), Sense::click());
            let t = ui.ctx().animate_bool_with_time(resp.id, resp.hovered(), 0.08);
            ui.painter().rect_filled(r, 8.0, icons::mix(theme::card(), theme::tab_active(), t));
            ui.painter().text(r.center(), Align2::CENTER_CENTER, *e, FontId::monospace(14.0), theme::text());
            if resp.clicked() {
                out = Some(*e);
            }
            if i % 6 == 5 {
                ui.end_row();
            }
        }
    });
    out
}

fn export_popup(ui: &mut egui::Ui, s: &mut Settings, st: &mut State) -> Option<Action> {
    let mut out = None;
    ui.set_width(330.0);
    heading(ui, "EXPORT");

    ui.label(RichText::new("File name").size(12.0).color(theme::muted()));
    let ext = s.export_format.ext();
    let name_resp = ui
        .horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            let r = ui.add(
                egui::TextEdit::singleline(&mut st.export_name)
                    .desired_width(330.0 - 52.0)
                    .hint_text("Untitled")
                    .margin(vec2(8.0, 6.0)),
            );
            ui.label(RichText::new(format!(".{ext}")).color(theme::muted()));
            r
        })
        .inner;
    let enter = name_resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));

    ui.add_space(6.0);
    ui.label(RichText::new("Format").size(12.0).color(theme::muted()));
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        for f in ExportFormat::ALL {
            let sel = s.export_format == f;
            let (r, resp) = ui.allocate_exact_size(vec2(60.0, 30.0), Sense::click());
            let t = ui.ctx().animate_bool_with_time(resp.id, resp.hovered(), 0.1);
            let p = ui.painter();
            p.rect_filled(r, 8.0, if sel { theme::tab_active() } else { icons::mix(theme::card(), theme::tab_active(), t) });
            if sel {
                p.rect_stroke(r, 8.0, Stroke::new(1.5f32, theme::accent()));
            }
            p.text(r.center(), Align2::CENTER_CENTER, f.label(), FontId::proportional(13.0), if sel { theme::text() } else { theme::muted() });
            let tip = match f {
                ExportFormat::Txt => "Plain text",
                ExportFormat::Md => "Markdown, first line as heading",
                ExportFormat::Png => "Styled sheet image (one file per page)",
                ExportFormat::Jpeg => "Styled sheet image, smaller files (one per page)",
                ExportFormat::Pdf => "Styled A4 pages in one PDF",
            };
            if resp.on_hover_text(tip).clicked() {
                s.export_format = f;
            }
        }
    });

    ui.add_space(10.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        let save = egui::Button::new(RichText::new("Export\u{2026}").color(Color32::WHITE).strong())
            .fill(theme::accent())
            .rounding(8.0)
            .min_size(vec2(110.0, 32.0));
        if ui.add(save).on_hover_text("Choose where to save").clicked() || enter {
            let name = export::sanitize(&st.export_name);
            st.export_name = name.clone();
            out = Some(Action::Export(s.export_format, name));
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            if ui.add(egui::Button::new("Copy image").rounding(8.0).min_size(vec2(0.0, 32.0))).on_hover_text("Copy the styled sheet to the clipboard").clicked() {
                out = Some(Action::CopyImage);
            }
            if ui.add(egui::Button::new("Copy text").rounding(8.0).min_size(vec2(0.0, 32.0))).clicked() {
                out = Some(Action::CopyText);
            }
        });
    });

    if let Some((msg, err)) = &st.export_status {
        ui.add_space(8.0);
        let col = if *err { Color32::from_rgb(220, 90, 80) } else { theme::muted() };
        ui.add(egui::Label::new(RichText::new(msg.as_str()).size(12.0).color(col)).wrap());
    }
    out
}

fn about_popup(ui: &mut egui::Ui, s: &mut Settings, upd: &Status) -> Option<Action> {
    let mut out = None;
    ui.set_width(300.0);
    heading(ui, "ABOUT & UPDATES");
    ui.label(RichText::new(format!("Rustype {}", update::CURRENT)).size(16.0).color(theme::text()));
    ui.add_space(8.0);

    let primary = |ui: &mut egui::Ui, label: &str| {
        ui.add(egui::Button::new(RichText::new(label).color(Color32::WHITE).strong()).fill(theme::accent()).rounding(8.0).min_size(vec2(0.0, 30.0)))
            .clicked()
    };
    let plain = |ui: &mut egui::Ui, label: &str| ui.add(egui::Button::new(label).rounding(8.0).min_size(vec2(0.0, 30.0))).clicked();
    let note = |ui: &mut egui::Ui, msg: &str, err: bool| {
        let col = if err { Color32::from_rgb(220, 90, 80) } else { theme::muted() };
        ui.add(egui::Label::new(RichText::new(msg).size(12.5).color(col)).wrap());
    };

    match upd {
        Status::Idle => {
            if plain(ui, "Check for updates") {
                out = Some(Action::CheckUpdate);
            }
        }
        Status::Checking => {
            ui.horizontal(|ui| {
                ui.spinner();
                note(ui, "Checking GitHub\u{2026}", false);
            });
        }
        Status::UpToDate => {
            note(ui, "You have the latest version.", false);
            ui.add_space(4.0);
            if plain(ui, "Check again") {
                out = Some(Action::CheckUpdate);
            }
        }
        Status::Available(rel) => {
            note(ui, &format!("Version {} is available.", rel.version), false);
            let summary = notes_preview(&rel.notes);
            if !summary.is_empty() {
                ui.add_space(4.0);
                Frame::none().fill(theme::card()).rounding(8.0).inner_margin(Margin::same(10.0)).show(ui, |ui| {
                    ui.set_width(280.0);
                    ui.add(egui::Label::new(RichText::new(summary).size(12.0).color(theme::muted())).wrap());
                });
            }
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if primary(ui, &format!("Update to {}", rel.version)) {
                    out = Some(Action::InstallUpdate);
                }
                if plain(ui, "Release notes") {
                    out = Some(Action::OpenUrl(rel.page.clone()));
                }
            });
            note(ui, "Your sheets are saved and Rustype restarts on its own.", false);
        }
        Status::Downloading(rel, got, total) => {
            note(ui, &format!("Downloading {}\u{2026}", rel.version), false);
            let frac = if *total > 0 { *got as f32 / *total as f32 } else { 0.0 };
            ui.add(egui::ProgressBar::new(frac).desired_width(280.0).show_percentage());
        }
        Status::Installed => {
            ui.horizontal(|ui| {
                ui.spinner();
                note(ui, "Installed. Restarting\u{2026}", false);
            });
        }
        Status::Failed(e) => {
            note(ui, e, true);
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if plain(ui, "Try again") {
                    out = Some(Action::CheckUpdate);
                }
                if plain(ui, "Open releases") {
                    out = Some(Action::OpenUrl("https://github.com/MusAhmedd/rustype/releases".into()));
                }
            });
        }
    }

    ui.add_space(8.0);
    ui.separator();
    icons::toggle(ui, &mut s.auto_update_check, "Check automatically at launch");
    out
}

/// First lines of the release notes as plain text, about 240 chars at most.
fn notes_preview(notes: &str) -> String {
    let plain: Vec<String> = notes
        .lines()
        .map(|l| l.trim().trim_start_matches(['-', '*', ' ']).replace("**", "").replace('`', ""))
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect();
    let mut out = String::new();
    for l in plain {
        if out.chars().count() + l.chars().count() > 240 {
            if out.is_empty() {
                out = l.chars().take(237).collect::<String>() + "\u{2026}";
            }
            break;
        }
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(&l);
    }
    out
}
