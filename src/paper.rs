//! Draws the typewriter: paper rising out of the platen, the machine body
//! with its typebar basket and ribbon spools, and the optional focus light.
//! The typing point stays centred; the paper moves left as you type and up
//! (turning the platen) on each return.

use crate::icons::{mix, vgrad};
use crate::editor::VLine;
use crate::settings::Settings;
use eframe::egui::{
    self, epaint::Mesh, epaint::Shadow, pos2, vec2, Align2, Color32, ColorImage, FontId, Painter, Pos2, Rect, Sense, Stroke,
    TextureHandle, TextureOptions, Vec2,
};
use std::f32::consts::{PI, TAU};

pub const LINE_COLS: usize = 64;
pub const BELL_COL: usize = 58;

const FONT_SIZE: f32 = 22.0;
pub const LINE_H: f32 = 33.0;
/// Distance from the bottom edge to the typing line.
const BOTTOM_GAP: f32 = 230.0;
const PLATEN_H: f32 = 38.0;
const STRIKE_SECS: f32 = 0.22;
const TYPEBARS: usize = 23;
const GRAIN: usize = 256;

fn hash(a: usize, b: usize, c: u32) -> u32 {
    let mut h = (a as u32).wrapping_mul(0x9E37_79B9) ^ (b as u32).wrapping_mul(0x85EB_CA6B) ^ c.wrapping_mul(0xC2B2_AE35);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h
}

fn unit(h: u32, shift: u32) -> f32 {
    ((h >> shift) & 0xFF) as f32 / 255.0
}

fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn is_dark(c: [u8; 3]) -> bool {
    (0.299 * c[0] as f32 + 0.587 * c[1] as f32 + 0.114 * c[2] as f32) < 110.0
}

/// Last key strike: seconds since it happened and the character typed.
pub struct Strike {
    pub age: f32,
    pub ch: char,
}

impl Strike {
    fn phase(&self) -> f32 {
        self.age / STRIKE_SECS
    }

    /// Typebar travel: accelerates up, dwells on impact, falls back with a
    /// small rebound in the basket (can dip slightly below 0).
    fn swing(&self) -> f32 {
        let p = self.phase();
        if p >= 1.0 {
            0.0
        } else if p < 0.3 {
            (p / 0.3).powi(2)
        } else if p < 0.38 {
            1.0
        } else {
            let q = (p - 0.38) / 0.62;
            // ease-out-back, inverted
            let c = 1.6;
            let e = 1.0 + (c + 1.0) * (q - 1.0).powi(3) + c * (q - 1.0).powi(2);
            1.0 - e
        }
    }

    /// Short paper jolt at the moment of impact.
    fn impact(&self) -> f32 {
        let p = self.phase();
        if (0.28..0.5).contains(&p) {
            (PI * (p - 0.28) / 0.22).sin()
        } else {
            0.0
        }
    }
}

/// What to draw on the sheet.
pub struct View<'a> {
    pub chars: &'a [char],
    pub lines: &'a [VLine],
    /// Where the carriage sits (line, column).
    pub carriage: (usize, usize),
    /// Where the blinking cursor is drawn.
    pub caret: (usize, usize),
    pub caret_on: bool,
    pub overtype: bool,
    /// Selected char range.
    pub selection: Option<(usize, usize)>,
    /// Read-back offset in lines.
    pub scroll: f32,
    /// 0..1 warm glow at the type point when the writing goal is reached.
    pub glow: f32,
}

/// Paper geometry from the last frame, for mouse hit testing.
pub struct Sheet {
    pub response: egui::Response,
    origin: Pos2,
    cw: f32,
    /// Bottom of the visible paper (just above the platen).
    paper_bottom: f32,
}

impl Sheet {
    /// (line, column) under a screen position; may be out of range.
    pub fn line_col_at(&self, p: Pos2) -> (isize, isize) {
        let line = ((p.y - self.origin.y) / LINE_H).ceil() as isize;
        let col = ((p.x - self.origin.x) / self.cw).round() as isize;
        (line, col)
    }

    pub fn on_paper(&self, p: Pos2) -> bool {
        self.response.rect.contains(p) && p.y <= self.paper_bottom
    }
}

