//! Small vector icons and shared custom widgets.

use crate::theme;
use eframe::egui::{self, epaint::Mesh, pos2, vec2, Color32, Painter, Pos2, Rect, Response, Sense, Shape, Stroke, Ui, Vec2};
use std::f32::consts::TAU;

fn at(r: Rect, x: f32, y: f32) -> Pos2 {
    pos2(r.left() + x * r.width(), r.top() + y * r.height())
}

fn stroke(c: Color32) -> Stroke {
    Stroke::new(1.6f32, c)
}

pub fn home(p: &Painter, r: Rect, c: Color32) {
    p.add(Shape::line(vec![at(r, 0.1, 0.5), at(r, 0.5, 0.14), at(r, 0.9, 0.5)], stroke(c)));
    p.add(Shape::line(vec![at(r, 0.22, 0.42), at(r, 0.22, 0.86), at(r, 0.78, 0.86), at(r, 0.78, 0.42)], stroke(c)));
    p.rect_filled(Rect::from_min_max(at(r, 0.42, 0.6), at(r, 0.58, 0.86)), 1.0, c);
}

pub fn plus(p: &Painter, r: Rect, c: Color32) {
    p.line_segment([at(r, 0.5, 0.2), at(r, 0.5, 0.8)], stroke(c));
    p.line_segment([at(r, 0.2, 0.5), at(r, 0.8, 0.5)], stroke(c));
}

pub fn close(p: &Painter, r: Rect, c: Color32) {
    p.line_segment([at(r, 0.25, 0.25), at(r, 0.75, 0.75)], stroke(c));
    p.line_segment([at(r, 0.75, 0.25), at(r, 0.25, 0.75)], stroke(c));
}

pub fn minimize(p: &Painter, r: Rect, c: Color32) {
    p.line_segment([at(r, 0.2, 0.5), at(r, 0.8, 0.5)], Stroke::new(1.0f32, c));
}

pub fn maximize(p: &Painter, r: Rect, c: Color32, restored: bool) {
    let s = Stroke::new(1.0f32, c);
    if restored {
        p.rect_stroke(Rect::from_min_max(at(r, 0.32, 0.2), at(r, 0.8, 0.68)), 1.0, s);
        p.rect_filled(Rect::from_min_max(at(r, 0.2, 0.32), at(r, 0.68, 0.8)), 1.0, theme::bar());
        p.rect_stroke(Rect::from_min_max(at(r, 0.2, 0.32), at(r, 0.68, 0.8)), 1.0, s);
    } else {
        p.rect_stroke(Rect::from_min_max(at(r, 0.22, 0.22), at(r, 0.78, 0.78)), 1.0, s);
    }
}

pub fn search(p: &Painter, r: Rect, c: Color32) {
    p.circle_stroke(at(r, 0.43, 0.43), r.width() * 0.27, stroke(c));
    p.line_segment([at(r, 0.63, 0.63), at(r, 0.86, 0.86)], Stroke::new(2.0f32, c));
}

pub fn trash(p: &Painter, r: Rect, c: Color32) {
    let s = Stroke::new(1.4f32, c);
    p.line_segment([at(r, 0.18, 0.27), at(r, 0.82, 0.27)], s);
    p.add(Shape::line(vec![at(r, 0.38, 0.27), at(r, 0.4, 0.15), at(r, 0.6, 0.15), at(r, 0.62, 0.27)], s));
    p.add(Shape::line(vec![at(r, 0.26, 0.3), at(r, 0.31, 0.88), at(r, 0.69, 0.88), at(r, 0.74, 0.3)], s));
    p.line_segment([at(r, 0.44, 0.42), at(r, 0.44, 0.76)], s);
    p.line_segment([at(r, 0.56, 0.42), at(r, 0.56, 0.76)], s);
}

