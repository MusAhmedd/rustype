//! Rustype: a note-taking app with a typewriter interface.
#![windows_subsystem = "windows"]

mod bar;
mod chrome;
mod editor;
mod export;
mod goal;
mod home;
mod icons;
mod notes;
mod paper;
mod settings;
mod sound;
mod tags;
mod theme;
mod update;

use eframe::egui::{self, vec2, Align2, Color32, Frame, Id, Margin, Order, RichText, Sense};
use settings::Settings;
use std::path::PathBuf;
use std::time::{Duration, Instant};

const AUTOSAVE_DELAY: Duration = Duration::from_millis(800);

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1240.0, 820.0])
            .with_min_inner_size([760.0, 560.0])
            .with_decorations(false)
            .with_resizable(true)
            .with_title("Rustype"),
        ..Default::default()
    };
    eframe::run_native(
        "Rustype",
        options,
        Box::new(|cc| {
            theme::install_fonts(&cc.egui_ctx);
            theme::apply(&cc.egui_ctx, false);
            Ok(Box::new(App::new()))
        }),
    )
}

struct Tab {
    path: PathBuf,
    text: String,
    dirty_since: Option<Instant>,
    ed: editor::State,
    /// Word count after the last edit, to measure what each edit adds.
    words: usize,
}

impl Tab {
    fn open(path: PathBuf) -> Self {
        let text = notes::load(&path);
        Self { ed: editor::State::new(&text), words: goal::words(&text), path, text, dirty_since: None }
    }

    fn save_now(&mut self) {
        if self.dirty_since.take().is_some() {
            notes::save(&self.path, &self.text);
        }
    }

    fn title(&self) -> String {
        let t = notes::title_of(&self.text);
        if t.is_empty() {
            "Untitled".into()
        } else if t.chars().count() > 20 {
            format!("{}\u{2026}", t.chars().take(19).collect::<String>())
        } else {
            t
        }
    }
}

struct App {
    store: notes::Store,
    settings: Settings,
    saved_settings: Settings,
    tabs: Vec<Tab>,
    /// `None` shows the home screen.
    active: Option<usize>,
    notes: Vec<notes::Note>,
    notes_stale: bool,
    search: String,
    tags: tags::Tags,
    home: home::State,
    confirm_delete: Option<PathBuf>,
    sound: Option<sound::Sound>,
    /// Time (egui seconds) and character of the last key strike.
    last_strike: Option<(f64, char)>,
    now: f64,
    bar: bar::State,
    low_light_applied: bool,
    /// Insert key toggles typing over existing letters.
    overtype: bool,
    /// Last cursor activity, keeps the caret solid while working.
    caret_epoch: f64,
    /// A mouse selection is in progress on the sheet.
    selecting: bool,
    updater: update::Updater,
    /// The launch-time update check has been considered.
    launch_checked: bool,
    restarting: bool,
    session: goal::Session,
    /// Net words typed today; copied into settings when sheets save.
    today_words: i64,
}

impl App {
    fn new() -> Self {
        update::cleanup_old();
        let store = notes::Store::open_default();
        let tags = tags::Tags::load(store.tags_path());
        let settings = Settings::load(&store.settings_path());
        let tabs: Vec<Tab> = settings.open_tabs.iter().filter(|p| p.exists()).cloned().map(Tab::open).collect();
        let active = settings.active_tab.filter(|&i| i < tabs.len());
        Self {
            sound: sound::Sound::new(settings.pack),
            saved_settings: settings.clone(),
            settings,
            store,
            tabs,
            active,
            notes: Vec::new(),
            notes_stale: true,
            search: String::new(),
            tags,
            home: home::State::default(),
            confirm_delete: None,
            last_strike: None,
            now: 0.0,
            bar: bar::State::default(),
            low_light_applied: false,
            overtype: false,
            caret_epoch: 0.0,
            selecting: false,
            updater: update::Updater::default(),
            launch_checked: false,
            restarting: false,
            session: goal::Session::default(),
            today_words: 0,
        }
        .with_today()
    }

    fn with_today(mut self) -> Self {
        self.today_words = self.settings.words_today;
        self.roll_day();
        self
    }

    /// Starts a fresh count when the local date changes.
    fn roll_day(&mut self) {
        let d = goal::today();
        if d != self.settings.goal_day {
            self.settings.goal_day = d;
            self.settings.words_today = 0;
            self.today_words = 0;
        }
    }

