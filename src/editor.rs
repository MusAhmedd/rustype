//! Editing a sheet: soft-wrapped layout, cursor, selection and undo.
//! Positions are char indices into the note text.

use crate::paper::LINE_COLS;

/// One line as shown on the paper.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VLine {
    pub start: usize,
    pub len: usize,
    /// Ends at a real line break (or the end of the text), not a wrap.
    pub para_end: bool,
}

impl VLine {
    /// Furthest column the cursor may sit on in this line.
    pub fn max_col(&self) -> usize {
        if self.para_end {
            self.len
        } else {
            self.len.saturating_sub(1)
        }
    }
}

/// Soft-wraps at a word boundary within `LINE_COLS` columns; a word longer
/// than a line is broken at the edge. The space that ends a wrapped line
/// stays on it.
pub fn layout(chars: &[char]) -> Vec<VLine> {
    let mut out = Vec::new();
    let n = chars.len();
    let mut i = 0;
    loop {
        let pe = chars[i..].iter().position(|&c| c == '\n').map_or(n, |p| i + p);
        let mut s = i;
        while pe - s > LINE_COLS {
            let window = &chars[s..=s + LINE_COLS];
            let brk = window.iter().rposition(|&c| c == ' ').filter(|&p| p > 0).map_or(LINE_COLS, |p| p + 1);
            out.push(VLine { start: s, len: brk, para_end: false });
            s += brk;
        }
        out.push(VLine { start: s, len: pe - s, para_end: true });
        if pe >= n {
            break;
        }
        i = pe + 1;
    }
    out
}

/// (line, column) of a char index.
pub fn line_col(lines: &[VLine], idx: usize) -> (usize, usize) {
    let l = lines.partition_point(|v| v.start <= idx).saturating_sub(1);
    (l, idx - lines[l].start)
}

/// Char index at a (line, column), clamped to the line.
pub fn index_at(lines: &[VLine], line: usize, col: usize) -> usize {
    let v = lines[line.min(lines.len() - 1)];
    v.start + col.min(v.max_col())
}

fn byte_at(text: &str, idx: usize) -> usize {
    text.char_indices().nth(idx).map_or(text.len(), |(b, _)| b)
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    None,
    Word,
    Space,
    Delete,
    Other,
}

struct Snap {
    text: String,
    cursor: usize,
}

pub enum Nav {
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    PageUp,
    PageDown,
}

const PAGE_LINES: usize = 10;
const UNDO_LIMIT: usize = 400;

pub struct State {
    pub cursor: usize,
    /// Other end of the selection, if any.
    pub anchor: Option<usize>,
    /// Column kept while moving up and down.
    want_col: Option<usize>,
    undo: Vec<Snap>,
    redo: Vec<Snap>,
    last_kind: Kind,
    last_time: f64,
    /// Read-back offset in lines; positive shows earlier lines.
    pub scroll: f32,
    /// While dragging a selection the paper holds still at this index.
    pub view_hold: Option<usize>,
}

impl State {
    pub fn new(text: &str) -> Self {
        Self {
            cursor: text.chars().count(),
            anchor: None,
            want_col: None,
            undo: Vec::new(),
            redo: Vec::new(),
            last_kind: Kind::None,
            last_time: 0.0,
            scroll: 0.0,
            view_hold: None,
        }
    }

    /// Where the carriage should sit.
    pub fn view_index(&self) -> usize {
        self.view_hold.unwrap_or(self.cursor)
    }

    pub fn selection(&self) -> Option<(usize, usize)> {
        self.anchor.filter(|&a| a != self.cursor).map(|a| (a.min(self.cursor), a.max(self.cursor)))
    }

    pub fn selected_text(&self, text: &str) -> Option<String> {
        self.selection().map(|(a, b)| text.chars().skip(a).take(b - a).collect())
    }

    fn snapshot(&mut self, text: &str, kind: Kind, now: f64) {
        let fresh = kind != self.last_kind || kind == Kind::Other || now - self.last_time > 1.5;
        if fresh {
            self.undo.push(Snap { text: text.to_owned(), cursor: self.cursor });
            if self.undo.len() > UNDO_LIMIT {
                self.undo.remove(0);
            }
        }
        self.redo.clear();
        self.last_kind = kind;
        self.last_time = now;
    }

    fn touched(&mut self) {
        self.want_col = None;
        self.scroll = 0.0;
    }

