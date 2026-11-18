//! Dark minimal theme with a dim low-light variant, plus embedded fonts.

use eframe::egui::{self, Color32, FontData, FontDefinitions, FontFamily, Rounding, Stroke};
use std::sync::atomic::{AtomicBool, Ordering};

static LOW: AtomicBool = AtomicBool::new(false);

fn pick(normal: Color32, low: Color32) -> Color32 {
    if LOW.load(Ordering::Relaxed) {
        low
    } else {
        normal
    }
}

pub fn bg() -> Color32 {
    pick(Color32::from_rgb(18, 18, 21), Color32::from_rgb(8, 8, 9))
}
pub fn bar() -> Color32 {
    pick(Color32::from_rgb(24, 24, 28), Color32::from_rgb(12, 12, 13))
}
pub fn card() -> Color32 {
    pick(Color32::from_rgb(30, 30, 35), Color32::from_rgb(18, 18, 20))
}
pub fn tab_active() -> Color32 {
    pick(Color32::from_rgb(44, 44, 51), Color32::from_rgb(30, 30, 33))
}
pub fn border() -> Color32 {
    pick(Color32::from_rgb(48, 48, 56), Color32::from_rgb(34, 34, 38))
}
pub fn text() -> Color32 {
    pick(Color32::from_rgb(232, 232, 236), Color32::from_rgb(150, 150, 156))
}
pub fn muted() -> Color32 {
    pick(Color32::from_rgb(140, 140, 152), Color32::from_rgb(92, 92, 100))
}
pub fn accent() -> Color32 {
    pick(Color32::from_rgb(214, 140, 72), Color32::from_rgb(150, 110, 70))
}

/// Embedded typewriter fonts: (family name, file bytes).
const FONTS: [(&str, &[u8]); 5] = [
    ("SpecialElite", include_bytes!("../assets/fonts/SpecialElite-Regular.ttf")),
    ("CourierPrime", include_bytes!("../assets/fonts/CourierPrime-Regular.ttf")),
    ("CutiveMono", include_bytes!("../assets/fonts/CutiveMono-Regular.ttf")),
    ("SpaceMono", include_bytes!("../assets/fonts/SpaceMono-Regular.ttf")),
    ("PlexMono", include_bytes!("../assets/fonts/IBMPlexMono-Regular.ttf")),
];

pub fn install_fonts(ctx: &egui::Context) {
    let mut defs = FontDefinitions::default();
    let fallback = defs.families[&FontFamily::Monospace].clone();
    for (name, bytes) in FONTS {
        defs.font_data.insert(name.into(), FontData::from_static(bytes));
        let mut chain = vec![name.to_string()];
        chain.extend(fallback.iter().cloned());
        defs.families.insert(FontFamily::Name(name.into()), chain);
    }
    ctx.set_fonts(defs);
}

pub fn apply(ctx: &egui::Context, low: bool) {
    LOW.store(low, Ordering::Relaxed);
    let mut v = egui::Visuals::dark();
    v.panel_fill = bg();
    v.window_fill = bar();
    v.extreme_bg_color = card();
    v.faint_bg_color = card();
    v.window_rounding = Rounding::same(12.0);
    v.window_stroke = Stroke::new(1.0_f32, border());
    v.selection.bg_fill = accent().linear_multiply(0.6);
    v.selection.stroke = Stroke::new(1.0_f32, accent());
    v.hyperlink_color = accent();
    for w in [&mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.noninteractive] {
        w.rounding = Rounding::same(8.0);
    }
    v.widgets.noninteractive.fg_stroke.color = text();
    v.widgets.inactive.fg_stroke.color = text();
    v.widgets.hovered.fg_stroke.color = text();
    v.widgets.inactive.bg_fill = tab_active();
    v.widgets.inactive.weak_bg_fill = card();
    v.widgets.hovered.weak_bg_fill = tab_active();
    v.widgets.hovered.bg_fill = tab_active();
    v.widgets.noninteractive.bg_stroke.color = border();
    v.override_text_color = if low { Some(text()) } else { None };
    ctx.set_visuals(v);

    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::vec2(8.0, 8.0);
    style.spacing.button_padding = egui::vec2(10.0, 5.0);
    ctx.set_style(style);
}

/// Raw TTF bytes for a typeface, used when rendering exports.
pub fn font_bytes(f: crate::settings::TypeFont) -> std::borrow::Cow<'static, [u8]> {
    use crate::settings::TypeFont;
    let name = match f {
        TypeFont::Standard => return hack_bytes(),
        TypeFont::SpecialElite => "SpecialElite",
        TypeFont::CourierPrime => "CourierPrime",
        TypeFont::CutiveMono => "CutiveMono",
        TypeFont::SpaceMono => "SpaceMono",
        TypeFont::PlexMono => "PlexMono",
    };
    FONTS.iter().find(|(n, _)| *n == name).map(|(_, b)| std::borrow::Cow::Borrowed(*b)).unwrap_or_else(hack_bytes)
}

/// egui's built-in monospace font (Hack), also the glyph fallback.
pub fn hack_bytes() -> std::borrow::Cow<'static, [u8]> {
    FontDefinitions::default().font_data.get("Hack").map(|d| d.font.clone()).unwrap_or_default()
}
