//! Exporting a sheet: plain text, Markdown, and styled renders of the page
//! (PNG / JPEG / PDF) in the user's paper colour and typeface, with the same
//! ink jitter as the editor. Long notes split into A4 pages.

use crate::paper::LINE_COLS;
use crate::settings::{ExportFormat, Settings};
use crate::theme;
use ab_glyph::{point, Font, FontRef, PxScale, ScaleFont};
use image::{codecs::jpeg::JpegEncoder, ImageFormat, RgbImage};
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};

/// A4 at 150 dpi.
const PAGE_W: u32 = 1240;
const PAGE_H: u32 = 1754;
const MARGIN_X: f32 = 110.0;
const MARGIN_Y: f32 = 130.0;
/// Same line spacing as the editor (LINE_H / FONT_SIZE).
const LINE_RATIO: f32 = 1.5;

/// Characters not allowed in Windows file names are removed.
pub fn sanitize(name: &str) -> String {
    let s: String =
        name.chars().filter(|c| !c.is_control() && !matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*')).collect();
    let s = s.trim().trim_end_matches('.').trim();
    let s: String = s.chars().take(80).collect();
    if s.is_empty() {
        "Untitled".into()
    } else {
        s
    }
}

/// Note text as Markdown: the first line becomes a heading and line breaks
/// are kept as hard breaks.
fn to_markdown(text: &str) -> String {
    let mut lines = text.lines().skip_while(|l| l.trim().is_empty());
    let mut out = String::new();
    if let Some(first) = lines.next() {
        out.push_str("# ");
        out.push_str(first.trim());
        out.push_str("\n\n");
    }
    let rest: Vec<&str> = lines.skip_while(|l| l.trim().is_empty()).collect();
    let n = rest.len();
    for (i, l) in rest.iter().enumerate() {
        let l = l.trim_end();
        out.push_str(l);
        let next_blank = rest.get(i + 1).map_or(true, |x| x.trim().is_empty());
        if i + 1 < n && !l.is_empty() && !next_blank {
            out.push_str("  ");
        }
        out.push('\n');
    }
    out
}

/// Soft-wraps lines exactly as the editor shows them.
fn layout_lines(text: &str) -> Vec<Vec<char>> {
    let chars = crate::editor::chars(text);
    let mut out: Vec<Vec<char>> =
        crate::editor::layout(&chars).iter().map(|v| chars[v.start..v.start + v.len].to_vec()).collect();
    while out.len() > 1 && out.last().is_some_and(Vec::is_empty) {
        out.pop();
    }
    out
}

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

fn is_dark(c: [u8; 3]) -> bool {
    (0.299 * c[0] as f32 + 0.587 * c[1] as f32 + 0.114 * c[2] as f32) < 110.0
}

struct Typesetter<'a> {
    main: FontRef<'a>,
    fallback: FontRef<'a>,
    px: f32,
    cw: f32,
    line_h: f32,
    ascent: f32,
    paper: [u8; 3],
    ink: [u8; 3],
    dark: bool,
}

impl<'a> Typesetter<'a> {
    fn new(main: &'a [u8], fallback: &'a [u8], s: &Settings) -> Option<Self> {
        let fallback = FontRef::try_from_slice(fallback).ok()?;
        let main = FontRef::try_from_slice(main).unwrap_or_else(|_| fallback.clone());
        let cw = (PAGE_W as f32 - 2.0 * MARGIN_X) / LINE_COLS as f32;
        let adv = main.as_scaled(PxScale::from(100.0)).h_advance(main.glyph_id('M')).max(1.0);
        let px = cw * 100.0 / adv;
        let ascent = main.as_scaled(PxScale::from(px)).ascent();
        let dark = is_dark(s.paper);
        Some(Self {
            main,
            fallback,
            px,
            cw,
            line_h: px * LINE_RATIO,
            ascent,
            paper: s.paper,
            ink: if dark { [232, 226, 212] } else { [26, 24, 30] },
            dark,
        })
    }

    fn lines_per_page(&self) -> usize {
        (((PAGE_H as f32 - 2.0 * MARGIN_Y) / self.line_h) as usize).max(1)
    }

    /// Blank sheet with fine paper grain.
    fn blank(&self, h: u32) -> RgbImage {
        let amp = if self.dark { 3.0 } else { 5.0 };
        let mut img = RgbImage::new(PAGE_W, h);
        for (x, y, px) in img.enumerate_pixels_mut() {
            // Grain in 2x2 specks keeps PNG files small.
            let n = unit(hash(x as usize / 2, y as usize / 2, 7), 0) - 0.5;
            let n = (n * 3.0).round() / 3.0;
            // Faint horizontal fibres every so often.
            let fibre = if hash(x as usize / 9, y as usize, 3) % 97 == 0 { -2.5 } else { 0.0 };
            let d = n * amp + fibre;
            *px = image::Rgb(self.paper.map(|c| (c as f32 + d).clamp(0.0, 255.0) as u8));
        }
        img
    }

