//! Home screen: wordmark, search, colour-tag chips, and sheets shown as a
//! grid of index cards, a list, or piles per tag.

use crate::icons;
use crate::notes::{ago, Note};
use crate::settings::{HomeView, Settings, SortBy};
use crate::tags::{self, Tags};
use crate::theme;
use eframe::egui::{
    self, epaint::Shadow, pos2, text::LayoutJob, text::TextWrapping, vec2, Align2, Color32, FontFamily, FontId, Frame, Id,
    Margin, Order, Pos2, Rect, RichText, Sense, Stroke, TextEdit, TextFormat, Vec2,
};
use std::cmp::Ordering;
use std::path::PathBuf;

pub enum Action {
    New,
    Open(PathBuf),
    Delete(PathBuf),
}

/// How cards are drawn: the user's paper, ink and typeface.
pub struct Look {
    pub paper: Color32,
    pub ink: Color32,
    pub family: FontFamily,
}

#[derive(Clone, Copy, PartialEq, Default)]
pub enum Filter {
    #[default]
    All,
    Untagged,
    Tag(u32),
}

impl Filter {
    fn matches(self, tag: Option<u32>) -> bool {
        match self {
            Filter::All => true,
            Filter::Untagged => tag.is_none(),
            Filter::Tag(id) => tag == Some(id),
        }
    }
}

#[derive(Default)]
pub struct State {
    pub filter: Filter,
    /// Sheet whose tag picker is open, where, and the frame it opened on.
    picker: Option<(PathBuf, Pos2, u64)>,
    edit: Option<(tags::Edit, Pos2, u64)>,
}

const CARD: Vec2 = vec2(250.0, 176.0);
const RULE: f32 = 19.0;
const ROW_H: f32 = 58.0;

/// Sheets in display order.
fn sorted<'a>(notes: Vec<&'a Note>, tags: &Tags, s: &Settings, sizes: &dyn Fn(Option<u32>) -> usize) -> Vec<&'a Note> {
    let mut v = notes;
    let title = |n: &Note| if n.title.is_empty() { "untitled".to_string() } else { n.title.to_lowercase() };
    v.sort_by(|a, b| {
        let o = match s.sort_by {
            SortBy::Edited => a.modified.cmp(&b.modified),
            SortBy::Created => a.created.cmp(&b.created),
            SortBy::Name => title(b).cmp(&title(a)), // reversed so "desc" means A-Z first below
            SortBy::TagSize => {
                let (ta, tb) = (tags.of(&a.path).map(|t| t.id), tags.of(&b.path).map(|t| t.id));
                sizes(ta).cmp(&sizes(tb)).then(a.modified.cmp(&b.modified))
            }
        };
        if s.sort_desc {
            o.reverse()
        } else {
            o
        }
    });
    v
}

