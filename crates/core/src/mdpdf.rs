//! Markdown → PDF. CommonMark with tables, task lists and strikethrough,
//! laid out on A4 or Letter pages with the bundled Liberation fonts: text is
//! shaped and embedded as subsets, so it can be selected, copied and searched.
//! Headings become bookmarks. Characters no font has are drawn as "?" and
//! counted in the report rather than failing the whole document.

use crate::error::{Error, Result};
use crate::pdf::{self, n};
use crate::typeset::{Line, Typesetter};
use lopdf::{Dictionary, Document, Object, ObjectId, Stream, dictionary};
use pulldown_cmark::{Alignment, Event, HeadingLevel, Options as Md, Parser, Tag, TagEnd};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct MdPdfOptions {
    /// Bundled font key (e.g. `sans-bold`) → index among the font inputs,
    /// which follow the Markdown file.
    pub bundled: BTreeMap<String, usize>,
    /// `a4` or `letter`.
    pub page_size: String,
    /// Margin on every side, in points.
    pub margin: f32,
    /// Body text size, in points.
    pub font_size: f32,
    /// `sans` or `serif`; code is always in the mono family.
    pub font: String,
    /// Document title; the first heading when empty.
    pub title: String,
    pub page_numbers: bool,
}

impl Default for MdPdfOptions {
    fn default() -> Self {
        MdPdfOptions {
            bundled: BTreeMap::new(),
            page_size: "a4".into(),
            margin: 50.0,
            font_size: 10.5,
            font: "sans".into(),
            title: String::new(),
            page_numbers: true,
        }
    }
}

#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MdPdfReport {
    pub pages: usize,
    /// Characters no font could draw, each replaced with "?".
    pub missing: String,
    pub replaced: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct Style {
    bold: bool,
    italic: bool,
    mono: bool,
    link: bool,
}

/// A run of inline text in one style; `"\n"` alone is a hard line break.
#[derive(Debug, Clone)]
struct Piece {
    text: String,
    style: Style,
}

/// One word (or word fragment) measured and ready to draw.
struct Word {
    line: Line,
    size: f32,
    style: Style,
    /// Room for a space after it, 0 when the next piece follows directly.
    space: f32,
    brk: bool,
}

#[derive(Clone, Copy)]
struct Faces {
    regular: usize,
    bold: usize,
    italic: usize,
    bolditalic: usize,
    mono: usize,
    mono_bold: usize,
}

struct Writer<'a> {
    ts: Typesetter<'a>,
    faces: Faces,
    doc: Document,
    pages_id: ObjectId,
    pages: Vec<(ObjectId, Vec<u8>)>,
    width: f32,
    height: f32,
    margin: f32,
    body: f32,
    y: f32,
    missing: Vec<char>,
    replaced: usize,
    outline: Vec<(String, usize, f32)>,
}

const GRAY_RULE: &str = "0.75 G";
const LINE_GAP: f32 = 1.38;

impl<'a> Writer<'a> {
    fn chain(&self, s: Style) -> Vec<usize> {
        let f = self.faces;
        let primary = match (s.mono, s.bold, s.italic) {
            (true, true, _) => f.mono_bold,
            (true, false, _) => f.mono,
            (false, true, true) => f.bolditalic,
            (false, true, false) => f.bold,
            (false, false, true) => f.italic,
            (false, false, false) => f.regular,
        };
        let mut c = vec![primary];
        for x in [f.regular, f.mono] {
            if !c.contains(&x) {
                c.push(x);
            }
        }
        c
    }

    /// Swap characters no face in `chain` has for a close stand-in (√ for ✓,
    /// -> for →…) or else "?", so layout cannot fail.
    fn drawable(&mut self, chain: &[usize], text: &str) -> String {
        let has = |ts: &Typesetter, c: char| c == ' ' || chain.iter().any(|&f| ts.covers(f, c));
        let mut out = String::with_capacity(text.len());
        for c in text.chars().filter(|c| *c == ' ' || !c.is_control()) {
            if has(&self.ts, c) {
                out.push(c);
                continue;
            }
            match stand_ins(c)
                .iter()
                .find(|alt| alt.chars().all(|a| has(&self.ts, a)))
            {
                Some(alt) => out.push_str(alt),
                None => {
                    if !self.missing.contains(&c) {
                        self.missing.push(c);
                    }
                    self.replaced += 1;
                    out.push('?');
                }
            }
        }
        out
    }

    fn measure(&mut self, text: &str, style: Style, size: f32) -> Result<Line> {
        let chain = self.chain(style);
        let t = self.drawable(&chain, text);
        self.ts.layout(&chain, &t, size)
    }

