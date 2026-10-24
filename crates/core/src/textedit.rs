//! Existing text as the editor offers it: lines of one font, size and colour
//! that a click can turn into an editable box, and how a changed line is
//! written back (in its own font when that font has every character, or in
//! the closest bundled font otherwise).

use crate::interp::{self, Page};
use crate::pdf;
use crate::pdffont::{Family, Font, Kind};
use lopdf::Document;
use serde::Serialize;
use std::collections::HashMap;

/// One line of existing text, in visual page space.
#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TextRun {
    pub page: usize,
    /// Box around the glyphs: bottom-left corner, width and height.
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    /// Where the first glyph sits on its baseline.
    pub start: f32,
    pub baseline: f32,
    pub size: f32,
    pub text: String,
    pub family: Family,
    pub bold: bool,
    pub italic: bool,
    pub color: String,
    /// The PDF font's name, for display.
    pub font_name: String,
    pub hscale: f32,
    /// The text is known for every glyph and the line runs left to right.
    pub editable: bool,
}

fn hex_color(c: [f32; 3]) -> String {
    let b = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("#{:02x}{:02x}{:02x}", b(c[0]), b(c[1]), b(c[2]))
}

/// Group a page's visible glyphs into lines.
pub fn runs_of(page: &Page, index: usize) -> Vec<TextRun> {
    let mut runs: Vec<(TextRun, Vec<usize>)> = vec![];
    let mut last: Option<usize> = None;
    for (i, g) in page.glyphs.iter().enumerate() {
        // Invisible text (OCR layers, clipping-only modes) is not offered.
        if g.mode == 3 || g.mode == 7 || g.size < 0.5 {
            continue;
        }
        let joins = last.is_some_and(|l| {
            let p = &page.glyphs[l];
            let gap = g.origin[0] - p.end[0];
            p.font == g.font
                && (p.size - g.size).abs() <= g.size * 0.02
                && p.color == g.color
                && (p.origin[1] - g.origin[1]).abs() <= g.size * 0.1
                && (p.angle - g.angle).abs() < 0.5
                && gap > -g.size * 0.3
                && gap < g.size * 1.2
        });
        let text = g.text.clone();
        if joins && let Some((run, members)) = runs.last_mut() {
            let p = &page.glyphs[*members.last().unwrap()];
            let gap = g.origin[0] - p.end[0];
            let t = text.as_deref().unwrap_or("\u{fffd}");
            if gap > g.size * 0.18 && !run.text.ends_with(' ') && !t.starts_with(' ') {
                run.text.push(' ');
            }
            run.text.push_str(t);
            run.editable &= text.is_some();
            members.push(i);
        } else {
            let f = &page.fonts[g.font];
            runs.push((
                TextRun {
                    page: index,
                    x: 0.0,
                    y: 0.0,
                    w: 0.0,
                    h: 0.0,
                    start: g.origin[0],
                    baseline: g.origin[1],
                    size: g.size,
                    text: text.clone().unwrap_or_else(|| "\u{fffd}".into()),
                    family: f.family,
                    bold: f.bold,
                    italic: f.italic,
                    color: hex_color(g.color),
                    font_name: f.base.clone(),
                    hscale: g.hscale,
                    editable: text.is_some() && g.angle.abs() < 0.5 && !f.vertical(),
                },
                vec![i],
            ));
        }
        last = Some(i);
    }
    runs.into_iter()
        .filter_map(|(mut run, members)| {
            let mut b = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
            for &m in &members {
                let gb = page.glyphs[m].bbox;
                b = [
                    b[0].min(gb[0]),
                    b[1].min(gb[1]),
                    b[2].max(gb[2]),
                    b[3].max(gb[3]),
                ];
            }
            run.text = run.text.trim_end().to_string();
            if run.text.trim().is_empty() {
                return None;
            }
            (run.x, run.y, run.w, run.h) = (b[0], b[1], b[2] - b[0], b[3] - b[1]);
            Some(run)
        })
        .collect()
}

/// Lines of existing text on the given pages (0-based).
pub fn text_runs(doc: &Document, pages: &[usize]) -> Vec<TextRun> {
    let ids = pdf::page_ids(doc);
    pages
        .iter()
        .filter_map(|&p| ids.get(p).map(|&id| (p, id)))
        .flat_map(|(p, id)| runs_of(&interp::interpret(doc, id), p))
        .collect()
}