pub fn speaker(p: &Painter, r: Rect, c: Color32, muted: bool) {
    p.add(Shape::convex_polygon(
        vec![at(r, 0.14, 0.38), at(r, 0.3, 0.38), at(r, 0.5, 0.2), at(r, 0.5, 0.8), at(r, 0.3, 0.62), at(r, 0.14, 0.62)],
        c,
        Stroke::NONE,
    ));
    if muted {
        p.line_segment([at(r, 0.62, 0.38), at(r, 0.86, 0.62)], stroke(c));
        p.line_segment([at(r, 0.86, 0.38), at(r, 0.62, 0.62)], stroke(c));
    } else {
        for (rad, a) in [(0.18, 0.7), (0.32, 0.6)] {
            let pts: Vec<Pos2> = (0..=10)
                .map(|i| {
                    let t = -a + 2.0 * a * i as f32 / 10.0;
                    let ctr = at(r, 0.5, 0.5);
                    ctr + vec2(t.cos(), t.sin()) * rad * r.width()
                })
                .collect();
            p.add(Shape::line(pts, Stroke::new(1.4f32, c)));
        }
    }
}

pub fn bulb(p: &Painter, r: Rect, c: Color32, lit: bool) {
    let ctr = at(r, 0.5, 0.4);
    let rad = r.width() * 0.24;
    if lit {
        for i in 0..6 {
            let a = -TAU * 0.5 + TAU * 0.1 * (i as f32 - 0.0) + 0.1;
            let d = vec2(a.cos(), a.sin());
            if d.y > 0.2 {
                continue;
            }
            p.line_segment([ctr + d * (rad + 3.0), ctr + d * (rad + 6.0)], Stroke::new(1.3f32, c));
        }
        p.circle_filled(ctr, rad, c);
    } else {
        p.circle_stroke(ctr, rad, stroke(c));
    }
    p.line_segment([at(r, 0.4, 0.72), at(r, 0.6, 0.72)], stroke(c));
    p.line_segment([at(r, 0.43, 0.83), at(r, 0.57, 0.83)], stroke(c));
}

/// Two-stop color interpolation.
pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgba_premultiplied(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()), l(a.a(), b.a()))
}

/// Vertical gradient across a rect: stops are (fraction, color).
pub fn vgrad(p: &Painter, r: Rect, stops: &[(f32, Color32)]) {
    let mut m = Mesh::default();
    for (i, (t, c)) in stops.iter().enumerate() {
        let y = r.top() + r.height() * t;
        m.colored_vertex(pos2(r.left(), y), *c);
        m.colored_vertex(pos2(r.right(), y), *c);
        if i > 0 {
            let b = i as u32 * 2;
            m.add_triangle(b - 2, b - 1, b);
            m.add_triangle(b - 1, b + 1, b);
        }
    }
    p.add(m);
}

/// Animated toggle switch with a label.
pub fn toggle(ui: &mut Ui, on: &mut bool, label: &str) -> Response {
    ui.horizontal(|ui| {
        let (r, mut resp) = ui.allocate_exact_size(vec2(38.0, 22.0), Sense::click());
        if resp.clicked() {
            *on = !*on;
            resp.mark_changed();
        }
        let t = ui.ctx().animate_bool_with_time(resp.id, *on, 0.14);
        let p = ui.painter();
        p.rect_filled(r, 11.0, mix(theme::border(), theme::accent(), t));
        let x = egui::lerp((r.left() + 11.0)..=(r.right() - 11.0), t);
        p.circle_filled(pos2(x, r.center().y), 8.0, Color32::from_gray(240));
        ui.add_space(4.0);
        ui.label(egui::RichText::new(label).color(theme::text()));
        resp
    })
    .inner
}