    fn sync_words(&mut self) {
        self.settings.words_today = self.today_words;
    }

    /// Adds typed words to today's count and rings once the goal is met.
    fn count_typing(&mut self, delta: i64) {
        self.roll_day();
        self.session.keystroke(self.now);
        self.session.words += delta;
        self.today_words = (self.today_words + delta).max(0);
        let s = &self.settings;
        if s.goal_on && self.today_words >= s.goal_words as i64 && s.goal_hit_day != s.goal_day {
            self.settings.goal_hit_day = self.settings.goal_day;
            self.session.reached_at = Some(self.now);
            self.play(sound::Effect::Goal);
        }
    }

    fn play(&self, e: sound::Effect) {
        if self.settings.sound_on {
            if let Some(s) = &self.sound {
                s.play(e, self.settings.volume);
            }
        }
    }

    fn new_tab(&mut self) {
        self.tabs.push(Tab { path: self.store.new_path(), text: String::new(), dirty_since: None, ed: editor::State::new(""), words: 0 });
        self.active = Some(self.tabs.len() - 1);
    }

    fn open_path(&mut self, path: PathBuf) {
        if let Some(i) = self.tabs.iter().position(|t| t.path == path) {
            self.active = Some(i);
        } else {
            self.tabs.push(Tab::open(path));
            self.active = Some(self.tabs.len() - 1);
        }
    }

    fn close_tab(&mut self, i: usize) {
        let mut tab = self.tabs.remove(i);
        tab.save_now();
        if tab.text.trim().is_empty() {
            notes::delete(&tab.path); // don't keep blank sheets around
        }
        self.notes_stale = true;
        self.active = match self.active {
            Some(a) if a == i => {
                if self.tabs.is_empty() {
                    None
                } else {
                    Some(i.min(self.tabs.len() - 1))
                }
            }
            Some(a) if a > i => Some(a - 1),
            other => other,
        };
    }

    fn handle_typing(&mut self, ctx: &egui::Context) {
        let Some(i) = self.active else { return };
        if ctx.wants_keyboard_input() || self.confirm_delete.is_some() {
            return;
        }
        for ev in ctx.input(|inp| inp.events.clone()) {
            match ev {
                egui::Event::Text(t) => {
                    for ch in t.chars() {
                        self.type_char(i, ch);
                    }
                }
                egui::Event::Paste(p) => self.paste(i, &p),
                egui::Event::Copy => {
                    let tab = &self.tabs[i];
                    if let Some(s) = tab.ed.selected_text(&tab.text) {
                        ctx.copy_text(s);
                    }
                }
                egui::Event::Cut => {
                    let tab = &mut self.tabs[i];
                    if let Some(s) = tab.ed.cut(&mut tab.text, self.now) {
                        ctx.copy_text(s);
                        self.edited(i, false);
                        self.play(sound::Effect::Key);
                    }
                }
                egui::Event::Key { key, pressed: true, modifiers, .. } => self.key(i, key, modifiers),
                _ => {}
            }
        }
    }

    /// Marks a tab changed and wakes the caret.
    /// `typed` edits (keys, not paste/undo/cut) count toward the goal.
    fn edited(&mut self, i: usize, typed: bool) {
        self.tabs[i].dirty_since = Some(Instant::now());
        self.bar.last_typed = self.now;
        self.caret_epoch = self.now;
        let w = goal::words(&self.tabs[i].text);
        let delta = w as i64 - std::mem::replace(&mut self.tabs[i].words, w) as i64;
        if typed {
            self.count_typing(delta);
        }
    }