pub fn show(
    ui: &mut egui::Ui,
    notes: &[Note],
    search: &mut String,
    look: &Look,
    tags: &mut Tags,
    s: &mut Settings,
    st: &mut State,
) -> Option<Action> {
    let mut action = None;
    let width = (ui.available_width() - 48.0).min(1120.0);
    let side = ((ui.available_width() - width) / 2.0).max(0.0);
    let frame_nr = ui.ctx().cumulative_pass_nr();

    // Drop a filter on a tag that no longer exists.
    if let Filter::Tag(id) = st.filter {
        if tags.get(id).is_none() {
            st.filter = Filter::All;
        }
    }

    let tag_of = |n: &Note| tags.of(&n.path).map(|t| t.id);
    let count = |f: Filter| notes.iter().filter(|n| f.matches(tag_of(n))).count();

    egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
        ui.add_space(40.0);
        ui.horizontal(|ui| {
            ui.add_space(side);
            ui.vertical(|ui| {
                ui.set_width(width);

                // Header: wordmark + new sheet.
                ui.horizontal(|ui| {
                    wordmark(ui);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if new_button(ui).clicked() {
                            action = Some(Action::New);
                        }
                    });
                });
                ui.add_space(26.0);

                // Search.
                Frame::none()
                    .fill(theme::card())
                    .rounding(12.0)
                    .stroke(Stroke::new(1.0f32, theme::border()))
                    .inner_margin(Margin::symmetric(14.0, 11.0))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let (r, _) = ui.allocate_exact_size(Vec2::splat(18.0), Sense::hover());
                            icons::search(ui.painter(), r, theme::muted());
                            ui.add_space(4.0);
                            ui.add(
                                TextEdit::singleline(search)
                                    .hint_text("Search every sheet\u{2026}")
                                    .frame(false)
                                    .font(FontId::proportional(15.0))
                                    .desired_width(f32::INFINITY),
                            );
                        });
                    });
                ui.add_space(16.0);

                // Tag chips: filter, edit, add.
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
                    if chip(ui, None, "All", count(Filter::All), st.filter == Filter::All, false).0.clicked() {
                        st.filter = Filter::All;
                    }
                    for t in tags.tags.clone() {
                        let f = Filter::Tag(t.id);
                        let (resp, edit) = chip(ui, Some(t.color32()), &t.name, count(f), st.filter == f, true);
                        if let Some(e) = edit {
                            if e.clicked() {
                                let at = pos2(e.rect.left(), e.rect.bottom() + 6.0);
                                st.edit = Some((tags::Edit { id: Some(t.id), name: t.name.clone(), color: t.color, confirm_delete: false }, at, frame_nr));
                            }
                        }
                        if resp.clicked() {
                            st.filter = if st.filter == f { Filter::All } else { f };
                        }
                    }
                    let untagged = count(Filter::Untagged);
                    if !tags.tags.is_empty() && untagged > 0 {
                        let (resp, _) = chip(ui, Some(Color32::TRANSPARENT), "Untagged", untagged, st.filter == Filter::Untagged, false);
                        if resp.clicked() {
                            st.filter = if st.filter == Filter::Untagged { Filter::All } else { Filter::Untagged };
                        }
                    }
                    let add = add_chip(ui);
                    if add.clicked() {
                        let at = pos2(add.rect.left(), add.rect.bottom() + 6.0);
                        st.edit = Some((tags::Edit { id: None, name: String::new(), color: tags.next_color(), confirm_delete: false }, at, frame_nr));
                    }
                });
                ui.add_space(18.0);

                // Which sheets, in which order.
                let q = search.trim().to_lowercase();
                let searched: Vec<&Note> = notes.iter().filter(|n| q.is_empty() || n.body.to_lowercase().contains(&q)).collect();
                let sizes = |t: Option<u32>| notes.iter().filter(|n| tag_of(n) == t).count();
                let piles = s.home_view == HomeView::Stack && st.filter == Filter::All;
                let shown = sorted(searched.iter().copied().filter(|n| st.filter.matches(tag_of(n))).collect(), tags, s, &sizes);

                // Toolbar: heading on the left, view + sort on the right.
                ui.horizontal(|ui| {
                    let heading = if !q.is_empty() {
                        format!("{} MATCH{}", shown.len(), if shown.len() == 1 { "" } else { "ES" })
                    } else {
                        let n = shown.len();
                        format!("{n} SHEET{}", if n == 1 { "" } else { "S" })
                    };
                    if s.home_view == HomeView::Stack && st.filter != Filter::All {
                        let (br, back) = ui.allocate_exact_size(vec2(84.0, 24.0), Sense::click());
                        let col = if back.hovered() { theme::text() } else { theme::muted() };
                        let c = pos2(br.left() + 6.0, br.center().y);
                        let sk = Stroke::new(1.6f32, col);
                        ui.painter().line_segment([c + vec2(4.0, -5.0), c + vec2(-1.0, 0.0)], sk);
                        ui.painter().line_segment([c + vec2(-1.0, 0.0), c + vec2(4.0, 5.0)], sk);
                        ui.painter().text(pos2(br.left() + 18.0, br.center().y), Align2::LEFT_CENTER, "All stacks", FontId::proportional(12.5), col);
                        if back.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                        if back.clicked() {
                            st.filter = Filter::All;
                        }
                        ui.label(RichText::new("\u{00b7}").color(theme::muted()));
                    }
                    ui.label(RichText::new(heading).size(12.0).strong().color(theme::muted()));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        let size = Vec2::splat(30.0);
                        let tip = match (s.sort_by, s.sort_desc) {
                            (SortBy::Name, true) => "A to Z",
                            (SortBy::Name, false) => "Z to A",
                            (SortBy::TagSize, true) => "Biggest first",
                            (SortBy::TagSize, false) => "Smallest first",
                            (_, true) => "Newest first",
                            (_, false) => "Oldest first",
                        };
                        let desc = s.sort_desc;
                        if icons::icon_button(ui, size, false, |p, r, c| icons::arrow(p, r, c, !desc)).on_hover_text(tip).clicked() {
                            s.sort_desc = !s.sort_desc;
                        }
                        egui::ComboBox::from_id_salt("sort_by")
                            .selected_text(RichText::new(format!("Sort: {}", s.sort_by.name())).size(13.0))
                            .width(150.0)
                            .show_ui(ui, |ui| {
                                for o in SortBy::ALL {
                                    ui.selectable_value(&mut s.sort_by, o, o.name());
                                }
                            });
                        ui.add_space(10.0);
                        for (v, name) in [(HomeView::Stack, "Stacks"), (HomeView::List, "List"), (HomeView::Grid, "Grid")] {
                            let r = icons::icon_button(ui, size, s.home_view == v, |p, r, c| match v {
                                HomeView::Grid => icons::grid(p, r, c),
                                HomeView::List => icons::list(p, r, c),
                                HomeView::Stack => icons::stack(p, r, c),
                            });
                            if r.on_hover_text(name).clicked() {
                                s.home_view = v;
                            }
                        }
                    });
                });
                ui.add_space(14.0);

                if notes.is_empty() || shown.is_empty() {
                    ui.add_space(40.0);
                    let msg = if notes.is_empty() {
                        "No sheets yet. Feed one in with  + New sheet."
                    } else if !q.is_empty() {
                        "Nothing typed matches that."
                    } else {
                        "No sheets with this tag yet."
                    };
                    ui.label(RichText::new(msg).font(FontId::new(17.0, FontFamily::Name("SpecialElite".into()))).color(theme::muted()));
                }

                let mut card_action = None;
                if piles {
                    // One pile per tag, plus untagged.
                    let mut groups: Vec<(Option<u32>, Vec<&Note>)> = Vec::new();
                    for n in &shown {
                        let t = tag_of(n);
                        match groups.iter_mut().find(|(g, _)| *g == t) {
                            Some((_, v)) => v.push(n),
                            None => groups.push((t, vec![n])),
                        }
                    }
                    let name_of = |t: Option<u32>| t.and_then(|id| tags.get(id)).map_or("untagged".to_string(), |t| t.name.to_lowercase());
                    groups.sort_by(|a, b| {
                        let o = match s.sort_by {
                            SortBy::TagSize => a.1.len().cmp(&b.1.len()),
                            SortBy::Name => name_of(b.0).cmp(&name_of(a.0)),
                            SortBy::Edited => a.1.iter().map(|n| n.modified).max().cmp(&b.1.iter().map(|n| n.modified).max()),
                            SortBy::Created => a.1.iter().map(|n| n.created).max().cmp(&b.1.iter().map(|n| n.created).max()),
                        };
                        let o = if o == Ordering::Equal { a.0.is_none().cmp(&b.0.is_none()).reverse() } else { o };
                        if s.sort_desc {
                            o.reverse()
                        } else {
                            o
                        }
                    });
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = vec2(28.0, 30.0);
                        for (t, members) in &groups {
                            let tag = t.and_then(|id| tags.get(id)).cloned();
                            if pile(ui, tag.as_ref(), members, look).clicked() {
                                st.filter = t.map_or(Filter::Untagged, Filter::Tag);
                            }
                        }
                    });
                } else if s.home_view == HomeView::List {
                    ui.spacing_mut().item_spacing.y = 6.0;
                    for n in &shown {
                        let tag = tags.of(&n.path).cloned();
                        if let Some(a) = row(ui, n, tag.as_ref(), look) {
                            card_action = Some(a);
                        }
                    }
                } else {
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = vec2(20.0, 22.0);
                        for n in &shown {
                            let tag = tags.of(&n.path).cloned();
                            if let Some(a) = card(ui, n, tag.as_ref(), look) {
                                card_action = Some(a);
                            }
                        }
                    });
                }
                match card_action {
                    Some(CardAction::Tag(p, at)) => st.picker = Some((p, at, frame_nr)),
                    Some(CardAction::Do(a)) => action = Some(a),
                    None => {}
                }
                ui.add_space(110.0); // room for the floating bar
            });
        });
    });

    popups(ui.ctx(), tags, st, notes, frame_nr);
    action
}