/// Round icon button for the floating bar. `draw` paints the icon.
pub fn icon_button(ui: &mut Ui, size: Vec2, active: bool, draw: impl FnOnce(&Painter, Rect, Color32)) -> Response {
    let (r, resp) = ui.allocate_exact_size(size, Sense::click());
    let t = ui.ctx().animate_bool_with_time(resp.id, resp.hovered() || active, 0.12);
    let p = ui.painter();
    if t > 0.0 {
        p.rect_filled(r, size.y / 2.0, theme::tab_active().gamma_multiply(t));
    }
    let col = if active { theme::accent() } else { mix(theme::muted(), theme::text(), t) };
    let icon = Rect::from_center_size(r.center(), Vec2::splat(size.y * 0.52));
    draw(p, icon, col);
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

/// Tray with an arrow rising out of it.
pub fn export(p: &Painter, r: Rect, c: Color32) {
    p.line_segment([at(r, 0.5, 0.62), at(r, 0.5, 0.2)], stroke(c));
    p.line_segment([at(r, 0.35, 0.34), at(r, 0.5, 0.2)], stroke(c));
    p.line_segment([at(r, 0.65, 0.34), at(r, 0.5, 0.2)], stroke(c));
    p.add(Shape::line(vec![at(r, 0.24, 0.55), at(r, 0.24, 0.8), at(r, 0.76, 0.8), at(r, 0.76, 0.55)], stroke(c)));
}

/// Luggage-style tag with a hole.
pub fn tag(p: &Painter, r: Rect, c: Color32) {
    p.add(Shape::closed_line(
        vec![at(r, 0.14, 0.2), at(r, 0.52, 0.2), at(r, 0.88, 0.56), at(r, 0.56, 0.88), at(r, 0.14, 0.52)],
        stroke(c),
    ));
    p.circle_stroke(at(r, 0.32, 0.38), r.width() * 0.07, Stroke::new(1.4f32, c));
}

pub fn grid(p: &Painter, r: Rect, c: Color32) {
    for (x, y) in [(0.14, 0.14), (0.56, 0.14), (0.14, 0.56), (0.56, 0.56)] {
        p.rect_stroke(Rect::from_min_size(at(r, x, y), r.size() * 0.3), 1.5, Stroke::new(1.4f32, c));
    }
}

pub fn list(p: &Painter, r: Rect, c: Color32) {
    for y in [0.24, 0.5, 0.76] {
        p.circle_filled(at(r, 0.16, y), 1.6, c);
        p.line_segment([at(r, 0.32, y), at(r, 0.88, y)], stroke(c));
    }
}

/// Fanned pile of sheets.
pub fn stack(p: &Painter, r: Rect, c: Color32) {
    p.rect_stroke(Rect::from_min_max(at(r, 0.2, 0.36), at(r, 0.86, 0.88)), 1.5, Stroke::new(1.4f32, c));
    p.add(Shape::line(vec![at(r, 0.14, 0.76), at(r, 0.14, 0.26), at(r, 0.76, 0.26)], Stroke::new(1.4f32, c)));
    p.add(Shape::line(vec![at(r, 0.08, 0.64), at(r, 0.08, 0.14), at(r, 0.66, 0.14)], Stroke::new(1.2f32, c.gamma_multiply(0.6))));
}

pub fn pencil(p: &Painter, r: Rect, c: Color32) {
    p.line_segment([at(r, 0.22, 0.78), at(r, 0.74, 0.26)], Stroke::new(2.4f32, c));
    p.line_segment([at(r, 0.16, 0.84), at(r, 0.22, 0.78)], Stroke::new(1.2f32, c));
}

/// Arrow pointing down (or up when `up`).
pub fn arrow(p: &Painter, r: Rect, c: Color32, up: bool) {
    let (a, b) = if up { (0.8, 0.2) } else { (0.2, 0.8) };
    p.line_segment([at(r, 0.5, a), at(r, 0.5, b)], stroke(c));
    let tip = if up { 0.2 } else { 0.8 };
    let back = if up { 0.44 } else { 0.56 };
    p.line_segment([at(r, 0.28, back), at(r, 0.5, tip)], stroke(c));
    p.line_segment([at(r, 0.72, back), at(r, 0.5, tip)], stroke(c));
}