pub fn draw(ui: &mut egui::Ui, v: &View, s: &Settings, strike: Option<Strike>) -> Sheet {
    let (rect, response) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
    let painter = ui.painter_at(rect);
    let ctx = ui.ctx().clone();
    let font = FontId::new(FONT_SIZE, s.font.family());
    let cw = ui.fonts(|f| f.glyph_width(&font, 'M'));

    let dark = is_dark(s.paper);
    let paper = Color32::from_rgb(s.paper[0], s.paper[1], s.paper[2]);
    let ink: (u8, u8, u8) = if dark { (232, 226, 212) } else { (26, 24, 30) };
    let inked = |a: u8| Color32::from_rgba_unmultiplied(ink.0, ink.1, ink.2, a);

    let (cur_line, cur_col) = v.carriage;
    let col_anim = ctx.animate_value_with_time(egui::Id::new("carriage_col"), cur_col as f32, 0.05);
    let line_anim = ctx.animate_value_with_time(egui::Id::new("carriage_line"), cur_line as f32 - v.scroll, 0.2);
    let spool = ctx.animate_value_with_time(egui::Id::new("spool"), v.chars.len() as f32, 0.15);

    let swing = strike.as_ref().map_or(0.0, Strike::swing);
    let impact = strike.as_ref().map_or(0.0, Strike::impact);
    if strike.as_ref().is_some_and(|k| k.phase() < 1.1) {
        ctx.request_repaint();
    }

    let point = pos2(rect.center().x, rect.bottom() - BOTTOM_GAP);
    let platen_top = point.y + 5.0;
    let platen_bot = platen_top + PLATEN_H;
    let tip = point.x + cw * 0.5;
    let origin = pos2(point.x - col_anim * cw, point.y - line_anim * LINE_H - 1.3 * impact);

    // ---- Paper -------------------------------------------------------
    let paper_rect = Rect::from_min_max(rect.min, pos2(rect.right(), platen_top + 4.0));
    painter.rect_filled(paper_rect, 0.0, paper);
    grain(&painter, &ctx, paper_rect, origin, dark);
    // Soft falloff at the top of the page.
    vgrad(
        &painter,
        Rect::from_min_size(rect.min, vec2(rect.width(), 70.0)),
        &[(0.0, Color32::from_black_alpha(26)), (1.0, Color32::TRANSPARENT)],
    );

    let visible = |y: f32| y >= rect.top() - LINE_H && y <= platen_top + 8.0;

    // Selection: a soft highlight behind the ink.
    if let Some((a, b)) = v.selection {
        let sel = if dark { Color32::from_rgba_unmultiplied(120, 160, 230, 70) } else { Color32::from_rgba_unmultiplied(70, 120, 210, 55) };
        for (l, vl) in v.lines.iter().enumerate() {
            let y = origin.y + l as f32 * LINE_H;
            let end = vl.start + vl.len;
            if !visible(y) || b <= vl.start || a > end {
                continue;
            }
            let c0 = a.max(vl.start) - vl.start;
            // Show the selected line break as one extra cell.
            let c1 = if b > end { vl.len + usize::from(vl.para_end) } else { b - vl.start };
            if c1 > c0 {
                let r = Rect::from_min_max(pos2(origin.x + c0 as f32 * cw, y - LINE_H * 0.84), pos2(origin.x + c1 as f32 * cw, y + 3.0));
                painter.rect_filled(r, 2.0, sel);
            }
        }
    }

    // Typed glyphs with ink jitter.
    for (l, vl) in v.lines.iter().enumerate() {
        let y = origin.y + l as f32 * LINE_H;
        if !visible(y) {
            continue;
        }
        for (c, &ch) in v.chars[vl.start..vl.start + vl.len].iter().enumerate() {
            if ch.is_whitespace() {
                continue;
            }
            let h = hash(l, c, ch as u32);
            let dx = (unit(h, 0) - 0.5) * 1.6;
            let dy = (unit(h, 8) - 0.5) * 1.8;
            let strength = 0.62 + 0.38 * unit(h, 16).powf(0.4);
            let pos = pos2(origin.x + c as f32 * cw + dx, y + dy);
            painter.text(pos, Align2::LEFT_BOTTOM, ch, font.clone(), inked((strength * 255.0) as u8));
            if unit(h, 24) > 0.85 {
                painter.text(pos + vec2(0.6, 0.3), Align2::LEFT_BOTTOM, ch, font.clone(), inked(70));
            }
        }
    }

    // Cursor: a thin bar for insert, an underline for overtype.
    if v.caret_on {
        let (l, c) = v.caret;
        let y = origin.y + l as f32 * LINE_H;
        let x = origin.x + c as f32 * cw;
        if visible(y) {
            if v.overtype {
                painter.rect_filled(Rect::from_min_max(pos2(x, y + 1.0), pos2(x + cw, y + 3.5)), 1.0, inked(170));
            } else {
                painter.rect_filled(Rect::from_min_max(pos2(x - 0.75, y - LINE_H * 0.78), pos2(x + 0.75, y + 2.0)), 0.0, inked(150));
            }
        }
    }

    // Paper curving into the platen: shading just above it.
    vgrad(
        &painter,
        Rect::from_min_max(pos2(rect.left(), platen_top - 16.0), pos2(rect.right(), platen_top + 4.0)),
        &[(0.0, Color32::TRANSPARENT), (1.0, Color32::from_black_alpha(55))],
    );

    platen(&painter, rect, platen_top, platen_bot, line_anim);
    scale(&painter, rect, origin, tip, platen_top, cw);
    body(&painter, rect, point, platen_bot, spool);
    typebars(&painter, point, platen_bot, pos2(tip, point.y - FONT_SIZE * 0.36), strike.as_ref(), swing);
    ribbon_vibrator(&painter, tip, platen_top, cw, swing);

    if s.light_on {
        torch(&painter, rect, pos2(tip, platen_top), s.light_strength);
    }
    if v.glow > 0.0 {
        goal_glow(&painter, pos2(tip, platen_top), v.glow);
    }
    if response.hovered() && ctx.input(|i| i.pointer.hover_pos()).is_some_and(|p| p.y <= platen_top) {
        ctx.set_cursor_icon(egui::CursorIcon::Text);
    }
    Sheet { response, origin, cw, paper_bottom: platen_top }
}