    fn blend(&self, img: &mut RgbImage, x: i32, y: i32, a: f32) {
        if x < 0 || y < 0 || x >= img.width() as i32 || y >= img.height() as i32 || a <= 0.0 {
            return;
        }
        let p = img.get_pixel_mut(x as u32, y as u32);
        for k in 0..3 {
            p.0[k] = (p.0[k] as f32 * (1.0 - a) + self.ink[k] as f32 * a).round() as u8;
        }
    }

    fn glyph(&self, img: &mut RgbImage, ch: char, x: f32, baseline: f32, alpha: f32) {
        let font = if self.main.glyph_id(ch).0 != 0 { &self.main } else { &self.fallback };
        let g = font.glyph_id(ch).with_scale_and_position(PxScale::from(self.px), point(x, baseline));
        if let Some(og) = font.outline_glyph(g) {
            let b = og.px_bounds();
            og.draw(|gx, gy, cov| self.blend(img, b.min.x as i32 + gx as i32, b.min.y as i32 + gy as i32, cov * alpha));
        }
    }

    /// Draws `lines` (indices `first..` of the whole note, for stable jitter)
    /// onto a sheet of height `h`.
    fn render(&self, lines: &[Vec<char>], first: usize, h: u32, page_label: Option<String>) -> RgbImage {
        let mut img = self.blank(h);
        let j = self.cw / 13.2; // jitter scale relative to the editor
        for (i, line) in lines.iter().enumerate() {
            let l = first + i;
            let baseline = MARGIN_Y + i as f32 * self.line_h + self.ascent;
            for (c, &ch) in line.iter().enumerate() {
                if ch.is_whitespace() {
                    continue;
                }
                let hsh = hash(l, c, ch as u32);
                let dx = (unit(hsh, 0) - 0.5) * 1.6 * j;
                let dy = (unit(hsh, 8) - 0.5) * 1.8 * j;
                let strength = 0.62 + 0.38 * unit(hsh, 16).powf(0.4);
                let x = MARGIN_X + c as f32 * self.cw + dx;
                self.glyph(&mut img, ch, x, baseline + dy, strength);
                if unit(hsh, 24) > 0.85 {
                    self.glyph(&mut img, ch, x + 0.6 * j, baseline + dy + 0.3 * j, 0.27);
                }
            }
        }
        if let Some(label) = page_label {
            let w = label.chars().count() as f32 * self.cw;
            let baseline = h as f32 - MARGIN_Y * 0.5;
            for (k, ch) in label.chars().enumerate() {
                self.glyph(&mut img, ch, (PAGE_W as f32 - w) / 2.0 + k as f32 * self.cw, baseline, 0.45);
            }
        }
        img
    }

    fn pages(&self, text: &str) -> Vec<RgbImage> {
        let lines = layout_lines(text);
        let per = self.lines_per_page();
        let chunks: Vec<&[Vec<char>]> = lines.chunks(per).collect();
        let n = chunks.len();
        chunks
            .iter()
            .enumerate()
            .map(|(p, chunk)| self.render(chunk, p * per, PAGE_H, (n > 1).then(|| format!("- {} -", p + 1))))
            .collect()
    }

    /// The whole note on one sheet, tall as needed (at least A4).
    fn single(&self, text: &str) -> RgbImage {
        let lines = layout_lines(text);
        let h = (2.0 * MARGIN_Y + lines.len() as f32 * self.line_h).ceil() as u32;
        self.render(&lines, 0, h.max(PAGE_H), None)
    }
}

fn with_fonts<T>(s: &Settings, f: impl FnOnce(&Typesetter) -> T) -> Result<T, String> {
    let main = theme::font_bytes(s.font);
    let fallback = theme::hack_bytes();
    let ts = Typesetter::new(&main, &fallback, s).ok_or("could not load the typeface")?;
    Ok(f(&ts))
}

fn jpeg(img: &RgbImage, quality: u8) -> Result<Vec<u8>, String> {
    let mut buf = Vec::new();
    JpegEncoder::new_with_quality(&mut buf, quality).encode_image(img).map_err(|e| e.to_string())?;
    Ok(buf)
}