/// Draws the tag picker / tag editor and closes them on an outside click.
fn popups(ctx: &egui::Context, tags: &mut Tags, st: &mut State, notes: &[Note], frame_nr: u64) {
    let pressed_at = ctx.input(|i| if i.pointer.any_pressed() { i.pointer.interact_pos() } else { None })
        .filter(|_| !ctx.memory(|m| m.any_popup_open()));
    let frame = || {
        Frame::none()
            .fill(theme::bar())
            .stroke(Stroke::new(1.0f32, theme::border()))
            .rounding(12.0)
            .inner_margin(Margin::same(14.0))
            .shadow(Shadow { offset: vec2(0.0, 8.0), blur: 24.0, spread: 0.0, color: Color32::from_black_alpha(110) })
    };
    let screen = ctx.screen_rect();
    let place = |at: Pos2, w: f32| pos2(at.x.min(screen.right() - w - 16.0).max(8.0), at.y);

    if let Some((path, at, opened)) = st.picker.clone() {
        let resp = egui::Area::new(Id::new("tag_picker")).order(Order::Foreground).fixed_pos(place(at, 262.0)).show(ctx, |ui| {
            frame().show(ui, |ui| tags::picker(ui, tags, &path)).inner
        });
        let outside = pressed_at.is_some_and(|p| !resp.response.rect.contains(p)) && opened != frame_nr;
        if resp.inner || outside || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            st.picker = None;
        }
    }

    if let Some((mut e, at, opened)) = st.edit.take() {
        let used = e.id.map_or(0, |id| notes.iter().filter(|n| tags.of(&n.path).map(|t| t.id) == Some(id)).count());
        let resp = egui::Area::new(Id::new("tag_editor")).order(Order::Foreground).fixed_pos(place(at, 282.0)).show(ctx, |ui| {
            frame().show(ui, |ui| tags::editor(ui, tags, &mut e, used)).inner
        });
        let outside = pressed_at.is_some_and(|p| !resp.response.rect.contains(p)) && opened != frame_nr;
        match resp.inner {
            tags::EditResult::Open if !outside && !ctx.input(|i| i.key_pressed(egui::Key::Escape)) => st.edit = Some((e, at, opened)),
            tags::EditResult::Deleted(id) if st.filter == Filter::Tag(id) => st.filter = Filter::All,
            tags::EditResult::Created(id) => st.filter = Filter::Tag(id),
            _ => {}
        }
    }
}