    fn remove(&mut self, text: &mut String, a: usize, b: usize) {
        let (ba, bb) = (byte_at(text, a), byte_at(text, b));
        text.replace_range(ba..bb, "");
        self.cursor = a;
        self.anchor = None;
    }

    /// Inserts `s` at the cursor, replacing any selection. With `overtype`
    /// a single typed char replaces the one under the cursor.
    pub fn insert(&mut self, text: &mut String, s: &str, overtype: bool, now: f64) {
        let kind = match s.chars().next() {
            Some(c) if s.chars().count() == 1 && c.is_whitespace() => Kind::Space,
            Some(_) if s.chars().count() == 1 => Kind::Word,
            _ => Kind::Other,
        };
        self.snapshot(text, kind, now);
        if let Some((a, b)) = self.selection() {
            self.remove(text, a, b);
        } else if overtype && s.chars().count() == 1 {
            if text.chars().nth(self.cursor).is_some_and(|c| c != '\n') {
                self.remove(text, self.cursor, self.cursor + 1);
            }
        }
        let at = byte_at(text, self.cursor);
        text.insert_str(at, s);
        self.cursor += s.chars().count();
        self.anchor = None;
        self.touched();
    }

    pub fn backspace(&mut self, text: &mut String, word: bool, now: f64) -> bool {
        if let Some((a, b)) = self.selection() {
            self.snapshot(text, Kind::Other, now);
            self.remove(text, a, b);
        } else if self.cursor > 0 {
            self.snapshot(text, Kind::Delete, now);
            let from = if word { word_left(&chars(text), self.cursor) } else { self.cursor - 1 };
            self.remove(text, from, self.cursor);
        } else {
            return false;
        }
        self.touched();
        true
    }

    pub fn delete(&mut self, text: &mut String, word: bool, now: f64) -> bool {
        let n = text.chars().count();
        if let Some((a, b)) = self.selection() {
            self.snapshot(text, Kind::Other, now);
            self.remove(text, a, b);
        } else if self.cursor < n {
            self.snapshot(text, Kind::Delete, now);
            let to = if word { word_right(&chars(text), self.cursor) } else { self.cursor + 1 };
            let c = self.cursor;
            self.remove(text, c, to);
        } else {
            return false;
        }
        self.touched();
        true
    }

    /// Removes and returns the selection.
    pub fn cut(&mut self, text: &mut String, now: f64) -> Option<String> {
        let s = self.selected_text(text)?;
        self.backspace(text, false, now);
        Some(s)
    }

    pub fn undo(&mut self, text: &mut String) -> bool {
        let Some(s) = self.undo.pop() else { return false };
        self.redo.push(Snap { text: std::mem::replace(text, s.text), cursor: self.cursor });
        self.cursor = s.cursor.min(text.chars().count());
        self.after_history();
        true
    }

    pub fn redo(&mut self, text: &mut String) -> bool {
        let Some(s) = self.redo.pop() else { return false };
        self.undo.push(Snap { text: std::mem::replace(text, s.text), cursor: self.cursor });
        self.cursor = s.cursor.min(text.chars().count());
        self.after_history();
        true
    }

    fn after_history(&mut self) {
        self.anchor = None;
        self.last_kind = Kind::None;
        self.touched();
    }

    /// Moves the cursor, extending the selection when `extend` is set.
    pub fn move_to(&mut self, idx: usize, extend: bool) {
        if extend {
            self.anchor.get_or_insert(self.cursor);
        } else {
            self.anchor = None;
        }
        self.cursor = idx;
        self.want_col = None;
        self.last_kind = Kind::None;
        self.scroll = 0.0;
    }

    pub fn select_all(&mut self, text: &str) {
        self.anchor = Some(0);
        self.cursor = text.chars().count();
        self.touched();
    }

    pub fn select_word(&mut self, text: &str, idx: usize) {
        let c = chars(text);
        let is_word = |i: usize| c.get(i).is_some_and(|ch| ch.is_alphanumeric() || *ch == '_' || *ch == '\'');
        let (mut a, mut b) = (idx, idx);
        if is_word(idx) || (idx > 0 && is_word(idx - 1)) {
            while a > 0 && is_word(a - 1) {
                a -= 1;
            }
            while is_word(b) {
                b += 1;
            }
        } else if idx < c.len() && c[idx] != '\n' {
            b = idx + 1;
        }
        self.anchor = Some(a);
        self.cursor = b;
        self.touched();
    }