    fn new_page(&mut self) {
        let page = self.doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => self.pages_id,
            "MediaBox" => vec![0.into(), 0.into(), self.width.into(), self.height.into()],
            "Resources" => Dictionary::new(),
        });
        self.pages.push((page, vec![]));
        self.y = self.height - self.margin;
    }

    fn bottom(&self) -> f32 {
        self.margin
    }

    /// Make room for `h` points, starting a page when the current one is full.
    fn need(&mut self, h: f32) {
        if self.pages.is_empty() || self.y - h < self.bottom() {
            self.new_page();
        }
    }

    fn at_top(&self) -> bool {
        self.pages.is_empty() || (self.y - (self.height - self.margin)).abs() < 0.01
    }

    fn ops(&mut self) -> &mut Vec<u8> {
        if self.pages.is_empty() {
            self.new_page();
        }
        &mut self.pages.last_mut().unwrap().1
    }

    fn draw(&mut self, line: &Line, x: f32, baseline: f32, size: f32, style: Style) -> Result<()> {
        if line.glyphs.is_empty() {
            return Ok(());
        }
        if self.pages.is_empty() {
            self.new_page();
        }
        let page = self.pages.last().unwrap().0;
        let mut ops = vec![];
        if style.link {
            ops.extend(b"0.05 0.3 0.65 rg\n");
        }
        ops.extend(self.ts.draw(&mut self.doc, page, line, x, baseline, size)?);
        if style.link {
            ops.extend(b"0 g\n");
        }
        self.ops().extend(ops);
        Ok(())
    }

    fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, fill_gray: f32) {
        let s = format!(
            "q {} g {} {} {} {} re f Q\n",
            n(fill_gray),
            n(x),
            n(y),
            n(w),
            n(h)
        );
        self.ops().extend(s.into_bytes());
    }

    fn hline(&mut self, x0: f32, x1: f32, y: f32, width: f32) {
        let s = format!(
            "q {GRAY_RULE} {} w {} {} m {} {} l S Q\n",
            n(width),
            n(x0),
            n(y),
            n(x1),
            n(y)
        );
        self.ops().extend(s.into_bytes());
    }

    /// Split pieces into measured words.
    fn words(&mut self, pieces: &[Piece], size: f32) -> Result<Vec<Word>> {
        let mut out: Vec<Word> = vec![];
        for p in pieces {
            if p.text == "\n" {
                out.push(Word {
                    line: Line::default(),
                    size,
                    style: p.style,
                    space: 0.0,
                    brk: true,
                });
                continue;
            }
            let space = self.measure(" ", p.style, size)?.width;
            let text = keep_together(&p.text.replace(['\t', '\n'], " "));
            let starts_with_space = text.starts_with(' ');
            if starts_with_space && let Some(last) = out.last_mut() {
                last.space = last.space.max(space);
            }
            let parts: Vec<&str> = text.split(' ').filter(|w| !w.is_empty()).collect();
            let ends_with_space = text.ends_with(' ');
            for (i, w) in parts.iter().enumerate() {
                let line = self.measure(w, p.style, size)?;
                let gap = if i + 1 < parts.len() || ends_with_space {
                    space
                } else {
                    0.0
                };
                out.push(Word {
                    line,
                    size,
                    style: p.style,
                    space: gap,
                    brk: false,
                });
            }
        }
        Ok(out)
    }

    /// Cut a word wider than `width` into pieces that fit.
    fn split_word(&mut self, w: &Word, width: f32) -> Result<Vec<Word>> {
        let text: String = w.line.glyphs.iter().map(|g| g.text.as_str()).collect();
        let mut out = vec![];
        let mut cur = String::new();
        for c in text.chars() {
            let mut trial = cur.clone();
            trial.push(c);
            if !cur.is_empty() && self.measure(&trial, w.style, w.size)?.width > width {
                let line = self.measure(&cur, w.style, w.size)?;
                out.push(Word {
                    line,
                    size: w.size,
                    style: w.style,
                    space: 0.0,
                    brk: false,
                });
                cur = c.to_string();
            } else {
                cur = trial;
            }
        }
        let line = self.measure(&cur, w.style, w.size)?;
        out.push(Word {
            line,
            size: w.size,
            style: w.style,
            space: w.space,
            brk: false,
        });
        Ok(out)
    }

    /// Break words into lines no wider than `width`.
    fn lines(&mut self, words: Vec<Word>, width: f32) -> Result<Vec<Vec<Word>>> {
        let mut lines: Vec<Vec<Word>> = vec![vec![]];
        let mut x = 0.0;
        for w in words {
            if w.brk {
                lines.push(vec![]);
                x = 0.0;
                continue;
            }
            let pieces = if w.line.width > width {
                self.split_word(&w, width)?
            } else {
                vec![w]
            };
            for w in pieces {
                let cur = lines.last_mut().unwrap();
                if !cur.is_empty() && x + w.line.width > width {
                    lines.push(vec![]);
                    x = 0.0;
                }
                x += w.line.width + w.space;
                lines.last_mut().unwrap().push(w);
            }
        }
        if lines.len() > 1 && lines.last().is_some_and(Vec::is_empty) {
            lines.pop();
        }
        Ok(lines)
    }

    fn line_width(line: &[Word]) -> f32 {
        let mut w = 0.0;
        for (i, word) in line.iter().enumerate() {
            w += word.line.width;
            if i + 1 < line.len() {
                w += word.space;
            }
        }
        w
    }

    fn draw_words(&mut self, line: &[Word], x: f32, baseline: f32) -> Result<()> {
        let mut pen = x;
        for w in line {
            self.draw(&w.line, pen, baseline, w.size, w.style)?;
            pen += w.line.width + w.space;
        }
        Ok(())
    }

    /// Flow a paragraph at `x`, `width` wide. `marker` is drawn in the margin
    /// of the first line (list bullets); `bar` draws a quote bar at that x.
    fn paragraph(
        &mut self,
        pieces: &[Piece],
        size: f32,
        x: f32,
        width: f32,
        marker: Option<(String, f32)>,
        bars: &[f32],
    ) -> Result<()> {
        let words = self.words(pieces, size)?;
        let lines = self.lines(words, width)?;
        let lh = size * LINE_GAP;
        for (i, line) in lines.iter().enumerate() {
            self.need(lh);
            let baseline = self.y - size;
            if i == 0
                && let Some((m, mx)) = &marker
            {
                let ml = self.measure(m, Style::default(), size)?;
                self.draw(&ml, *mx, baseline, size, Style::default())?;
            }
            for &bx in bars {
                let top = self.y;
                self.rect(bx, top - lh, 2.0, lh, 0.78);
            }
            self.draw_words(line, x, baseline)?;
            self.y -= lh;
        }
        Ok(())
    }

    fn heading(&mut self, pieces: &[Piece], level: usize, x: f32, width: f32) -> Result<()> {
        let k = [1.9, 1.5, 1.25, 1.1, 1.0, 0.95][level.min(6) - 1];
        let size = self.body * k;
        let bold: Vec<Piece> = pieces
            .iter()
            .map(|p| Piece {
                text: p.text.clone(),
                style: Style {
                    bold: true,
                    ..p.style
                },
            })
            .collect();
        let words = self.words(&bold, size)?;
        let lines = self.lines(words, width)?;
        let lh = size * 1.25;
        // Keep the heading with at least two lines of what follows.
        let before = if self.at_top() { 0.0 } else { size * 0.8 };
        self.need(before + lh * lines.len() as f32 + self.body * LINE_GAP * 2.0);
        if !self.at_top() {
            self.y -= before;
        }
        let title: String = pieces
            .iter()
            .map(|p| p.text.as_str())
            .collect::<String>()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        if level <= 3 && !title.is_empty() {
            self.outline.push((title, self.pages.len() - 1, self.y));
        }
        for line in &lines {
            let baseline = self.y - size;
            self.draw_words(line, x, baseline)?;
            self.y -= lh;
        }
        if level <= 2 {
            let y = self.y + size * 0.1;
            self.hline(x, x + width, y, if level == 1 { 0.8 } else { 0.4 });
            self.y -= size * 0.15;
        }
        self.y -= size * 0.35;
        Ok(())
    }

    fn code_block(&mut self, text: &str, x: f32, width: f32) -> Result<()> {
        let size = self.body * 0.88;
        let lh = size * 1.3;
        let pad = 4.0;
        let style = Style {
            mono: true,
            ..Style::default()
        };
        self.need(pad + lh);
        self.y -= pad * 0.5;
        for raw in text.trim_end_matches('\n').split('\n') {
            let line = raw.replace('\t', "    ");
            // Keep leading spaces: lay out the whole line, then wrap by characters.
            let measured = self.measure(&line, style, size)?;
            let chunks = if measured.width > width - 2.0 * pad {
                let w = Word {
                    line: measured,
                    size,
                    style,
                    space: 0.0,
                    brk: false,
                };
                self.split_word(&w, width - 2.0 * pad)?
            } else {
                vec![Word {
                    line: measured,
                    size,
                    style,
                    space: 0.0,
                    brk: false,
                }]
            };
            for c in chunks {
                self.need(lh);
                self.rect(x, self.y - lh, width, lh, 0.95);
                let baseline = self.y - size * 1.05;
                self.draw(&c.line, x + pad, baseline, size, style)?;
                self.y -= lh;
            }
        }
        self.y -= pad + self.body * 0.4;
        Ok(())
    }

    fn table(&mut self, t: &Table, x: f32, width: f32) -> Result<()> {
        let cols = t.rows.iter().map(Vec::len).max().unwrap_or(0);
        if cols == 0 {
            return Ok(());
        }
        let size = self.body * 0.88;
        let pad = 3.5;
        let lh = size * 1.3;
        // Natural (one line) and minimum (longest word) widths per column.
        let mut natural = vec![0.0f32; cols];
        let mut minimum = vec![0.0f32; cols];
        for (r, row) in t.rows.iter().enumerate() {
            for (c, cell) in row.iter().enumerate() {
                let style_bold = r == 0 && t.header;
                let cell = bolded(cell, style_bold);
                let words = self.words(&cell, size)?;
                let total = Self::line_width(&words) + 2.0 * pad;
                let longest = words.iter().map(|w| w.line.width).fold(0.0, f32::max) + 2.0 * pad;
                natural[c] = natural[c].max(total);
                minimum[c] = minimum[c].max(longest.min(width * 0.35));
            }
        }
        let widths = column_widths(&natural, &minimum, width);
        for (r, row) in t.rows.iter().enumerate() {
            let is_header = r == 0 && t.header;
            let mut cells: Vec<Vec<Vec<Word>>> = vec![];
            for (c, w) in widths.iter().enumerate() {
                let cell = row.get(c).cloned().unwrap_or_default();
                let words = self.words(&bolded(&cell, is_header), size)?;
                cells.push(self.lines(words, (w - 2.0 * pad).max(4.0))?);
            }
            let rows_h =
                cells.iter().map(|l| l.len().max(1)).max().unwrap_or(1) as f32 * lh + 2.0 * pad;
            let page_h = self.height - 2.0 * self.margin;
            if self.y - rows_h < self.bottom() && !self.at_top() {
                self.new_page();
                if t.header && !is_header && rows_h < page_h {
                    self.table_row(&t.rows[0], true, &widths, x, size, pad, lh)?;
                }
            }
            self.row_lines(&cells, is_header, &t.align, &widths, x, size, pad, lh)?;
        }
        self.y -= self.body * 0.6;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn table_row(
        &mut self,
        row: &[Vec<Piece>],
        header: bool,
        widths: &[f32],
        x: f32,
        size: f32,
        pad: f32,
        lh: f32,
    ) -> Result<()> {
        let mut cells = vec![];
        for (c, w) in widths.iter().enumerate() {
            let cell = row.get(c).cloned().unwrap_or_default();
            let words = self.words(&bolded(&cell, header), size)?;
            cells.push(self.lines(words, (w - 2.0 * pad).max(4.0))?);
        }
        self.row_lines(&cells, header, &[], widths, x, size, pad, lh)
    }

    /// Draw one table row; a row taller than the page continues on the next.
    #[allow(clippy::too_many_arguments)]
    fn row_lines(
        &mut self,
        cells: &[Vec<Vec<Word>>],
        header: bool,
        align: &[Alignment],
        widths: &[f32],
        x: f32,
        size: f32,
        pad: f32,
        lh: f32,
    ) -> Result<()> {
        let total: f32 = widths.iter().sum();
        let rows = cells.iter().map(|l| l.len().max(1)).max().unwrap_or(1);
        let mut i = 0;
        while i < rows {
            // As many lines of the row as fit on this page.
            let room = ((self.y - pad * 2.0 - self.bottom()) / lh).floor().max(0.0) as usize;
            let take = if room == 0 {
                self.new_page();
                continue;
            } else {
                room.min(rows - i)
            };
            let h = take as f32 * lh + 2.0 * pad;
            let top = self.y;
            if header {
                self.rect(x, top - h, total, h, 0.91);
            }
            let mut cx = x;
            for (c, w) in widths.iter().enumerate() {
                let lines = &cells[c];
                for (j, line) in lines.iter().enumerate().skip(i).take(take) {
                    let baseline = top - pad - (j - i) as f32 * lh - size;
                    let lw = Self::line_width(line);
                    let dx = match align.get(c) {
                        Some(Alignment::Right) => (w - 2.0 * pad - lw).max(0.0),
                        Some(Alignment::Center) => ((w - 2.0 * pad - lw) / 2.0).max(0.0),
                        _ => 0.0,
                    };
                    self.draw_words(line, cx + pad + dx, baseline)?;
                }
                cx += w;
            }
            // Grid: the box of each cell.
            let mut s = format!("q {GRAY_RULE} 0.5 w ");
            let mut cx = x;
            for w in widths {
                s.push_str(&format!("{} {} {} {} re ", n(cx), n(top - h), n(*w), n(h)));
                cx += w;
            }
            s.push_str("S Q\n");
            self.ops().extend(s.into_bytes());
            self.y -= h;
            i += take;
            if i < rows {
                self.new_page();
            }
        }
        Ok(())
    }

    fn finish(mut self, title: &str, page_numbers: bool) -> Result<(Vec<u8>, MdPdfReport)> {
        if self.pages.is_empty() {
            self.new_page();
        }
        let total = self.pages.len();
        if page_numbers {
            for i in 0..total {
                let label = format!("{} / {total}", i + 1);
                let size = 8.0;
                let line = self.measure(&label, Style::default(), size)?;
                let x = (self.width - line.width) / 2.0;
                let page = self.pages[i].0;
                let mut ops = b"0.45 g\n".to_vec();
                ops.extend(self.ts.draw(
                    &mut self.doc,
                    page,
                    &line,
                    x,
                    self.margin * 0.45,
                    size,
                )?);
                ops.extend(b"0 g\n");
                self.pages[i].1.extend(ops);
            }
        }
        let mut kids = vec![];
        for (page, ops) in std::mem::take(&mut self.pages) {
            let mut s = Stream::new(Dictionary::new(), ops);
            let _ = s.compress();
            let content = self.doc.add_object(s);
            self.doc.get_dictionary_mut(page)?.set("Contents", content);
            kids.push(Object::Reference(page));
        }
        let mut doc = self.doc;
        self.ts.finish(&mut doc)?;
        doc.objects.insert(
            self.pages_id,
            dictionary! { "Type" => "Pages", "Kids" => kids.clone(), "Count" => total as i64 }
                .into(),
        );
        let mut catalog = dictionary! { "Type" => "Catalog", "Pages" => self.pages_id };
        if !self.outline.is_empty() {
            let outlines = doc.new_object_id();
            let ids: Vec<ObjectId> = self.outline.iter().map(|_| doc.new_object_id()).collect();
            for (i, (t, page, y)) in self.outline.iter().enumerate() {
                let mut d = dictionary! {
                    "Title" => crate::meta::encode_text(t),
                    "Parent" => outlines,
                    "Dest" => vec![kids[*page].clone(), "XYZ".into(), 0.into(), (*y).into(), Object::Null],
                };
                if i > 0 {
                    d.set("Prev", ids[i - 1]);
                }
                if i + 1 < ids.len() {
                    d.set("Next", ids[i + 1]);
                }
                doc.objects.insert(ids[i], d.into());
            }
            doc.objects.insert(
                outlines,
                dictionary! { "Type" => "Outlines", "First" => ids[0], "Last" => *ids.last().unwrap(), "Count" => ids.len() as i64 }.into(),
            );
            catalog.set("Outlines", outlines);
            catalog.set("PageMode", "UseOutlines");
        }
        let root = doc.add_object(catalog);
        doc.trailer.set("Root", root);
        let mut info = dictionary! { "Producer" => "pdfvault" };
        if !title.is_empty() {
            info.set("Title", crate::meta::encode_text(title));
        }
        let info = doc.add_object(info);
        doc.trailer.set("Info", info);
        let report = MdPdfReport {
            pages: total,
            missing: self.missing.iter().collect(),
            replaced: self.replaced,
        };
        Ok((pdf::save(&mut doc)?, report))
    }
}