/// Filter chip. Returns the chip response and, for tags, the edit button.
fn chip(ui: &mut egui::Ui, color: Option<Color32>, name: &str, n: usize, selected: bool, editable: bool) -> (egui::Response, Option<egui::Response>) {
    let font = FontId::proportional(13.0);
    let label = format!("{name}  {n}");
    let text_w = ui.fonts(|f| f.layout_no_wrap(label.clone(), font.clone(), Color32::WHITE).size().x);
    let dot_w = if color.is_some() { 16.0 } else { 0.0 };
    let edit_w = if editable { 22.0 } else { 0.0 };
    let (r, resp) = ui.allocate_exact_size(vec2(text_w + dot_w + edit_w + 26.0, 30.0), Sense::click());
    let edit_rect = Rect::from_center_size(pos2(r.right() - 16.0, r.center().y), Vec2::splat(20.0));
    let edit = editable.then(|| ui.interact(edit_rect, resp.id.with("edit"), Sense::click()));
    let hovered = resp.hovered() || edit.as_ref().is_some_and(|e| e.hovered());
    let t = ui.ctx().animate_bool_with_time(resp.id, hovered, 0.1);
    let p = ui.painter();
    let fill = if selected { theme::tab_active() } else { icons::mix(theme::card(), theme::tab_active(), t * 0.7) };
    p.rect_filled(r, 15.0, fill);
    p.rect_stroke(r, 15.0, Stroke::new(1.0f32, if selected { theme::accent() } else { theme::border() }));
    let mut x = r.left() + 13.0;
    if let Some(c) = color {
        if c == Color32::TRANSPARENT {
            p.circle_stroke(pos2(x + 5.0, r.center().y), 4.5, Stroke::new(1.3f32, theme::muted()));
        } else {
            p.circle_filled(pos2(x + 5.0, r.center().y), 5.0, c);
        }
        x += dot_w;
    }
    p.text(pos2(x, r.center().y), Align2::LEFT_CENTER, name, font.clone(), theme::text());
    let name_w = ui.fonts(|f| f.layout_no_wrap(format!("{name}  "), font.clone(), Color32::WHITE).size().x);
    p.text(pos2(x + name_w, r.center().y), Align2::LEFT_CENTER, n.to_string(), font, theme::muted());
    if let Some(e) = &edit {
        let col = if e.hovered() { theme::text() } else { theme::muted().gamma_multiply(0.4 + 0.6 * t) };
        if e.hovered() {
            p.circle_filled(edit_rect.center(), 10.0, theme::border());
        }
        icons::pencil(p, edit_rect.shrink(4.0), col);
    }
    if hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    let resp = match &edit {
        Some(e) => resp.on_hover_text(if e.hovered() { "Edit tag" } else { "Show only this tag" }),
        None => resp,
    };
    (resp, edit)
}