/// Tiled paper-fibre texture, moving with the sheet.
fn grain(painter: &Painter, ctx: &egui::Context, r: Rect, origin: Pos2, dark: bool) {
    let id = egui::Id::new("paper_grain");
    // Look up the cached texture first; loading must happen outside the
    // context lock or it deadlocks.
    let cached: Option<TextureHandle> = ctx.data(|d| d.get_temp(id));
    let tex = cached.unwrap_or_else(|| {
        let tex = {
            let mut img = ColorImage::new([GRAIN, GRAIN], Color32::TRANSPARENT);
            let mut seed = 0x1234_5678u32;
            let mut rnd = || {
                seed ^= seed << 13;
                seed ^= seed >> 17;
                seed ^= seed << 5;
                seed as f32 / u32::MAX as f32
            };
            for px in img.pixels.iter_mut() {
                let n = rnd();
                *px = if n > 0.5 {
                    Color32::from_white_alpha(((n - 0.5) * 9.0) as u8)
                } else {
                    Color32::from_black_alpha(((0.5 - n) * 8.0) as u8)
                };
            }
            // Fibres: short faint diagonal streaks.
            for _ in 0..500 {
                let (x, y) = ((rnd() * GRAIN as f32) as i32, (rnd() * GRAIN as f32) as i32);
                let len = 4 + (rnd() * 14.0) as i32;
                let slope = rnd() * 0.6 - 0.3;
                let a = (3.0 + rnd() * 6.0) as u8;
                for k in 0..len {
                    let px = (x + k).rem_euclid(GRAIN as i32) as usize;
                    let py = (y + (k as f32 * slope) as i32).rem_euclid(GRAIN as i32) as usize;
                    img.pixels[py * GRAIN + px] = Color32::from_black_alpha(a);
                }
            }
            ctx.load_texture("paper_grain", img, TextureOptions::LINEAR_REPEAT)
        };
        ctx.data_mut(|d| d.insert_temp(id, tex.clone()));
        tex
    });
    let g = GRAIN as f32;
    let off = vec2(origin.x.rem_euclid(g), origin.y.rem_euclid(g));
    let uv = Rect::from_min_max(pos2(-off.x / g, -off.y / g), pos2((r.width() - off.x) / g, (r.height() - off.y) / g));
    let mut m = Mesh::with_texture(tex.id());
    let tint = if dark { Color32::from_white_alpha(28) } else { Color32::from_white_alpha(150) };
    m.add_rect_with_uv(r, uv, tint);
    painter.add(m);
}

