//! Custom window chrome: tab bar merged with the title bar, window buttons,
//! and edge resizing for the borderless window.

use crate::icons;
use crate::theme;
use eframe::egui::{
    self, pos2, vec2, viewport::ResizeDirection, Align2, Color32, CursorIcon, FontFamily, FontId, Frame, Id, Rect, Sense,
    ViewportCommand,
};

pub const HEIGHT: f32 = 42.0;
const EDGE: f32 = 6.0;

pub enum Action {
    Home,
    Select(usize),
    Close(usize),
    New,
}

/// Handles resizing from the window edges. Returns true while the pointer is
/// over a resize edge so the title bar does not start a drag.
pub fn handle_resize(ctx: &egui::Context) -> bool {
    if ctx.input(|i| i.viewport().maximized.unwrap_or(false)) {
        return false;
    }
    let Some(p) = ctx.input(|i| i.pointer.hover_pos()) else { return false };
    let r = ctx.screen_rect();
    let (l, rt, t, b) = (p.x < r.left() + EDGE, p.x > r.right() - EDGE, p.y < r.top() + EDGE, p.y > r.bottom() - EDGE);
    let dir = match (l, rt, t, b) {
        (true, _, true, _) => ResizeDirection::NorthWest,
        (_, true, true, _) => ResizeDirection::NorthEast,
        (true, _, _, true) => ResizeDirection::SouthWest,
        (_, true, _, true) => ResizeDirection::SouthEast,
        (true, ..) => ResizeDirection::West,
        (_, true, ..) => ResizeDirection::East,
        (_, _, true, _) => ResizeDirection::North,
        (_, _, _, true) => ResizeDirection::South,
        _ => return false,
    };
    ctx.set_cursor_icon(match dir {
        ResizeDirection::North | ResizeDirection::South => CursorIcon::ResizeVertical,
        ResizeDirection::East | ResizeDirection::West => CursorIcon::ResizeHorizontal,
        ResizeDirection::NorthWest | ResizeDirection::SouthEast => CursorIcon::ResizeNwSe,
        _ => CursorIcon::ResizeNeSw,
    });
    if ctx.input(|i| i.pointer.primary_pressed()) {
        ctx.send_viewport_cmd(ViewportCommand::BeginResize(dir));
    }
    true
}

pub fn title_bar(ctx: &egui::Context, titles: &[String], active: Option<usize>, on_edge: bool) -> Vec<Action> {
    let mut actions = Vec::new();
    let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
    egui::TopBottomPanel::top("titlebar")
        .exact_height(HEIGHT)
        .frame(Frame::none().fill(theme::bar()))
        .show_separator_line(false)
        .show(ctx, |ui| {
            // Empty space drags the window; double-click maximizes.
            let drag = ui.interact(ui.max_rect(), Id::new("titlebar_drag"), Sense::click_and_drag());
            if drag.drag_started() && !on_edge {
                ctx.send_viewport_cmd(ViewportCommand::StartDrag);
            }
            if drag.double_clicked() {
                ctx.send_viewport_cmd(ViewportCommand::Maximized(!maximized));
            }

            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                ui.add_space(14.0);
                ui.label(
                    egui::RichText::new("Rustype").font(FontId::new(16.0, FontFamily::Name("SpecialElite".into()))).color(theme::muted()),
                );
                ui.add_space(10.0);

                if home_chip(ui, active.is_none()).clicked() {
                    actions.push(Action::Home);
                }
                for (i, t) in titles.iter().enumerate() {
                    match tab(ui, i, t, active == Some(i)) {
                        Some(true) => actions.push(Action::Close(i)),
                        Some(false) => actions.push(Action::Select(i)),
                        None => {}
                    }
                }
                let plus = icons::icon_button(ui, vec2(28.0, 28.0), false, icons::plus).on_hover_text("New sheet (Ctrl+T)");
                if plus.clicked() {
                    actions.push(Action::New);
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    if window_button(ui, true, |p, r, c| icons::close(p, r, c)).clicked() {
                        ctx.send_viewport_cmd(ViewportCommand::Close);
                    }
                    if window_button(ui, false, |p, r, c| icons::maximize(p, r, c, maximized)).clicked() {
                        ctx.send_viewport_cmd(ViewportCommand::Maximized(!maximized));
                    }
                    if window_button(ui, false, icons::minimize).clicked() {
                        ctx.send_viewport_cmd(ViewportCommand::Minimized(true));
                    }
                });
            });
        });
    actions
}