fn add_chip(ui: &mut egui::Ui) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(vec2(84.0, 30.0), Sense::click());
    let t = ui.ctx().animate_bool_with_time(resp.id, resp.hovered(), 0.1);
    let p = ui.painter();
    p.rect_stroke(r, 15.0, Stroke::new(1.0f32, icons::mix(theme::border(), theme::muted(), t)));
    icons::plus(p, Rect::from_center_size(pos2(r.left() + 18.0, r.center().y), Vec2::splat(11.0)), theme::muted());
    p.text(pos2(r.left() + 30.0, r.center().y), Align2::LEFT_CENTER, "New tag", FontId::proportional(13.0), icons::mix(theme::muted(), theme::text(), t));
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

enum CardAction {
    Do(Action),
    Tag(PathBuf, Pos2),
}

/// Hover buttons shared by cards and rows: tag and trash.
fn hover_buttons(ui: &mut egui::Ui, n: &Note, tag_slot: Rect, trash_slot: Rect) -> (egui::Response, egui::Response) {
    let tag = ui.interact(tag_slot, Id::new(("tagbtn", &n.path)), Sense::click()).on_hover_text("Tag");
    let trash = ui.interact(trash_slot, Id::new(("trash", &n.path)), Sense::click()).on_hover_text("Delete");
    (tag, trash)
}

fn paint_hover_buttons(p: &egui::Painter, tag: &egui::Response, trash: &egui::Response, tr: Rect, gr: Rect, ink: Color32, t: f32) {
    if t <= 0.01 {
        return;
    }
    let red = Color32::from_rgb(200, 60, 55);
    if trash.hovered() {
        p.rect_filled(tr, 6.0, Color32::from_rgba_unmultiplied(200, 60, 55, 30));
    }
    icons::trash(p, tr.shrink(5.0), if trash.hovered() { red } else { ink.gamma_multiply(0.45 * t) });
    if tag.hovered() {
        p.rect_filled(gr, 6.0, ink.gamma_multiply(0.1));
    }
    icons::tag(p, gr.shrink(5.0), if tag.hovered() { ink } else { ink.gamma_multiply(0.45 * t) });
}

fn card_result(tag: &egui::Response, trash: &egui::Response, resp: &egui::Response, n: &Note) -> Option<CardAction> {
    if trash.clicked() {
        Some(CardAction::Do(Action::Delete(n.path.clone())))
    } else if tag.clicked() {
        Some(CardAction::Tag(n.path.clone(), pos2(tag.rect.left() - 100.0, tag.rect.bottom() + 6.0)))
    } else if resp.clicked() {
        Some(CardAction::Do(Action::Open(n.path.clone())))
    } else {
        None
    }
}