    fn key(&mut self, i: usize, key: egui::Key, m: egui::Modifiers) {
        use editor::Nav;
        use egui::Key;
        let nav = match key {
            Key::ArrowLeft => Some(Nav::Left),
            Key::ArrowRight => Some(Nav::Right),
            Key::ArrowUp => Some(Nav::Up),
            Key::ArrowDown => Some(Nav::Down),
            Key::Home => Some(Nav::Home),
            Key::End => Some(Nav::End),
            Key::PageUp => Some(Nav::PageUp),
            Key::PageDown => Some(Nav::PageDown),
            _ => None,
        };
        let now = self.now;
        let tab = &mut self.tabs[i];
        if let Some(n) = nav {
            tab.ed.nav(&tab.text, n, m.command, m.shift);
            self.caret_epoch = now;
            return;
        }
        match key {
            Key::Enter if !m.command => self.type_char(i, '\n'),
            Key::Tab if !m.command => {
                for _ in 0..4 {
                    self.type_char(i, ' ');
                }
            }
            Key::Backspace => {
                if tab.ed.backspace(&mut tab.text, m.command, now) {
                    self.edited(i, true);
                    self.play(sound::Effect::Key);
                }
            }
            Key::Delete => {
                if tab.ed.delete(&mut tab.text, m.command, now) {
                    self.edited(i, true);
                    self.play(sound::Effect::Key);
                }
            }
            Key::Insert if m.is_none() => {
                self.overtype = !self.overtype;
                self.caret_epoch = now;
            }
            Key::A if m.command => {
                tab.ed.select_all(&tab.text);
                self.caret_epoch = now;
            }
            Key::Z if m.command && m.shift => {
                if tab.ed.redo(&mut tab.text) {
                    self.edited(i, false);
                }
            }
            Key::Z if m.command => {
                if tab.ed.undo(&mut tab.text) {
                    self.edited(i, false);
                }
            }
            Key::Y if m.command => {
                if tab.ed.redo(&mut tab.text) {
                    self.edited(i, false);
                }
            }
            _ => {}
        }
    }

    /// Pasted text lands at once, with a single carriage return sound.
    fn paste(&mut self, i: usize, p: &str) {
        let clean = p.replace("\r\n", "\n").replace('\r', "\n").replace('\t', "    ");
        if clean.is_empty() {
            return;
        }
        let tab = &mut self.tabs[i];
        tab.ed.insert(&mut tab.text, &clean, false, self.now);
        self.edited(i, false);
        self.play(sound::Effect::Return);
    }

    fn type_char(&mut self, i: usize, ch: char) {
        let now = self.now;
        let overtype = self.overtype;
        let tab = &mut self.tabs[i];
        let line_before = editor::line_col(&editor::layout(&editor::chars(&tab.text)), tab.ed.cursor).0;
        tab.ed.insert(&mut tab.text, ch.encode_utf8(&mut [0; 4]), overtype, now);
        let (line, col) = editor::line_col(&editor::layout(&editor::chars(&tab.text)), tab.ed.cursor);
        self.edited(i, true);
        if !ch.is_whitespace() {
            self.last_strike = Some((now, ch));
        }
        if ch == '\n' || line > line_before {
            self.play(sound::Effect::Return);
        } else if col == paper::BELL_COL {
            self.play(sound::Effect::Bell);
        } else if ch == ' ' {
            self.play(sound::Effect::Space);
        } else {
            self.play(sound::Effect::Key);
        }
    }