/// Spaces a line must not break at become no-break spaces: inside numbers
/// (`32 792,80`), before a currency or percent sign (`5 €`, `12 %`), and
/// around French guillemets and high punctuation (`« oui »`, `note :`).
fn keep_together(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    for (i, &c) in chars.iter().enumerate() {
        if c != ' ' {
            out.push(c);
            continue;
        }
        let prev = i.checked_sub(1).map(|j| chars[j]);
        let next = chars.get(i + 1).copied();
        let number = matches!((prev, next), (Some(p), Some(nx)) if p.is_ascii_digit() && (nx.is_ascii_digit() || "€%$£".contains(nx)));
        let glue = number || prev == Some('«') || matches!(next, Some('»' | ':' | ';' | '!' | '?'));
        out.push(if glue { '\u{a0}' } else { ' ' });
    }
    out
}

/// What to draw instead of a character the fonts lack, best first.
fn stand_ins(c: char) -> &'static [&'static str] {
    match c {
        '✓' | '✔' | '☑' => &["√", "v"],
        '✗' | '✘' | '☒' => &["×", "x"],
        '☐' | '□' | '▢' => &["[ ]"],
        '■' | '▪' | '●' => &["•"],
        '→' | '⇒' | '➜' | '➔' | '⟶' => &["→", "->"],
        '←' | '⇐' | '⟵' => &["←", "<-"],
        '↔' | '⇔' => &["↔", "<->"],
        '…' => &["…", "..."],
        '−' => &["−", "-"],
        '≥' => &["≥", ">="],
        '≤' => &["≤", "<="],
        '≠' => &["≠", "!="],
        '\u{202f}' | '\u{2009}' | '\u{2007}' => &["\u{a0}", " "],
        '\u{200b}' | '\u{feff}' | '\u{ad}' => &[""],
        _ => &[],
    }
}