fn window_button(ui: &mut egui::Ui, danger: bool, draw: impl FnOnce(&egui::Painter, Rect, Color32)) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(vec2(46.0, HEIGHT), Sense::click());
    let t = ui.ctx().animate_bool_with_time(resp.id, resp.hovered(), 0.1);
    let hover_bg = if danger { Color32::from_rgb(196, 43, 28) } else { theme::tab_active() };
    if t > 0.0 {
        ui.painter().rect_filled(r, 0.0, hover_bg.gamma_multiply(t));
    }
    let col = if danger && t > 0.5 { Color32::WHITE } else { theme::text() };
    draw(ui.painter(), Rect::from_center_size(r.center(), vec2(11.0, 11.0)), col);
    resp
}

fn home_chip(ui: &mut egui::Ui, active: bool) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(vec2(36.0, 30.0), Sense::click());
    let t = ui.ctx().animate_bool_with_time(resp.id, resp.hovered(), 0.1);
    let fill = if active { theme::tab_active() } else { theme::tab_active().gamma_multiply(t * 0.6) };
    ui.painter().rect_filled(r, 8.0, fill);
    let col = if active { theme::text() } else { theme::muted() };
    icons::home(ui.painter(), Rect::from_center_size(r.center(), vec2(15.0, 15.0)), col);
    resp.on_hover_text("Home")
}

/// A tab chip. Returns Some(true) when closed, Some(false) when selected.
fn tab(ui: &mut egui::Ui, i: usize, title: &str, active: bool) -> Option<bool> {
    let font = FontId::proportional(13.5);
    let text_w = ui.fonts(|f| f.layout_no_wrap(title.to_string(), font.clone(), Color32::WHITE).size().x);
    let size = vec2(text_w + 14.0 + 26.0, 30.0);
    let (r, resp) = ui.allocate_exact_size(size, Sense::click());
    let close_rect = Rect::from_center_size(pos2(r.right() - 15.0, r.center().y), vec2(18.0, 18.0));
    let close = ui.interact(close_rect, Id::new(("tab_close", i)), Sense::click());

    let hover = ui.ctx().animate_bool_with_time(resp.id, resp.hovered() || close.hovered(), 0.1);
    let p = ui.painter();
    let fill = if active { theme::tab_active() } else { theme::tab_active().gamma_multiply(hover * 0.6) };
    p.rect_filled(r, 8.0, fill);
    if active {
        p.line_segment(
            [pos2(r.left() + 10.0, r.bottom() - 1.0), pos2(r.right() - 10.0, r.bottom() - 1.0)],
            egui::Stroke::new(2.0f32, theme::accent().gamma_multiply(0.8)),
        );
    }
    let col = if active { theme::text() } else { icons::mix(theme::muted(), theme::text(), hover) };
    p.text(pos2(r.left() + 12.0, r.center().y), Align2::LEFT_CENTER, title, font, col);

    if active || hover > 0.01 {
        if close.hovered() {
            p.rect_filled(close_rect, 5.0, theme::border());
        }
        let a = if active { 1.0 } else { hover };
        icons::close(p, close_rect.shrink(4.0), theme::muted().gamma_multiply(a));
    }

    if close.clicked() || resp.middle_clicked() {
        Some(true)
    } else if resp.clicked() {
        Some(false)
    } else {
        None
    }
}