    /// Draws the open sheet and handles the mouse on it.
    fn editor_view(&mut self, ui: &mut egui::Ui, i: usize) {
        let now = self.now;
        let tab = &mut self.tabs[i];
        let chars = editor::chars(&tab.text);
        let lines = editor::layout(&chars);
        let carriage = editor::line_col(&lines, tab.ed.view_index());
        let caret = editor::line_col(&lines, tab.ed.cursor);
        // Blink: solid while active, then 0.55 s on / 0.45 s off.
        let idle = now - self.caret_epoch.max(self.bar.last_typed);
        let caret_on = idle < 0.6 || idle.fract() < 0.55;
        ui.ctx().request_repaint_after(Duration::from_millis(120));
        let view = paper::View {
            chars: &chars,
            lines: &lines,
            carriage,
            caret,
            caret_on,
            overtype: self.overtype,
            selection: tab.ed.selection(),
            scroll: tab.ed.scroll,
            glow: self.session.celebration(now),
        };
        let strike = self.last_strike.map(|(t, ch)| paper::Strike { age: (now - t) as f32, ch });
        let sheet = paper::draw(ui, &view, &self.settings, strike);

        let ctx = ui.ctx().clone();
        let r = &sheet.response;
        let pos = ctx.input(|inp| inp.pointer.interact_pos());
        let to_index = |p: egui::Pos2| {
            let (l, c) = sheet.line_col_at(p);
            let l = l.clamp(0, lines.len() as isize - 1) as usize;
            editor::index_at(&lines, l, c.max(0) as usize)
        };
        let (pressed, released, shift) =
            ctx.input(|inp| (inp.pointer.primary_pressed(), inp.pointer.primary_released(), inp.modifiers.shift));

        if pressed && r.hovered() {
            if let Some(p) = pos.filter(|&p| sheet.on_paper(p)) {
                let idx = to_index(p);
                tab.ed.view_hold = Some(tab.ed.view_index());
                if shift {
                    tab.ed.move_to(idx, true);
                } else {
                    tab.ed.move_to(idx, false);
                    tab.ed.anchor = Some(idx);
                }
                self.selecting = true;
                self.caret_epoch = now;
            }
        }
        if self.selecting {
            if let Some(p) = pos {
                if r.dragged() {
                    let anchor = tab.ed.anchor;
                    tab.ed.move_to(to_index(p), true);
                    if anchor.is_some() {
                        tab.ed.anchor = anchor;
                    }
                }
                if r.double_clicked() {
                    tab.ed.select_word(&tab.text, to_index(p));
                }
            }
            if released || !ctx.input(|inp| inp.pointer.primary_down()) {
                self.selecting = false;
                tab.ed.view_hold = None;
                if tab.ed.anchor == Some(tab.ed.cursor) {
                    tab.ed.anchor = None;
                }
            }
        }

        // Mouse wheel reads back through the sheet; typing snaps back.
        if r.hovered() {
            let dy = ctx.input(|inp| inp.smooth_scroll_delta.y);
            if dy != 0.0 {
                let above = carriage.0 as f32;
                let below = (lines.len() - 1 - carriage.0) as f32;
                tab.ed.scroll = (tab.ed.scroll + dy / paper::LINE_H).clamp(-below, above);
            }
        }
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        let (new, close, save) = ctx.input_mut(|i| {
            (
                i.consume_key(egui::Modifiers::COMMAND, egui::Key::T) || i.consume_key(egui::Modifiers::COMMAND, egui::Key::N),
                i.consume_key(egui::Modifiers::COMMAND, egui::Key::W),
                i.consume_key(egui::Modifiers::COMMAND, egui::Key::S),
            )
        });
        if new {
            self.new_tab();
        }
        if close {
            if let Some(i) = self.active {
                self.close_tab(i);
            }
        }
        if save {
            if let Some(i) = self.active {
                let t = &mut self.tabs[i];
                t.dirty_since = Some(Instant::now());
                t.save_now();
            }
        }
    }

    fn autosave(&mut self, ctx: &egui::Context) {
        let mut saved = false;
        for t in &mut self.tabs {
            if let Some(since) = t.dirty_since {
                if since.elapsed() >= AUTOSAVE_DELAY {
                    t.save_now();
                    self.notes_stale = true;
                    saved = true;
                } else {
                    ctx.request_repaint_after(AUTOSAVE_DELAY);
                }
            }
        }
        if saved {
            self.sync_words();
        }
    }