/// Rubber platen roller with ridges that roll as lines feed.
fn platen(p: &Painter, rect: Rect, top: f32, bot: f32, line_anim: f32) {
    let r = Rect::from_min_max(pos2(rect.left(), top), pos2(rect.right(), bot));
    vgrad(
        p,
        r,
        &[
            (0.0, Color32::from_rgb(12, 12, 14)),
            (0.18, Color32::from_rgb(40, 40, 44)),
            (0.36, Color32::from_rgb(66, 66, 71)),
            (0.55, Color32::from_rgb(36, 36, 40)),
            (1.0, Color32::from_rgb(9, 9, 11)),
        ],
    );
    // Ridges projected onto the cylinder; they travel upward as paper feeds.
    let spacing = 0.075;
    let off = (line_anim * 0.55).rem_euclid(spacing);
    let mut u = -off;
    while u < 1.0 {
        if u > 0.0 {
            let y = bot - r.height() * (0.5 - 0.5 * (PI * u).cos());
            let a = (PI * u).sin() * 34.0;
            p.line_segment([pos2(r.left(), y), pos2(r.right(), y)], Stroke::new(1.0f32, Color32::from_black_alpha(a as u8)));
        }
        u += spacing;
    }
}

/// Paper scale on the carriage: ticks move with the paper, pointer is fixed.
fn scale(p: &Painter, rect: Rect, origin: Pos2, tip: f32, top: f32, cw: f32) {
    let half = (rect.width() * 0.32).min(300.0);
    let bar = Rect::from_min_max(pos2(tip - half, top - 1.0), pos2(tip + half, top + 8.0));
    vgrad(
        p,
        bar,
        &[(0.0, Color32::from_rgba_unmultiplied(220, 222, 230, 70)), (1.0, Color32::from_rgba_unmultiplied(120, 122, 130, 40))],
    );
    let first = ((bar.left() - origin.x) / cw).ceil() as i32;
    let last = ((bar.right() - origin.x) / cw).floor() as i32;
    for k in first..=last {
        let x = origin.x + k as f32 * cw;
        let fade = 1.0 - ((x - tip).abs() / half).powi(2);
        let h = if k % 5 == 0 { 6.0 } else { 3.0 };
        p.line_segment([pos2(x, bar.top()), pos2(x, bar.top() + h)], Stroke::new(1.0f32, Color32::from_black_alpha((110.0 * fade) as u8)));
    }
    // Fixed pointer.
    p.add(egui::Shape::convex_polygon(
        vec![pos2(tip, top), pos2(tip - 5.5, top + 10.0), pos2(tip + 5.5, top + 10.0)],
        Color32::from_rgb(222, 172, 86),
        Stroke::new(1.0f32, Color32::from_rgb(120, 84, 30)),
    ));
}