fn card(ui: &mut egui::Ui, n: &Note, tag: Option<&tags::Tag>, look: &Look) -> Option<CardAction> {
    let (slot, resp) = ui.allocate_exact_size(CARD, Sense::click());
    let trash_slot = Rect::from_center_size(pos2(slot.right() - 22.0, slot.bottom() - 20.0), Vec2::splat(26.0));
    let tag_slot = trash_slot.translate(vec2(-30.0, 0.0));
    let (tagb, trash) = hover_buttons(ui, n, tag_slot, trash_slot);
    let hovered = resp.hovered() || trash.hovered() || tagb.hovered();
    let t = ui.ctx().animate_bool_with_time(resp.id, hovered, 0.14);

    let r = slot.translate(vec2(0.0, -4.0 * t));
    let p = ui.painter();
    p.add(
        Shadow { offset: vec2(0.0, 5.0 + 6.0 * t), blur: 16.0 + 12.0 * t, spread: 0.0, color: Color32::from_black_alpha((110.0 + 50.0 * t) as u8) }
            .as_shape(r, 4.0),
    );
    p.rect_filled(r, 4.0, look.paper);

    // Index-card rules.
    let head = r.top() + 44.0;
    p.line_segment([pos2(r.left(), head), pos2(r.right(), head)], Stroke::new(1.2f32, Color32::from_rgba_unmultiplied(214, 96, 96, 150)));
    let mut y = head + RULE;
    while y < r.bottom() - 30.0 {
        p.line_segment([pos2(r.left(), y), pos2(r.right(), y)], Stroke::new(1.0f32, Color32::from_rgba_unmultiplied(110, 150, 205, 55)));
        y += RULE;
    }

    // Tag: a coloured tab on the card's top edge.
    if let Some(tg) = tag {
        let tab = Rect::from_min_size(pos2(r.right() - 58.0, r.top()), vec2(40.0, 7.0));
        p.rect_filled(tab, egui::Rounding { nw: 0.0, ne: 0.0, sw: 3.0, se: 3.0 }, tg.color32());
    }

    let title = if n.title.is_empty() { "Untitled" } else { &n.title };
    let title_g = ui.fonts(|f| f.layout_job(elided(title, FontId::new(17.0, look.family.clone()), look.ink, r.width() - 32.0, 1, None)));
    p.galley(pos2(r.left() + 16.0, head - 8.0 - title_g.size().y), title_g, look.ink);

    let snippet: String = n.body.lines().skip_while(|l| l.trim().is_empty()).skip(1).collect::<Vec<_>>().join(" ");
    let ink2 = look.ink.gamma_multiply(0.72);
    let body_g = ui.fonts(|f| f.layout_job(elided(&snippet, FontId::new(13.0, look.family.clone()), ink2, r.width() - 32.0, 4, Some(RULE))));
    p.galley(pos2(r.left() + 16.0, head + 3.0), body_g, ink2);

    let foot = pos2(r.left() + 16.0, r.bottom() - 18.0);
    let mut x = foot.x;
    if let Some(tg) = tag {
        p.circle_filled(pos2(x + 4.0, foot.y), 4.0, tg.color32());
        let g = p.layout_no_wrap(tg.name.clone(), FontId::proportional(11.5), look.ink.gamma_multiply(0.7));
        let w = g.size().x;
        p.galley(pos2(x + 13.0, foot.y - g.size().y / 2.0), g, look.ink);
        x += 13.0 + w + 10.0;
    }
    p.text(pos2(x, foot.y), Align2::LEFT_CENTER, ago(n.modified), FontId::proportional(11.5), look.ink.gamma_multiply(0.5));

    let lift = vec2(0.0, -4.0 * t);
    paint_hover_buttons(p, &tagb, &trash, trash_slot.translate(lift), tag_slot.translate(lift), look.ink, t);
    if hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    card_result(&tagb, &trash, &resp, n)
}