    fn confirm_window(&mut self, ctx: &egui::Context) {
        let Some(path) = self.confirm_delete.clone() else { return };
        let title = self.notes.iter().find(|n| n.path == path).map(|n| n.title.clone()).unwrap_or_default();
        let name = if title.is_empty() { "Untitled".to_string() } else { title };
        let mut decision = None;

        // Dim and block the app behind the dialog.
        egui::Area::new(Id::new("modal_block")).order(Order::Foreground).fixed_pos(egui::Pos2::ZERO).show(ctx, |ui| {
            let r = ctx.screen_rect();
            ui.painter().rect_filled(r, 0.0, Color32::from_black_alpha(150));
            if ui.allocate_rect(r, Sense::click()).clicked() {
                decision = Some(false);
            }
        });
        egui::Area::new(Id::new("modal")).order(Order::Tooltip).anchor(Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
            Frame::none()
                .fill(theme::bar())
                .stroke(egui::Stroke::new(1.0f32, theme::border()))
                .rounding(14.0)
                .inner_margin(Margin::same(22.0))
                .show(ui, |ui| {
                    ui.set_width(340.0);
                    ui.label(RichText::new("Delete this sheet?").size(18.0).strong().color(theme::text()));
                    ui.add_space(6.0);
                    ui.label(RichText::new(format!("\u{201c}{name}\u{201d} will be permanently deleted. This cannot be undone.")).color(theme::muted()));
                    ui.add_space(16.0);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let del = egui::Button::new(RichText::new("Delete").color(Color32::WHITE))
                            .fill(Color32::from_rgb(196, 58, 52))
                            .rounding(8.0)
                            .min_size(vec2(84.0, 32.0));
                        if ui.add(del).clicked() {
                            decision = Some(true);
                        }
                        if ui.add(egui::Button::new("Cancel").rounding(8.0).min_size(vec2(84.0, 32.0))).clicked() {
                            decision = Some(false);
                        }
                    });
                });
        });
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            decision = Some(false);
        }
        match decision {
            Some(true) => {
                if let Some(i) = self.tabs.iter().position(|t| t.path == path) {
                    self.tabs[i].dirty_since = None;
                    self.close_tab(i);
                }
                notes::delete(&path);
                self.notes_stale = true;
                self.confirm_delete = None;
            }
            Some(false) => self.confirm_delete = None,
            None => {}
        }
    }

    fn persist_settings(&mut self) {
        self.settings.open_tabs = self.tabs.iter().map(|t| t.path.clone()).collect();
        self.settings.active_tab = self.active;
        if self.settings != self.saved_settings {
            self.settings.save(&self.store.settings_path());
            self.saved_settings = self.settings.clone();
        }
    }

    /// Shows the Save As dialog, then writes the export. Returns a status line.
    fn export_to_file(&mut self, frame: &eframe::Frame, i: usize, format: settings::ExportFormat, name: &str) -> (String, bool) {
        let mut dialog = rfd::FileDialog::new()
            .set_title("Export sheet")
            .set_file_name(format!("{name}.{}", format.ext()))
            .add_filter(format.label(), &[format.ext()])
            .set_parent(frame);
        let dir = self.settings.export_dir.clone().filter(|d| d.is_dir()).or_else(dirs::document_dir);
        if let Some(d) = dir {
            dialog = dialog.set_directory(d);
        }
        let Some(mut path) = dialog.save_file() else {
            return ("Export cancelled".into(), false);
        };
        if path.extension().is_none_or(|e| !e.eq_ignore_ascii_case(format.ext())) {
            path.set_extension(format.ext());
        }
        if let Some(parent) = path.parent() {
            self.settings.export_dir = Some(parent.to_path_buf());
        }
        match export::export(&self.tabs[i].text, &self.settings, format, &path) {
            Ok(msg) => (format!("{msg} in {}", path.parent().map(|p| p.display().to_string()).unwrap_or_default()), false),
            Err(e) => (format!("Export failed: {e}"), true),
        }
    }

    /// Launch-time update check, and restarting once an update is in place.
    fn updates(&mut self, ctx: &egui::Context) {
        if !self.launch_checked {
            self.launch_checked = true;
            let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
            if self.settings.auto_update_check && now.saturating_sub(self.settings.last_update_check) >= 24 * 3600 {
                self.settings.last_update_check = now;
                self.updater.check(ctx);
            }
        }
        if !self.restarting && matches!(self.updater.status(), update::Status::Installed) {
            self.restarting = true;
            for t in &mut self.tabs {
                t.save_now();
            }
            self.sync_words();
            self.persist_settings();
            update::relaunch();
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    /// Short "goal reached" banner at the top of the sheet.
    fn goal_banner(&self, ctx: &egui::Context) {
        let t = self.session.celebration(self.now);
        if t <= 0.0 {
            return;
        }
        ctx.request_repaint();
        egui::Area::new(Id::new("goal_banner"))
            .order(Order::Foreground)
            .interactable(false)
            .anchor(Align2::CENTER_TOP, [0.0, 58.0 - 10.0 * (1.0 - t)])
            .show(ctx, |ui| {
                ui.multiply_opacity(t);
                Frame::none()
                    .fill(theme::bar())
                    .stroke(egui::Stroke::new(1.5f32, theme::accent()))
                    .rounding(18.0)
                    .inner_margin(Margin::symmetric(18.0, 9.0))
                    .show(ui, |ui| {
                        let msg = format!("Goal reached \u{00b7} {} today", goal::n_words(self.today_words));
                        ui.label(RichText::new(msg).size(14.5).strong().color(theme::text()));
                    });
            });
    }

    fn card_look(&self) -> home::Look {
        let p = self.settings.paper;
        let dark = (0.299 * p[0] as f32 + 0.587 * p[1] as f32 + 0.114 * p[2] as f32) < 110.0;
        home::Look {
            paper: Color32::from_rgb(p[0], p[1], p[2]),
            ink: if dark { Color32::from_rgb(232, 226, 212) } else { Color32::from_rgb(30, 28, 34) },
            family: self.settings.font.family(),
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.now = ctx.input(|i| i.time);
        let on_edge = chrome::handle_resize(ctx);
        self.updates(ctx);
        self.shortcuts(ctx);
        self.handle_typing(ctx);
        self.autosave(ctx);
        self.roll_day();

        let low = self.settings.light_on;
        if low != self.low_light_applied {
            theme::apply(ctx, low);
            self.low_light_applied = low;
        }

        let titles: Vec<String> = self.tabs.iter().map(Tab::title).collect();
        for a in chrome::title_bar(ctx, &titles, self.active, on_edge) {
            match a {
                chrome::Action::Home => self.active = None,
                chrome::Action::Select(i) => self.active = Some(i),
                chrome::Action::Close(i) => self.close_tab(i),
                chrome::Action::New => self.new_tab(),
            }
        }

        if self.active.is_none() && self.notes_stale {
            self.notes = self.store.list();
            self.tags.prune(&self.notes.iter().map(|n| n.path.clone()).collect::<Vec<_>>());
            self.notes_stale = false;
        }

        match self.active {
            Some(i) => {
                egui::CentralPanel::default().frame(Frame::none()).show(ctx, |ui| self.editor_view(ui, i));
                self.goal_banner(ctx);
            }
            None => {
                let look = self.card_look();
                let mut action = None;
                egui::CentralPanel::default()
                    .frame(Frame::none().fill(theme::bg()))
                    .show(ctx, |ui| {
                        action = home::show(ui, &self.notes, &mut self.search, &look, &mut self.tags, &mut self.settings, &mut self.home)
                    });
                match action {
                    Some(home::Action::New) => self.new_tab(),
                    Some(home::Action::Open(p)) => self.open_path(p),
                    Some(home::Action::Delete(p)) => self.confirm_delete = Some(p),
                    None => {}
                }
            }
        }

        let editor_title = self.active.map(|i| notes::title_of(&self.tabs[i].text));
        let sheet = self.active.map(|i| self.tabs[i].path.clone());
        let upd_status = self.updater.status();
        let stats = self.active.map(|i| bar::Stats {
            sheet_words: self.tabs[i].words,
            sheet_chars: goal::chars(&self.tabs[i].text),
            today: self.today_words,
            session_words: self.session.words,
            session_secs: self.session.active_secs,
        });
        match bar::show(ctx, &mut self.settings, &mut self.bar, editor_title.as_deref(), sheet.as_deref(), &mut self.tags, &upd_status, stats.as_ref(), self.now) {
            Some(bar::Action::ResetToday) => {
                self.today_words = 0;
                self.settings.goal_hit_day = 0;
                self.sync_words();
            }
            Some(bar::Action::Export(format, name)) => {
                if let Some(i) = self.active {
                    self.bar.export_status = Some(self.export_to_file(frame, i, format, &name));
                }
            }
            Some(bar::Action::CopyText) => {
                if let Some(i) = self.active {
                    ctx.copy_text(self.tabs[i].text.clone());
                    self.bar.export_status = Some(("Text copied to clipboard".into(), false));
                }
            }
            Some(bar::Action::CopyImage) => {
                if let Some(i) = self.active {
                    self.bar.export_status = Some(match export::copy_image(&self.tabs[i].text, &self.settings) {
                        Ok(()) => ("Sheet image copied to clipboard".into(), false),
                        Err(e) => (format!("Copy failed: {e}"), true),
                    });
                }
            }
            Some(bar::Action::CheckUpdate) => self.updater.check(ctx),
            Some(bar::Action::InstallUpdate) => self.updater.install(ctx),
            Some(bar::Action::OpenUrl(u)) => update::open_url(&u),
            Some(bar::Action::Insert(e)) => {
                if let Some(i) = self.active {
                    for ch in e.chars() {
                        self.type_char(i, ch);
                    }
                }
            }
            Some(bar::Action::TestSound) => {
                if let Some(s) = &mut self.sound {
                    s.set_pack(self.settings.pack);
                }
                for e in [sound::Effect::Key, sound::Effect::Key, sound::Effect::Space, sound::Effect::Key] {
                    self.play(e);
                }
            }
            None => {}
        }
        if let Some(s) = &mut self.sound {
            s.set_pack(self.settings.pack);
        }

        self.confirm_window(ctx);
        self.persist_settings();
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        for t in &mut self.tabs {
            t.save_now();
        }
        self.sync_words();
        self.persist_settings();
    }
}