/// Glyphs whose centre lies in a box (visual space, normalized).
pub fn glyphs_in(page: &Page, b: [f32; 4]) -> Vec<usize> {
    page.glyphs
        .iter()
        .enumerate()
        .filter(|(_, g)| {
            let c = g.center();
            c[0] >= b[0] && c[0] <= b[2] && c[1] >= b[1] && c[1] <= b[3]
        })
        .map(|(i, _)| i)
        .collect()
}

/// The font most of `glyphs` use.
pub fn dominant_font(page: &Page, glyphs: &[usize]) -> Option<usize> {
    let mut count: HashMap<usize, usize> = HashMap::new();
    for &g in glyphs {
        *count.entry(page.glyphs[g].font).or_default() += 1;
    }
    count
        .into_iter()
        .max_by_key(|(f, c)| (*c, usize::MAX - f))
        .map(|(f, _)| f)
}

/// Codes that write `text` in font `font`, from what this page shows with it
/// (and, for complete simple fonts, their whole encoding). `None` when a
/// character is missing.
pub fn encode_with(page: &Page, font: usize, text: &str) -> Option<Vec<(u32, usize)>> {
    let f: &Font = &page.fonts[font];
    if f.vertical() || f.uncertain {
        return None;
    }
    let mut map: HashMap<String, (u32, usize)> = HashMap::new();
    if let Some(all) = f.charset() {
        for (t, c) in all {
            map.entry(t).or_insert((c, 1));
        }
    }
    for g in page.glyphs.iter().filter(|g| g.font == font) {
        if let Some(t) = &g.text
            && t.chars().count() == 1
        {
            map.entry(t.clone()).or_insert((g.code, g.len));
        }
    }
    // A simple font's space is code 32 more often than not.
    if f.kind == Kind::Simple && !map.contains_key(" ") && f.width(32, 1) > 0.0 {
        map.insert(" ".into(), (32, 1));
    }
    text.chars()
        .map(|c| map.get(c.to_string().as_str()).copied())
        .collect()
}

/// Name of a bundled font: "sans", "serif" or "mono", then "-regular",
/// "-bold", "-italic" or "-bolditalic".
pub fn bundled_key(family: Family, bold: bool, italic: bool) -> String {
    let fam = match family {
        Family::Sans => "sans",
        Family::Serif => "serif",
        Family::Mono => "mono",
    };
    let style = match (bold, italic) {
        (false, false) => "regular",
        (true, false) => "bold",
        (false, true) => "italic",
        (true, true) => "bolditalic",
    };
    format!("{fam}-{style}")
}

pub fn parse_family(s: &str) -> Option<Family> {
    match s {
        "sans" => Some(Family::Sans),
        "serif" => Some(Family::Serif),
        "mono" => Some(Family::Mono),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures;

    #[test]
    fn lines_of_the_fixture() {
        let doc = pdf::load(&fixtures::text_pdf(1), None).unwrap();
        let runs = text_runs(&doc, &[0]);
        let texts: Vec<&str> = runs.iter().map(|r| r.text.as_str()).collect();
        assert_eq!(
            texts,
            [
                "Page 1",
                "The quick brown fox jumps over the lazy dog.",
                "Second line of body text."
            ]
        );
        let r = &runs[0];
        assert!(r.editable && !r.bold && r.family == Family::Sans);
        assert!(
            (r.size - 24.0).abs() < 0.01
                && (r.start - 72.0).abs() < 0.01
                && (r.baseline - 720.0).abs() < 0.01
        );
        assert_eq!(r.color, "#000000");
        assert!(r.y < 720.0 && r.y + r.h > 720.0);
    }

    #[test]
    fn reuses_the_font_when_it_has_the_letters() {
        let doc = pdf::load(&fixtures::text_pdf(1), None).unwrap();
        let page = interp::interpret(&doc, pdf::page_ids(&doc)[0]);
        // Helvetica with WinAnsi is complete: any Latin-1 text fits.
        assert!(encode_with(&page, 0, "Où est l'été ?").is_some());
        assert!(encode_with(&page, 0, "Ωμέγα").is_none());
        assert_eq!(bundled_key(Family::Serif, true, true), "serif-bolditalic");
    }
}