fn row(ui: &mut egui::Ui, n: &Note, tag: Option<&tags::Tag>, look: &Look) -> Option<CardAction> {
    let w = ui.available_width();
    let (r, resp) = ui.allocate_exact_size(vec2(w, ROW_H), Sense::click());
    let trash_slot = Rect::from_center_size(pos2(r.right() - 24.0, r.center().y), Vec2::splat(28.0));
    let tag_slot = trash_slot.translate(vec2(-34.0, 0.0));
    let (tagb, trash) = hover_buttons(ui, n, tag_slot, trash_slot);
    let hovered = resp.hovered() || trash.hovered() || tagb.hovered();
    let t = ui.ctx().animate_bool_with_time(resp.id, hovered, 0.12);
    let p = ui.painter();
    p.rect_filled(r, 10.0, icons::mix(theme::card(), theme::tab_active(), t));
    let strip = Rect::from_min_size(r.min + vec2(0.0, 10.0), vec2(4.0, ROW_H - 20.0));
    p.rect_filled(strip, 2.0, tag.map_or(theme::border(), |tg| tg.color32()));

    // Small paper swatch with the sheet's typeface.
    let sw = Rect::from_center_size(pos2(r.left() + 36.0, r.center().y), vec2(30.0, 38.0));
    p.rect_filled(sw, 3.0, look.paper);
    p.text(sw.center(), Align2::CENTER_CENTER, "Aa", FontId::new(13.0, look.family.clone()), look.ink);

    let tx = r.left() + 64.0;
    let right_room = 300.0;
    let title = if n.title.is_empty() { "Untitled" } else { &n.title };
    let tg = ui.fonts(|f| f.layout_job(elided(title, FontId::new(16.0, look.family.clone()), theme::text(), w - 64.0 - right_room, 1, None)));
    p.galley(pos2(tx, r.top() + 10.0), tg, theme::text());
    let snippet: String = n.body.lines().skip_while(|l| l.trim().is_empty()).skip(1).collect::<Vec<_>>().join(" ");
    let sg = ui.fonts(|f| f.layout_job(elided(&snippet, FontId::proportional(12.5), theme::muted(), w - 64.0 - right_room, 1, None)));
    p.galley(pos2(tx, r.top() + 33.0), sg, theme::muted());

    // Right side: tag name and age.
    let meta_x = r.right() - 84.0;
    p.text(pos2(meta_x, r.center().y), Align2::RIGHT_CENTER, ago(n.modified), FontId::proportional(12.0), theme::muted());
    if let Some(tg) = tag {
        let g = p.layout_no_wrap(tg.name.clone(), FontId::proportional(12.0), theme::text());
        let pill = Rect::from_min_size(pos2(meta_x - 110.0 - g.size().x, r.center().y - 11.0), vec2(g.size().x + 26.0, 22.0));
        p.rect_filled(pill, 11.0, tg.color32().gamma_multiply(0.22));
        p.circle_filled(pos2(pill.left() + 11.0, pill.center().y), 4.0, tg.color32());
        p.galley(pos2(pill.left() + 19.0, pill.center().y - g.size().y / 2.0), g, theme::text());
    }

    paint_hover_buttons(p, &tagb, &trash, trash_slot, tag_slot, theme::text(), t.max(0.35));
    if hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    card_result(&tagb, &trash, &resp, n)
}

/// A fanned pile of sheets for one tag.
fn pile(ui: &mut egui::Ui, tag: Option<&tags::Tag>, members: &[&Note], look: &Look) -> egui::Response {
    let (slot, resp) = ui.allocate_exact_size(vec2(250.0, 200.0), Sense::click());
    let t = ui.ctx().animate_bool_with_time(resp.id, resp.hovered(), 0.16);
    let p = ui.painter();
    let color = tag.map_or(theme::muted(), |tg| tg.color32());
    let base = Rect::from_min_size(slot.min + vec2(10.0, 22.0), vec2(230.0, 160.0));
    let layers = members.len().clamp(1, 4);
    // Back sheets fan out further on hover.
    for k in (1..layers).rev() {
        let k = k as f32;
        let off = vec2(-6.0 * k - 6.0 * k * t, -7.0 * k - 3.0 * k * t);
        let r = base.translate(off + vec2(4.0 * k, 0.0));
        p.add(Shadow { offset: vec2(0.0, 3.0), blur: 10.0, spread: 0.0, color: Color32::from_black_alpha(70) }.as_shape(r, 4.0));
        p.rect_filled(r, 4.0, icons::mix(look.paper, Color32::BLACK, 0.06 * k));
        p.rect_filled(Rect::from_min_size(r.min, vec2(r.width(), 5.0)), egui::Rounding { nw: 4.0, ne: 4.0, sw: 0.0, se: 0.0 }, color.gamma_multiply(0.7));
    }
    let front = base.translate(vec2(0.0, -3.0 * t));
    p.add(Shadow { offset: vec2(0.0, 6.0 + 5.0 * t), blur: 18.0 + 8.0 * t, spread: 0.0, color: Color32::from_black_alpha(120) }.as_shape(front, 4.0));
    p.rect_filled(front, 4.0, look.paper);
    p.rect_filled(Rect::from_min_size(front.min, vec2(front.width(), 7.0)), egui::Rounding { nw: 4.0, ne: 4.0, sw: 0.0, se: 0.0 }, color);

    let name = tag.map_or("Untagged", |tg| tg.name.as_str());
    let ng = ui.fonts(|f| f.layout_job(elided(name, FontId::new(20.0, look.family.clone()), look.ink, front.width() - 32.0, 1, None)));
    p.galley(pos2(front.left() + 16.0, front.top() + 22.0), ng, look.ink);
    let n = members.len();
    p.text(
        pos2(front.left() + 16.0, front.top() + 58.0),
        Align2::LEFT_CENTER,
        format!("{n} sheet{}", if n == 1 { "" } else { "s" }),
        FontId::proportional(12.5),
        look.ink.gamma_multiply(0.6),
    );
    // Titles of the first few sheets.
    let ink2 = look.ink.gamma_multiply(0.7);
    for (i, m) in members.iter().take(3).enumerate() {
        let title = if m.title.is_empty() { "Untitled" } else { &m.title };
        let g = ui.fonts(|f| f.layout_job(elided(title, FontId::new(13.0, look.family.clone()), ink2, front.width() - 32.0, 1, None)));
        p.galley(pos2(front.left() + 16.0, front.top() + 82.0 + i as f32 * 20.0), g, ink2);
    }
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp.on_hover_text("Open stack")
}