    pub fn nav(&mut self, text: &str, nav: Nav, word: bool, extend: bool) {
        let c = chars(text);
        let lines = layout(&c);
        // Collapsing a selection with plain Left/Right jumps to its edge.
        if !extend {
            if let Some((a, b)) = self.selection() {
                match nav {
                    Nav::Left if !word => return self.move_to(a, false),
                    Nav::Right if !word => return self.move_to(b, false),
                    _ => {}
                }
            }
        }
        let (line, col) = line_col(&lines, self.cursor);
        let want = self.want_col.unwrap_or(col);
        let vertical = |to: usize| index_at(&lines, to, want);
        let (idx, keep_col) = match nav {
            Nav::Left if word => (word_left(&c, self.cursor), false),
            Nav::Right if word => (word_right(&c, self.cursor), false),
            Nav::Left => (self.cursor.saturating_sub(1), false),
            Nav::Right => ((self.cursor + 1).min(c.len()), false),
            Nav::Home if word => (0, false),
            Nav::End if word => (c.len(), false),
            Nav::Home => (lines[line].start, false),
            Nav::End => (lines[line].start + lines[line].max_col(), false),
            Nav::Up if line == 0 => (0, false),
            Nav::Up => (vertical(line - 1), true),
            Nav::Down if line + 1 >= lines.len() => (c.len(), false),
            Nav::Down => (vertical(line + 1), true),
            Nav::PageUp => (vertical(line.saturating_sub(PAGE_LINES)), true),
            Nav::PageDown => (vertical((line + PAGE_LINES).min(lines.len() - 1)), true),
        };
        self.move_to(idx, extend);
        self.want_col = keep_col.then_some(want);
    }
}

pub fn chars(text: &str) -> Vec<char> {
    text.chars().collect()
}

fn word_left(c: &[char], mut i: usize) -> usize {
    while i > 0 && c[i - 1].is_whitespace() {
        i -= 1;
    }
    while i > 0 && !c[i - 1].is_whitespace() {
        i -= 1;
    }
    i
}

fn word_right(c: &[char], mut i: usize) -> usize {
    while i < c.len() && !c[i].is_whitespace() {
        i += 1;
    }
    while i < c.len() && c[i].is_whitespace() && c[i] != '\n' {
        i += 1;
    }
    i
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wraps_at_words() {
        let t: String = "word ".repeat(20);
        let c = chars(&t);
        let l = layout(&c);
        assert!(l.len() > 1);
        assert!(l.iter().all(|v| v.len <= LINE_COLS + 1));
        assert!(!l[0].para_end);
        // Every char is on exactly one line.
        let total: usize = l.iter().map(|v| v.len).sum();
        assert_eq!(total, c.len());
        // Cursor at a wrap boundary sits at the start of the next line.
        assert_eq!(line_col(&l, l[1].start), (1, 0));
    }

    #[test]
    fn long_word_breaks_at_edge() {
        let t = "x".repeat(150);
        let l = layout(&chars(&t));
        assert_eq!(l.iter().map(|v| v.len).collect::<Vec<_>>(), vec![64, 64, 22]);
    }

    #[test]
    fn edit_undo_redo() {
        let mut t = String::from("hello world");
        let mut s = State::new(&t);
        s.nav(&t, Nav::Left, true, false); // before "world"
        s.insert(&mut t, "big ", false, 0.0);
        assert_eq!(t, "hello big world");
        s.nav(&t, Nav::Home, false, false);
        s.nav(&t, Nav::Right, true, true); // select "hello "
        assert_eq!(s.selected_text(&t).as_deref(), Some("hello "));
        s.backspace(&mut t, false, 10.0);
        assert_eq!(t, "big world");
        s.undo(&mut t);
        assert_eq!(t, "hello big world");
        s.undo(&mut t);
        assert_eq!(t, "hello world");
        s.redo(&mut t);
        assert_eq!(t, "hello big world");
        s.cursor = 0;
        s.insert(&mut t, "J", true, 20.0);
        assert_eq!(t, "Jello big world");
    }

    #[test]
    fn up_down_keeps_column() {
        let t = String::from("abcdef\nab\nabcdef");
        let mut s = State::new(&t);
        s.nav(&t, Nav::Up, false, false);
        s.nav(&t, Nav::Up, false, false);
        assert_eq!(s.cursor, 6);
        s.move_to(5, false);
        s.nav(&t, Nav::Down, false, false);
        assert_eq!(s.cursor, 9); // clamped to end of "ab"
        s.nav(&t, Nav::Down, false, false);
        assert_eq!(s.cursor, 15); // back to column 5
    }
}