fn bolded(cell: &[Piece], bold: bool) -> Vec<Piece> {
    cell.iter()
        .map(|p| Piece {
            text: p.text.clone(),
            style: Style {
                bold: p.style.bold || bold,
                ..p.style
            },
        })
        .collect()
}

/// Share `width` between columns: everyone gets its natural width when it
/// fits; otherwise each gets its minimum and the rest goes to the columns
/// that need it most.
fn column_widths(natural: &[f32], minimum: &[f32], width: f32) -> Vec<f32> {
    let sum_nat: f32 = natural.iter().sum();
    if sum_nat <= width {
        return natural.to_vec();
    }
    let sum_min: f32 = minimum.iter().sum();
    if sum_min >= width {
        return minimum.iter().map(|m| m * width / sum_min).collect();
    }
    let extra = width - sum_min;
    let want: Vec<f32> = natural
        .iter()
        .zip(minimum)
        .map(|(n, m)| (n - m).max(0.0))
        .collect();
    let sum_want: f32 = want.iter().sum::<f32>().max(1.0);
    minimum
        .iter()
        .zip(&want)
        .map(|(m, w)| m + extra * w / sum_want)
        .collect()
}

#[derive(Default)]
struct Table {
    rows: Vec<Vec<Vec<Piece>>>,
    align: Vec<Alignment>,
    header: bool,
}