fn wordmark(ui: &mut egui::Ui) {
    let font = FontId::new(46.0, FontFamily::Name("SpecialElite".into()));
    let text = "Rustype";
    let widths: Vec<f32> = ui.fonts(|f| text.chars().map(|c| f.glyph_width(&font, c)).collect());
    let total: f32 = widths.iter().sum();
    let (r, _) = ui.allocate_exact_size(vec2(total + 6.0, 64.0), Sense::hover());
    let p = ui.painter();
    let mut x = r.left();
    for (i, ch) in text.chars().enumerate() {
        let j = ((i * 7919) % 13) as f32 / 13.0 - 0.5;
        let pos = pos2(x, r.top() + 46.0 + j * 2.5);
        x += widths[i];
        let a = 200 + ((i * 31) % 55) as u8;
        p.text(pos, Align2::LEFT_BOTTOM, ch, font.clone(), Color32::from_rgba_unmultiplied(236, 232, 222, a).gamma_multiply(if theme::text().r() < 200 { 0.7 } else { 1.0 }));
    }
    p.text(
        pos2(r.left() + 2.0, r.top() + 60.0),
        Align2::LEFT_BOTTOM,
        "a typewriter for your thoughts",
        FontId::proportional(13.0),
        theme::muted(),
    );
}

fn new_button(ui: &mut egui::Ui) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(vec2(146.0, 42.0), Sense::click());
    let t = ui.ctx().animate_bool_with_time(resp.id, resp.hovered(), 0.12);
    let fill = icons::mix(theme::accent(), Color32::from_rgb(236, 168, 100), t * 0.6);
    let p = ui.painter();
    p.add(Shadow { offset: vec2(0.0, 4.0), blur: 14.0, spread: 0.0, color: theme::accent().gamma_multiply(0.25 + 0.2 * t) }.as_shape(r, 21.0));
    p.rect_filled(r, 21.0, fill);
    icons::plus(p, Rect::from_center_size(pos2(r.left() + 26.0, r.center().y), Vec2::splat(14.0)), Color32::WHITE);
    p.text(pos2(r.left() + 40.0, r.center().y), Align2::LEFT_CENTER, "New sheet", FontId::proportional(15.0), Color32::WHITE);
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

fn elided(text: &str, font: FontId, color: Color32, width: f32, rows: usize, line_height: Option<f32>) -> LayoutJob {
    let mut job = LayoutJob::single_section(text.to_string(), TextFormat { font_id: font, color, line_height, ..Default::default() });
    job.wrap = TextWrapping { max_width: width, max_rows: rows, break_anywhere: false, overflow_character: Some('\u{2026}') };
    job
}