/// Enamel machine body with the basket opening and ribbon spools.
fn body(p: &Painter, rect: Rect, point: Pos2, top: f32, spool: f32) {
    let r = Rect::from_min_max(pos2(rect.left(), top), rect.max);
    vgrad(
        p,
        r,
        &[
            (0.0, Color32::from_rgb(52, 53, 58)),
            (0.03, Color32::from_rgb(34, 35, 39)),
            (0.5, Color32::from_rgb(24, 24, 27)),
            (1.0, Color32::from_rgb(14, 14, 16)),
        ],
    );
    p.line_segment([pos2(r.left(), top + 0.5), pos2(r.right(), top + 0.5)], Stroke::new(1.5f32, Color32::from_rgb(150, 150, 158)));
    vgrad(
        p,
        Rect::from_min_size(pos2(r.left(), top + 1.0), vec2(r.width(), 8.0)),
        &[(0.0, Color32::from_black_alpha(90)), (1.0, Color32::TRANSPARENT)],
    );

    // Basket opening.
    let (rx, ry) = (190.0, 112.0);
    let half_ellipse = |sx: f32, sy: f32| -> Vec<Pos2> {
        (0..=48).map(|i| {
            let a = PI * i as f32 / 48.0;
            pos2(point.x + a.cos() * sx, top + a.sin() * sy)
        })
        .collect()
    };
    p.add(egui::Shape::convex_polygon(half_ellipse(rx, ry), Color32::from_rgb(9, 9, 10), Stroke::NONE));
    p.add(egui::Shape::line(half_ellipse(rx + 1.5, ry + 1.5), Stroke::new(1.5f32, Color32::from_rgb(70, 70, 76))));

    // Ribbon spools on either side.
    for side in [-1.0f32, 1.0] {
        let c = pos2(point.x + side * (rx + 95.0), top + 52.0);
        p.add(Shadow { offset: vec2(0.0, 4.0), blur: 12.0, spread: 0.0, color: Color32::from_black_alpha(120) }.as_shape(
            Rect::from_center_size(c, Vec2::splat(64.0)),
            32.0,
        ));
        p.circle_filled(c, 32.0, Color32::from_rgb(28, 28, 31));
        p.circle_filled(c, 26.0, Color32::from_rgb(58, 22, 24)); // wound ribbon
        p.circle_stroke(c, 26.0, Stroke::new(1.0f32, Color32::from_black_alpha(120)));
        p.circle_filled(c, 11.0, Color32::from_rgb(150, 150, 158));
        let a0 = spool * 0.09 * side;
        for k in 0..3 {
            let a = a0 + TAU * k as f32 / 3.0;
            p.line_segment([c, c + vec2(a.cos(), a.sin()) * 10.0], Stroke::new(2.0f32, Color32::from_rgb(70, 70, 76)));
        }
        p.circle_filled(c, 3.0, Color32::from_rgb(200, 200, 208));
    }
    // Ribbon running from the spools to the guide.
    for side in [-1.0f32, 1.0] {
        let from = pos2(point.x + side * (rx + 95.0), top + 22.0);
        let to = pos2(point.x + side * 26.0, top - 18.0);
        p.line_segment([from, to], Stroke::new(3.0f32, Color32::from_rgba_unmultiplied(70, 22, 26, 200)));
    }
}

/// Typebars resting in the basket; the struck one swings up to the paper.
fn typebars(p: &Painter, point: Pos2, top: f32, strike_at: Pos2, strike: Option<&Strike>, swing: f32) {
    let centre = pos2(point.x, top + 2.0);
    let struck = strike.map(|k| hash(k.ch as usize, 7, 3) as usize % TYPEBARS);
    let pivot = |i: usize| {
        let a = (12.0 + 156.0 * i as f32 / (TYPEBARS - 1) as f32).to_radians();
        centre + vec2(a.cos() * 168.0, a.sin() * 96.0)
    };
    let arc: Vec<Pos2> = (0..TYPEBARS).map(pivot).collect();
    p.add(egui::Shape::line(arc.clone(), Stroke::new(6.0f32, Color32::from_rgb(40, 40, 45))));

    let bar = |from: Pos2, to: Pos2, bright: f32| {
        let base = mix(Color32::from_rgb(84, 84, 92), Color32::from_rgb(176, 176, 186), bright);
        p.line_segment([from, to], Stroke::new(3.0f32, Color32::from_black_alpha(140)));
        p.line_segment([from, to], Stroke::new(2.0f32, base));
        let dir = (to - from).normalized();
        let slug = Rect::from_center_size(to, vec2(7.0, 6.0));
        p.rect_filled(slug.translate(dir * 1.0), 1.5, mix(Color32::from_rgb(100, 100, 108), Color32::from_rgb(210, 210, 218), bright));
    };

    for (i, &pv) in arc.iter().enumerate() {
        if Some(i) == struck && swing != 0.0 {
            continue;
        }
        bar(pv, pv + (centre - pv) * 0.36, 0.0);
    }
    if let (Some(i), true) = (struck, swing != 0.0) {
        let pv = arc[i];
        let rest = pv + (centre - pv) * 0.36;
        let to = rest + (strike_at - rest) * swing;
        bar(pv, to, swing.clamp(0.0, 1.0));
    }
}