/// Minimal PDF with one full-bleed JPEG per A4 page.
fn pdf(pages: &[Vec<u8>]) -> Vec<u8> {
    let mut out: Vec<u8> = b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n".to_vec();
    let mut offsets = Vec::new();
    let n = pages.len();
    // Object ids: 1 catalog, 2 pages, then per page: page, content, image.
    let page_id = |i: usize| 3 + i * 3;
    let mut obj = |out: &mut Vec<u8>, id: usize, body: &[u8]| {
        offsets.push((id, out.len()));
        out.extend_from_slice(format!("{id} 0 obj\n").as_bytes());
        out.extend_from_slice(body);
        out.extend_from_slice(b"\nendobj\n");
    };
    obj(&mut out, 1, b"<< /Type /Catalog /Pages 2 0 R >>");
    let kids: Vec<String> = (0..n).map(|i| format!("{} 0 R", page_id(i))).collect();
    obj(&mut out, 2, format!("<< /Type /Pages /Kids [{}] /Count {n} >>", kids.join(" ")).as_bytes());
    let (w, h) = (595.28f32, 841.89f32);
    for (i, jpg) in pages.iter().enumerate() {
        let (pid, cid, iid) = (page_id(i), page_id(i) + 1, page_id(i) + 2);
        obj(
            &mut out,
            pid,
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {w} {h}] /Resources << /XObject << /Im0 {iid} 0 R >> >> /Contents {cid} 0 R >>"
            )
            .as_bytes(),
        );
        let content = format!("q {w} 0 0 {h} 0 0 cm /Im0 Do Q");
        obj(&mut out, cid, format!("<< /Length {} >>\nstream\n{content}\nendstream", content.len()).as_bytes());
        let mut body = format!(
            "<< /Type /XObject /Subtype /Image /Width {PAGE_W} /Height {PAGE_H} /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /DCTDecode /Length {} >>\nstream\n",
            jpg.len()
        )
        .into_bytes();
        body.extend_from_slice(jpg);
        body.extend_from_slice(b"\nendstream");
        obj(&mut out, iid, &body);
    }
    offsets.sort();
    let xref = out.len();
    let count = offsets.len() + 1;
    out.extend_from_slice(format!("xref\n0 {count}\n0000000000 65535 f \n").as_bytes());
    for (_, off) in &offsets {
        out.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(format!("trailer\n<< /Size {count} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n").as_bytes());
    out
}

/// `foo.png` page 2 becomes `foo-2.png`.
fn numbered(path: &Path, n: usize) -> PathBuf {
    let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let ext = path.extension().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    path.with_file_name(format!("{stem}-{n}.{ext}"))
}

/// Writes the note to `path` in `format`. Returns a short status line.
pub fn export(text: &str, s: &Settings, format: ExportFormat, path: &Path) -> Result<String, String> {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    match format {
        ExportFormat::Txt => {
            fs::write(path, text).map_err(|e| e.to_string())?;
            Ok(format!("Saved {name}"))
        }
        ExportFormat::Md => {
            fs::write(path, to_markdown(text)).map_err(|e| e.to_string())?;
            Ok(format!("Saved {name}"))
        }
        ExportFormat::Pdf => {
            let pages = with_fonts(s, |ts| ts.pages(text))?;
            let jpgs = pages.iter().map(|p| jpeg(p, 90)).collect::<Result<Vec<_>, _>>()?;
            fs::write(path, pdf(&jpgs)).map_err(|e| e.to_string())?;
            let n = jpgs.len();
            Ok(if n > 1 { format!("Saved {name} ({n} pages)") } else { format!("Saved {name}") })
        }
        ExportFormat::Png | ExportFormat::Jpeg => {
            let pages = with_fonts(s, |ts| ts.pages(text))?;
            let n = pages.len();
            for (i, page) in pages.iter().enumerate() {
                let target = if n > 1 { numbered(path, i + 1) } else { path.to_path_buf() };
                let bytes = if format == ExportFormat::Png {
                    let mut buf = Cursor::new(Vec::new());
                    page.write_to(&mut buf, ImageFormat::Png).map_err(|e| e.to_string())?;
                    buf.into_inner()
                } else {
                    jpeg(page, 92)?
                };
                fs::write(&target, bytes).map_err(|e| e.to_string())?;
            }
            Ok(if n > 1 { format!("Saved {n} pages as {}", numbered(Path::new(&name), 1).display()) } else { format!("Saved {name}") })
        }
    }
}

/// Copies the whole sheet to the clipboard as an image.
pub fn copy_image(text: &str, s: &Settings) -> Result<(), String> {
    let img = with_fonts(s, |ts| ts.single(text))?;
    let (w, h) = img.dimensions();
    let rgba: Vec<u8> = img.pixels().flat_map(|p| [p.0[0], p.0[1], p.0[2], 255]).collect();
    let mut cb = arboard::Clipboard::new().map_err(|e| e.to_string())?;
    cb.set_image(arboard::ImageData { width: w as usize, height: h as usize, bytes: rgba.into() }).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Writes every format for a two-page note into RUSTYPE_EXPORT_DIR
    /// (or the temp folder) so the output can be inspected.
    #[test]
    fn exports_all_formats() {
        let dir = std::env::var_os("RUSTYPE_EXPORT_DIR").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
        let mut text = String::from("A letter from the typewriter\n\nDear reader,\n");
        for i in 0..60 {
            text.push_str(&format!("Line {i}: the quick brown fox jumps over the lazy dog. :) <3\n"));
        }
        let s = Settings::default();
        for f in ExportFormat::ALL {
            let msg = export(&text, &s, f, &dir.join(format!("rt-test.{}", f.ext()))).unwrap();
            println!("{msg}");
        }
        let night = Settings { paper: [34, 34, 38], font: crate::settings::TypeFont::CourierPrime, ..Settings::default() };
        export("Night sheet\n\nwhite ink on dark paper", &night, ExportFormat::Png, &dir.join("rt-night.png")).unwrap();
        assert_eq!(sanitize("  a<b>:c?.  "), "abc");
        assert!(to_markdown("Title\nline one\nline two\n").starts_with("# Title\n\nline one  \nline two\n"));
    }
}