enum Block {
    Heading(usize),
    Paragraph,
    Item,
}

struct ListState {
    ordered: bool,
    next: u64,
}

/// Typeset `md` (UTF-8) with the fonts in `fonts`, keyed as in [`MdPdfOptions::bundled`].
pub fn md2pdf(md: &[u8], fonts: &[Vec<u8>], o: &MdPdfOptions) -> Result<(Vec<u8>, MdPdfReport)> {
    let text = String::from_utf8_lossy(md);
    let text = text.strip_prefix('\u{FEFF}').unwrap_or(&text);
    let (mut w, mut h): (f32, f32) = match o.page_size.as_str() {
        "a4" => (595.28, 841.89),
        "letter" => (612.0, 792.0),
        other => return Err(Error::option(format!("unknown page size '{other}'"))),
    };
    if !(4.0..=40.0).contains(&o.font_size) {
        return Err(Error::option("font size must be between 4 and 40 points"));
    }
    if !(0.0..=200.0).contains(&o.margin) || o.margin * 2.0 > w.min(h) - 100.0 {
        return Err(Error::option("the margin leaves no room for text"));
    }
    let family = match o.font.as_str() {
        "sans" | "serif" => o.font.as_str(),
        other => return Err(Error::option(format!("unknown font '{other}'"))),
    };
    let font = |key: &str| -> Result<&Vec<u8>> {
        let i = *o
            .bundled
            .get(key)
            .ok_or_else(|| Error::option(format!("the bundled font {key} was not given")))?;
        fonts
            .get(i)
            .ok_or_else(|| Error::option(format!("font file {} was not given", i + 1)))
    };
    let mut ts = Typesetter::new();
    let mut add = |key: &str| -> Result<usize> { ts.add_face(font(key)?, 0) };
    let faces = Faces {
        regular: add(&format!("{family}-regular"))?,
        bold: add(&format!("{family}-bold"))?,
        italic: add(&format!("{family}-italic"))?,
        bolditalic: add(&format!("{family}-bolditalic"))?,
        mono: add("mono-regular")?,
        mono_bold: add("mono-bold")?,
    };
    if w > h {
        std::mem::swap(&mut w, &mut h);
    }
    let mut doc = Document::with_version("1.7");
    let pages_id = doc.new_object_id();
    let mut wr = Writer {
        ts,
        faces,
        doc,
        pages_id,
        pages: vec![],
        width: w,
        height: h,
        margin: o.margin,
        body: o.font_size,
        y: h - o.margin,
        missing: vec![],
        replaced: 0,
        outline: vec![],
    };
    let full = w - 2.0 * o.margin;
    let mut style = Style::default();
    let mut inline: Vec<Piece> = vec![];
    let mut blocks: Vec<Block> = vec![];
    let mut lists: Vec<ListState> = vec![];
    let mut quotes = 0usize;
    let mut marker: Option<String> = None;
    let mut code: Option<String> = None;
    let mut table: Option<Table> = None;
    let mut cell: Vec<Piece> = vec![];
    let mut first_heading = String::new();

    let indent =
        |lists: &Vec<ListState>, quotes: usize| 18.0 * lists.len() as f32 + 14.0 * quotes as f32;
    let push = |inline: &mut Vec<Piece>,
                cell: &mut Vec<Piece>,
                table: &Option<Table>,
                text: String,
                style: Style| {
        let target = if table.is_some() { cell } else { inline };
        match target.last_mut() {
            Some(p) if p.style == style && p.text != "\n" && text != "\n" => p.text.push_str(&text),
            _ => target.push(Piece { text, style }),
        }
    };

    // Lay out what has accumulated in `inline` as a paragraph of the current block.
    macro_rules! flush {
        () => {{
            if inline
                .iter()
                .any(|p| !p.text.trim().is_empty() || p.text == "\n")
            {
                let ind = indent(&lists, quotes);
                let bars: Vec<f32> = (0..quotes).map(|q| o.margin + 14.0 * q as f32).collect();
                let m = marker.take().map(|m| (m, o.margin + ind - 14.0));
                wr.paragraph(&inline, wr.body, o.margin + ind, full - ind, m, &bars)?;
                wr.y -= if lists.is_empty() {
                    wr.body * 0.55
                } else {
                    wr.body * 0.15
                };
            }
            inline.clear();
        }};
    }

    let opts = Md::ENABLE_TABLES | Md::ENABLE_STRIKETHROUGH | Md::ENABLE_TASKLISTS;
    for ev in Parser::new_ext(text, opts) {
        match ev {
            Event::Start(tag) => match tag {
                Tag::Heading { level, .. } => {
                    flush!();
                    blocks.push(Block::Heading(match level {
                        HeadingLevel::H1 => 1,
                        HeadingLevel::H2 => 2,
                        HeadingLevel::H3 => 3,
                        HeadingLevel::H4 => 4,
                        HeadingLevel::H5 => 5,
                        HeadingLevel::H6 => 6,
                    }));
                }
                Tag::Paragraph => blocks.push(Block::Paragraph),
                Tag::BlockQuote(_) => {
                    flush!();
                    quotes += 1;
                }
                Tag::CodeBlock(_) => {
                    flush!();
                    code = Some(String::new());
                }
                Tag::List(start) => {
                    flush!();
                    lists.push(ListState {
                        ordered: start.is_some(),
                        next: start.unwrap_or(1),
                    });
                }
                Tag::Item => {
                    flush!();
                    blocks.push(Block::Item);
                    if let Some(l) = lists.last_mut() {
                        marker = Some(if l.ordered {
                            let m = format!("{}.", l.next);
                            l.next += 1;
                            m
                        } else {
                            ["•", "–", "·"][(lists.len() - 1) % 3].to_string()
                        });
                    }
                }
                Tag::Table(align) => {
                    flush!();
                    table = Some(Table {
                        align,
                        ..Table::default()
                    });
                }
                Tag::TableHead => {
                    if let Some(t) = table.as_mut() {
                        t.header = true;
                        t.rows.push(vec![]);
                    }
                }
                Tag::TableRow => {
                    if let Some(t) = table.as_mut() {
                        t.rows.push(vec![]);
                    }
                }
                Tag::TableCell => cell.clear(),
                Tag::Emphasis => style.italic = true,
                Tag::Strong => style.bold = true,
                Tag::Link { .. } => style.link = true,
                Tag::Image { .. } => {
                    push(
                        &mut inline,
                        &mut cell,
                        &table,
                        "[image : ".into(),
                        Style {
                            italic: true,
                            ..style
                        },
                    );
                    style.italic = true;
                }
                _ => {}
            },
            Event::End(tag) => match tag {
                TagEnd::Heading(_) => {
                    if let Some(Block::Heading(level)) = blocks.pop() {
                        if first_heading.is_empty() {
                            first_heading = inline
                                .iter()
                                .map(|p| p.text.as_str())
                                .collect::<String>()
                                .trim()
                                .to_string();
                        }
                        let ind = indent(&lists, quotes);
                        wr.heading(&inline, level, o.margin + ind, full - ind)?;
                        inline.clear();
                    }
                }
                TagEnd::Paragraph => {
                    blocks.pop();
                    flush!();
                }
                TagEnd::BlockQuote(_) => {
                    flush!();
                    quotes = quotes.saturating_sub(1);
                }
                TagEnd::CodeBlock => {
                    if let Some(c) = code.take() {
                        let ind = indent(&lists, quotes);
                        wr.code_block(&c, o.margin + ind, full - ind)?;
                    }
                }
                TagEnd::List(_) => {
                    flush!();
                    lists.pop();
                    if lists.is_empty() {
                        wr.y -= wr.body * 0.4;
                    }
                }
                TagEnd::Item => {
                    flush!();
                    blocks.pop();
                    marker = None;
                }
                TagEnd::Table => {
                    if let Some(mut t) = table.take() {
                        // `| | |` headers only exist to satisfy the syntax: drop them.
                        let empty = |row: &Vec<Vec<Piece>>| {
                            row.iter()
                                .all(|c| c.iter().all(|p| p.text.trim().is_empty()))
                        };
                        if t.header && t.rows.first().is_some_and(empty) {
                            t.rows.remove(0);
                            t.header = false;
                        }
                        let ind = indent(&lists, quotes);
                        wr.table(&t, o.margin + ind, full - ind)?;
                    }
                }
                TagEnd::TableCell => {
                    if let Some(t) = table.as_mut()
                        && let Some(row) = t.rows.last_mut()
                    {
                        row.push(std::mem::take(&mut cell));
                    }
                }
                TagEnd::Emphasis => style.italic = false,
                TagEnd::Strong => style.bold = false,
                TagEnd::Link => style.link = false,
                TagEnd::Image => {
                    style.italic = false;
                    push(
                        &mut inline,
                        &mut cell,
                        &table,
                        "]".into(),
                        Style {
                            italic: true,
                            ..style
                        },
                    );
                }
                _ => {}
            },
            Event::Text(t) => {
                if let Some(c) = code.as_mut() {
                    c.push_str(&t);
                } else {
                    push(&mut inline, &mut cell, &table, t.to_string(), style);
                }
            }
            Event::Code(t) => push(
                &mut inline,
                &mut cell,
                &table,
                t.to_string(),
                Style {
                    mono: true,
                    ..style
                },
            ),
            Event::SoftBreak => push(&mut inline, &mut cell, &table, " ".into(), style),
            Event::HardBreak => push(&mut inline, &mut cell, &table, "\n".into(), style),
            Event::Rule => {
                flush!();
                wr.need(wr.body);
                let y = wr.y - wr.body * 0.5;
                wr.hline(o.margin, o.margin + full, y, 0.6);
                wr.y -= wr.body;
            }
            Event::TaskListMarker(done) => push(
                &mut inline,
                &mut cell,
                &table,
                if done { "[x] ".into() } else { "[ ] ".into() },
                Style {
                    mono: true,
                    ..style
                },
            ),
            Event::FootnoteReference(r) => {
                push(&mut inline, &mut cell, &table, format!("[{r}]"), style)
            }
            // Raw HTML (comments, tags) has no place on the page.
            Event::Html(_) | Event::InlineHtml(_) => {}
            _ => {}
        }
    }
    flush!();
    let title = if o.title.trim().is_empty() {
        first_heading
    } else {
        o.title.trim().to_string()
    };
    wr.finish(&title, o.page_numbers)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures;
    use crate::interp;
    use crate::testutil::qpdf_check;

    fn fonts() -> (Vec<Vec<u8>>, BTreeMap<String, usize>) {
        let keys = [
            ("sans-regular", "LiberationSans-Regular.ttf"),
            ("sans-bold", "LiberationSans-Bold.ttf"),
            ("sans-italic", "LiberationSans-Italic.ttf"),
            ("sans-bolditalic", "LiberationSans-BoldItalic.ttf"),
            ("mono-regular", "LiberationMono-Regular.ttf"),
            ("mono-bold", "LiberationMono-Bold.ttf"),
        ];
        let files = keys
            .iter()
            .map(|(_, f)| fixtures::bundled_font(f))
            .collect();
        let map = keys
            .iter()
            .enumerate()
            .map(|(i, (k, _))| (k.to_string(), i))
            .collect();
        (files, map)
    }

    fn text_of(pdf_bytes: &[u8]) -> Vec<String> {
        let doc = pdf::load(pdf_bytes, None).unwrap();
        pdf::page_ids(&doc)
            .into_iter()
            .map(|p| {
                let page = interp::interpret(&doc, p);
                page.glyphs
                    .iter()
                    .filter_map(|g| g.text.clone())
                    .collect::<String>()
            })
            .collect()
    }

    #[test]
    fn typesets_headings_lists_tables_and_code() {
        let (files, bundled) = fonts();
        let md = "<!-- note -->\n# Dossier GOLFIER\n\nLe prêt de **154 312 €** a été *accepté* le 22/01/2011, voir [la note](<a b.md>).\n\n\
                  - premier\n- second\n  1. sous-point\n\n> une citation\n\n| Date | Montant |\n|---|---:|\n| 2011-04-05 | 534,95 |\n| 2013-11-05 | 158 050,06 |\n\n\
                  ```\nlet x = 1;\n```\n\n---\n\nFin ✓ → 世界\n";
        let o = MdPdfOptions {
            bundled,
            ..MdPdfOptions::default()
        };
        let (out, report) = md2pdf(md.as_bytes(), &files, &o).unwrap();
        qpdf_check(&out);
        assert_eq!(report.pages, 1);
        assert_eq!(report.missing, "世界");
        assert_eq!(report.replaced, 2);
        let text = text_of(&out).concat().replace([' ', '\u{a0}'], "");
        for want in [
            "√",
            "→",
            "DossierGOLFIER",
            "154312€",
            "accepté",
            "lanote",
            "•",
            "premier",
            "1.",
            "sous-point",
            "unecitation",
            "534,95",
            "158050,06",
            "letx=1;",
            "Fin",
            "1/1",
        ] {
            assert!(text.contains(want), "{want} missing from {text}");
        }
        assert!(!text.contains("note -->"), "HTML comments are not drawn");
        let doc = pdf::load(&out, None).unwrap();
        let root = doc.trailer.get(b"Root").unwrap().as_reference().unwrap();
        assert!(doc.get_dictionary(root).unwrap().has(b"Outlines"));
        let info = doc.trailer.get(b"Info").unwrap().as_reference().unwrap();
        assert!(doc.get_dictionary(info).unwrap().has(b"Title"));
    }

    #[test]
    fn empty_table_headers_are_dropped() {
        let (files, bundled) = fonts();
        let o = MdPdfOptions {
            bundled,
            ..MdPdfOptions::default()
        };
        let with = md2pdf(b"| | |\n|---|---|\n| Type | Acte |\n", &files, &o)
            .unwrap()
            .0;
        let without = md2pdf(b"| Type | Acte |\n|---|---|\n", &files, &o)
            .unwrap()
            .0;
        // The same single row either way: no grey header band.
        let ops = |b: &[u8]| {
            let doc = pdf::load(b, None).unwrap();
            let page = pdf::page_ids(&doc)[0];
            String::from_utf8_lossy(&doc.get_page_content(page))
                .matches(" re f")
                .count()
        };
        assert_eq!(ops(&with), 0);
        assert_eq!(ops(&without), 1, "a real header is shaded");
    }

    #[test]
    fn numbers_and_amounts_do_not_break() {
        assert_eq!(
            keep_together("payé 32 792,80 € « oui » : 12 %"),
            "payé 32\u{a0}792,80\u{a0}€ «\u{a0}oui\u{a0}»\u{a0}: 12\u{a0}%"
        );
        assert_eq!(keep_together("le 15 octobre 2011"), "le 15 octobre 2011");
    }

    #[test]
    fn long_documents_and_tables_break_across_pages() {
        let (files, bundled) = fonts();
        let mut md = String::from("# Long\n\n| N | Libellé |\n|---|---|\n");
        for i in 0..150 {
            md.push_str(&format!("| {i} | ligne numéro {i} avec un libellé assez long pour remplir la cellule du tableau |\n"));
        }
        md.push('\n');
        md.push_str(&"mot ".repeat(4000));
        md.push_str(&"\n\nhttps://exemple.fr/un/tres/long/chemin/sans/espace/".repeat(3));
        let o = MdPdfOptions {
            bundled,
            page_numbers: false,
            ..MdPdfOptions::default()
        };
        let (out, report) = md2pdf(md.as_bytes(), &files, &o).unwrap();
        qpdf_check(&out);
        assert!(report.pages >= 6, "{} pages", report.pages);
        let pages = text_of(&out);
        assert_eq!(pages.len(), report.pages);
        // The header row is repeated at the top of the table's next page.
        let second = pages.iter().skip(1).find(|p| p.contains("ligne")).unwrap();
        assert!(second.contains("Libellé"));
        let all = pages.concat();
        assert!(all.contains("149"));
        assert!(all.matches("mot").count() >= 4000);
    }

    #[test]
    fn rejects_bad_options_and_missing_fonts() {
        let (files, bundled) = fonts();
        let bad = MdPdfOptions {
            page_size: "a3".into(),
            bundled: bundled.clone(),
            ..MdPdfOptions::default()
        };
        assert!(md2pdf(b"x", &files, &bad).is_err());
        let no_fonts = MdPdfOptions::default();
        assert!(md2pdf(b"x", &[], &no_fonts).is_err());
        let serif = MdPdfOptions {
            font: "serif".into(),
            bundled,
            ..MdPdfOptions::default()
        };
        assert!(
            md2pdf(b"x", &files, &serif).is_err(),
            "serif faces were not given"
        );
        let o = MdPdfOptions {
            bundled: fonts().1,
            ..MdPdfOptions::default()
        };
        let (out, r) = md2pdf(b"", &files, &o).unwrap();
        qpdf_check(&out);
        assert_eq!(r.pages, 1);
    }
}