/// Ribbon guide that lifts the ribbon in front of the paper on each strike.
fn ribbon_vibrator(p: &Painter, tip: f32, platen_top: f32, cw: f32, swing: f32) {
    let lift = 22.0 * swing.clamp(0.0, 1.0);
    let w = cw * 2.0;
    let y = platen_top + 12.0 - lift;
    let ribbon = Rect::from_min_size(pos2(tip - w / 2.0, y), vec2(w, 11.0));
    let (a, b) = ribbon.split_top_bottom_at_fraction(0.5);
    p.rect_filled(a, 1.0, Color32::from_rgb(28, 26, 30));
    p.rect_filled(b, 1.0, Color32::from_rgb(140, 32, 34));
    // Guide frame.
    let s = Stroke::new(1.5f32, Color32::from_rgb(150, 150, 160));
    p.line_segment([pos2(ribbon.left() - 2.0, ribbon.top() - 1.0), pos2(ribbon.left() - 2.0, ribbon.bottom() + 6.0)], s);
    p.line_segment([pos2(ribbon.right() + 2.0, ribbon.top() - 1.0), pos2(ribbon.right() + 2.0, ribbon.bottom() + 6.0)], s);
}

/// Soft gold halo around the type point, drawn as a radial mesh.
fn goal_glow(painter: &Painter, center: Pos2, t: f32) {
    const SEG: u32 = 96;
    let radii = [0.0, 30.0, 70.0, 120.0, 180.0, 260.0];
    let alpha = [0.34, 0.26, 0.15, 0.07, 0.02, 0.0];
    let mut m = Mesh::default();
    for (r, a) in radii.iter().zip(alpha) {
        let c = Color32::from_rgba_unmultiplied(255, 196, 92, (a * t * 255.0) as u8);
        for s in 0..SEG {
            let ang = TAU * s as f32 / SEG as f32;
            m.colored_vertex(center + vec2(ang.cos(), ang.sin()) * *r, c);
        }
    }
    for ring in 0..radii.len() as u32 - 1 {
        for s in 0..SEG {
            let a = ring * SEG + s;
            let b = ring * SEG + (s + 1) % SEG;
            m.add_triangle(a, b, a + SEG);
            m.add_triangle(b, b + SEG, a + SEG);
        }
    }
    painter.add(m);
}

/// Dims everything except a small, subtle V-shaped beam rising from the
/// pointer tip. Built as a polar mesh around the tip for smooth edges.
fn torch(painter: &Painter, rect: Rect, apex: Pos2, strength: f32) {
    const SEG: u32 = 240;
    const HALF_ANGLE: f32 = 0.42; // ~24 degrees each side
    let darkness = 0.38 + 0.32 * strength;
    let far = rect.width().hypot(rect.height()) * 1.2;
    let radii = [0.0, 3.0, 7.0, 12.0, 19.0, 28.0, 40.0, 55.0, 72.0, 92.0, 115.0, 140.0, 168.0, 200.0, 240.0, 300.0, 400.0, far];

    let light = |dir: Vec2, d: f32| {
        let ang = dir.x.abs().atan2(-dir.y); // 0 = straight up
        let cone = 1.0 - smoothstep(HALF_ANGLE - 0.2, HALF_ANGLE + 0.2, ang);
        let range = 1.0 - smoothstep(50.0, 230.0, d);
        let spill = 1.0 - smoothstep(4.0, 28.0, d);
        (cone * range + spill * 0.6).clamp(0.0, 1.0)
    };

    let mut m = Mesh::default();
    for &r in &radii {
        for s in 0..SEG {
            let a = TAU * s as f32 / SEG as f32;
            let dir = vec2(a.cos(), a.sin());
            let alpha = darkness * (1.0 - light(dir, r));
            m.colored_vertex(apex + dir * r, Color32::from_black_alpha((alpha * 255.0) as u8));
        }
    }
    for ring in 0..radii.len() as u32 - 1 {
        for s in 0..SEG {
            let a = ring * SEG + s;
            let b = ring * SEG + (s + 1) % SEG;
            m.add_triangle(a, b, a + SEG);
            m.add_triangle(b, b + SEG, a + SEG);
        }
    }
    painter.add(m);
}
