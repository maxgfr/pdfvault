//! Paragraphs of a page's text, as the editor offers them: lines grouped by
//! left edge, leading and size, with every character tied to the glyph it
//! came from so that a rewrite can keep each character's font, size and
//! colour (a bold word stays bold) while the text reflows.

use crate::interp::Page;
use crate::pdffont::Family;
use serde::Serialize;
use std::collections::HashMap;

/// One line of glyphs on a common baseline, whatever their style.
#[derive(Debug, Clone)]
pub struct Line {
    pub glyphs: Vec<usize>,
    pub text: String,
    /// For each character of `text`, the glyph it came from (-1: a space the
    /// line implies by a gap).
    pub sources: Vec<i64>,
    pub start: f32,
    pub end: f32,
    /// Where its first word ends.
    pub first_word_end: f32,
    pub baseline: f32,
    pub size: f32,
    pub bbox: [f32; 4],
    pub editable: bool,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Paragraph {
    pub page: usize,
    /// Box around its glyphs: bottom-left corner, width and height.
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    /// Where the first line starts, where the others start, and the right
    /// edge text wraps at.
    pub start: f32,
    pub left: f32,
    pub right: f32,
    /// Where rewritten text wraps: the justified edge, else the column's
    /// right edge (a single line's own end is no limit).
    pub wrap: f32,
    /// First baseline, and the distance between baselines.
    pub baseline: f32,
    pub leading: f32,
    /// "left", "right", "center" or "justify".
    pub align: String,
    pub lines: usize,
    pub text: String,
    pub sources: Vec<i64>,
    pub glyphs: Vec<usize>,
    /// The style most of its characters have, for the editor's controls.
    pub size: f32,
    pub family: Family,
    pub bold: bool,
    pub italic: bool,
    pub color: String,
    pub font_name: String,
    pub font_family: String,
    pub hscale: f32,
    pub editable: bool,
}

pub(crate) fn hex_color(c: [f32; 3]) -> String {
    let b = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("#{:02x}{:02x}{:02x}", b(c[0]), b(c[1]), b(c[2]))
}

/// The page's visible, horizontal text as lines.
pub fn lines_of(page: &Page) -> Vec<Line> {
    let mut lines: Vec<Line> = vec![];
    let mut last: Option<usize> = None;
    for (i, g) in page.glyphs.iter().enumerate() {
        // Invisible text (OCR layers, clip-only modes) and turned text are not offered.
        if g.mode == 3 || g.mode == 7 || g.size < 0.5 || g.angle.abs() > 0.5 {
            continue;
        }
        let joins = last.is_some_and(|l| {
            let p = &page.glyphs[l];
            let gap = g.origin[0] - p.end[0];
            let size = p.size.max(g.size);
            (p.origin[1] - g.origin[1]).abs() <= size * 0.25
                && gap > -size * 0.3
                && gap < size * 1.2
        });
        let text = g.text.clone();
        let t = text.clone().unwrap_or_else(|| "\u{fffd}".into());
        if joins && let Some(line) = lines.last_mut() {
            let p = &page.glyphs[*line.glyphs.last().unwrap()];
            let gap = g.origin[0] - p.end[0];
            if gap > g.size.max(p.size) * 0.18 && !line.text.ends_with(' ') && !t.starts_with(' ') {
                line.text.push(' ');
                line.sources.push(-1);
            }
            for c in t.chars() {
                line.text.push(c);
                line.sources.push(i as i64);
            }
            line.editable &= text.is_some() && !page.fonts[g.font].vertical();
            line.glyphs.push(i);
            line.end = g.end[0];
        } else {
            lines.push(Line {
                glyphs: vec![i],
                sources: t.chars().map(|_| i as i64).collect(),
                text: t,
                start: g.origin[0],
                end: g.end[0],
                first_word_end: g.end[0],
                baseline: g.origin[1],
                size: g.size,
                bbox: g.bbox,
                editable: text.is_some() && !page.fonts[g.font].vertical(),
            });
        }
        last = Some(i);
    }
    for line in &mut lines {
        let mut b = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
        let mut sizes: HashMap<u32, usize> = HashMap::new();
        for &m in &line.glyphs {
            let gb = page.glyphs[m].bbox;
            b = [
                b[0].min(gb[0]),
                b[1].min(gb[1]),
                b[2].max(gb[2]),
                b[3].max(gb[3]),
            ];
            *sizes
                .entry((page.glyphs[m].size * 100.0) as u32)
                .or_default() += 1;
        }
        line.bbox = b;
        // The first word ends at the first space, written or implied by a gap.
        let mut end = line.start;
        for (k, &m) in line.glyphs.iter().enumerate() {
            let g = &page.glyphs[m];
            let gap = k > 0 && g.origin[0] - page.glyphs[line.glyphs[k - 1]].end[0] > g.size * 0.18;
            if gap || g.text.as_deref().is_some_and(|t| t.trim().is_empty()) {
                break;
            }
            end = g.end[0];
        }
        line.first_word_end = end;
        line.size = sizes
            .into_iter()
            .max_by_key(|(s, n)| (*n, *s))
            .map(|(s, _)| s as f32 / 100.0)
            .unwrap_or(line.size);
        // Trailing spaces carry no style worth keeping.
        while line.text.ends_with(' ') {
            line.text.pop();
            line.sources.pop();
        }
    }
    lines.retain(|l| !l.text.trim().is_empty());
    lines
}

fn median(mut v: Vec<f32>) -> Option<f32> {
    if v.is_empty() {
        return None;
    }
    v.sort_by(|a, b| a.total_cmp(b));
    Some(v[v.len() / 2])
}

/// Whether `next` continues the paragraph made of `lines` so far, in a column
/// whose lines reach `column_right`.
fn continues(lines: &[&Line], next: &Line, column_right: f32) -> bool {
    let prev = lines[lines.len() - 1];
    let size = prev.size;
    if (next.size - size).abs() > size * 0.15 {
        return false;
    }
    let step = prev.baseline - next.baseline;
    if step < size * 0.9 || step > size * 1.9 {
        return false;
    }
    // The spacing stays what it was.
    if lines.len() >= 2 {
        let first = lines[0].baseline - lines[1].baseline;
        if (step - first).abs() > size * 0.15 {
            return false;
        }
    }
    // Left edges line up (a first-line indent aside), and the columns overlap.
    let left = if lines.len() >= 2 {
        lines[1].start
    } else {
        prev.start.min(next.start)
    };
    let right = lines.iter().map(|l| l.end).fold(f32::MIN, f32::max);
    if (next.start - left).abs() > size * 1.5 || next.start >= right || next.end <= left {
        return false;
    }
    // Text wraps where the next word no longer fits: when it would have fit
    // on this line, the line ended on purpose (a list item, a short line).
    let word = next.first_word_end - next.start;
    prev.end + size * 0.25 + word > column_right - size * 0.5
}

/// Group a page's lines into paragraphs.
pub fn paragraphs_of(page: &Page, index: usize) -> Vec<Paragraph> {
    let lines = lines_of(page);
    // The right edge of each line's column: the furthest that nearby lines of
    // the same size reach when they start around it (a list item's column is
    // the text's, not the item's).
    let column_right = |l: &Line| {
        lines
            .iter()
            .filter(|o| {
                (o.size - l.size).abs() <= l.size * 0.15
                    && o.start <= l.start + l.size * 2.0
                    && o.start >= l.start - l.size * 4.0
                    && o.end >= l.start
                    && (o.baseline - l.baseline).abs() <= l.size * 30.0
            })
            .map(|o| o.end)
            .fold(l.end, f32::max)
    };
    let mut groups: Vec<Vec<&Line>> = vec![];
    for line in &lines {
        match groups.last_mut() {
            Some(g) if continues(g, line, column_right(g[g.len() - 1])) => g.push(line),
            _ => groups.push(vec![line]),
        }
    }
    groups
        .into_iter()
        .map(|g| {
            let column = column_right(g[0]);
            paragraph(page, index, &g, column)
        })
        .collect()
}

fn paragraph(page: &Page, index: usize, lines: &[&Line], column_right: f32) -> Paragraph {
    let first = lines[0];
    let left = if lines.len() >= 2 {
        lines[1..].iter().map(|l| l.start).fold(f32::MAX, f32::min)
    } else {
        first.start
    };
    let right = lines.iter().map(|l| l.end).fold(f32::MIN, f32::max);
    let leading = median(
        lines
            .windows(2)
            .map(|w| w[0].baseline - w[1].baseline)
            .collect(),
    )
    .unwrap_or(first.size * 1.2);
    let tol = first.size * 0.25;
    let align = if lines.len() < 2 {
        "left"
    } else {
        let starts_even = lines.iter().all(|l| (l.start - left).abs() <= tol);
        let ends_even = lines.iter().all(|l| (l.end - right).abs() <= tol);
        let centres = lines
            .iter()
            .map(|l| (l.start + l.end) / 2.0)
            .collect::<Vec<_>>();
        let centred = centres.iter().all(|c| (c - centres[0]).abs() <= tol);
        // Justified: every line but the last reaches both edges, which takes
        // at least two such lines to tell from ragged text.
        let body = &lines[..lines.len() - 1];
        let justified = body.len() >= 2
            && body.iter().all(|l| (l.end - right).abs() <= tol)
            && lines[1..].iter().all(|l| (l.start - left).abs() <= tol);
        if justified {
            "justify"
        } else if centred && !starts_even {
            "center"
        } else if ends_even && !starts_even {
            "right"
        } else {
            "left"
        }
    };
    let wrap = if align == "justify" {
        right
    } else if column_right > right + first.size {
        column_right
    } else if lines.len() == 1 {
        // A line on its own: up to the margin mirroring its left one.
        (page.width - first.start).max(right)
    } else {
        right
    };
    let mut text = String::new();
    let mut sources = vec![];
    for (i, l) in lines.iter().enumerate() {
        if i > 0 {
            let hyphenated =
                text.ends_with('-') && l.text.chars().next().is_some_and(char::is_lowercase);
            if hyphenated {
                // A word cut at the line end: join it back.
                text.pop();
                sources.pop();
            } else {
                text.push(' ');
                sources.push(-1);
            }
        }
        text.push_str(&l.text);
        sources.extend(&l.sources);
    }
    let glyphs: Vec<usize> = lines
        .iter()
        .flat_map(|l| l.glyphs.iter().copied())
        .collect();
    let mut b = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
    for l in lines {
        b = [
            b[0].min(l.bbox[0]),
            b[1].min(l.bbox[1]),
            b[2].max(l.bbox[2]),
            b[3].max(l.bbox[3]),
        ];
    }
    // The style most characters have.
    let mut count: HashMap<usize, usize> = HashMap::new();
    for &s in sources.iter().filter(|s| **s >= 0) {
        *count.entry(s as usize).or_default() += 1;
    }
    let mut by_font: HashMap<usize, usize> = HashMap::new();
    for (&g, &n) in &count {
        *by_font.entry(page.glyphs[g].font).or_default() += n;
    }
    let font = by_font
        .into_iter()
        .max_by_key(|(f, n)| (*n, usize::MAX - f))
        .map(|(f, _)| f)
        .unwrap_or(page.glyphs[first.glyphs[0]].font);
    let sample = glyphs
        .iter()
        .map(|&g| &page.glyphs[g])
        .find(|g| g.font == font)
        .unwrap_or(&page.glyphs[first.glyphs[0]]);
    let f = &page.fonts[font];
    Paragraph {
        page: index,
        x: b[0],
        y: b[1],
        w: b[2] - b[0],
        h: b[3] - b[1],
        start: first.start,
        left,
        right,
        wrap,
        baseline: first.baseline,
        leading,
        align: align.to_string(),
        lines: lines.len(),
        text,
        sources,
        glyphs,
        size: first.size,
        family: f.family,
        bold: f.bold,
        italic: f.italic,
        color: hex_color(sample.color),
        font_name: f.base.clone(),
        font_family: f.family_name.clone(),
        hscale: sample.hscale,
        editable: lines.iter().all(|l| l.editable),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures;
    use crate::interp;
    use crate::pdf;

    #[test]
    fn the_fixture_has_a_title_and_a_two_line_paragraph() {
        let doc = pdf::load(&fixtures::text_pdf(1), None).unwrap();
        let page = interp::interpret(&doc, pdf::page_ids(&doc)[0]);
        let paras = paragraphs_of(&page, 0);
        let texts: Vec<&str> = paras.iter().map(|p| p.text.as_str()).collect();
        assert_eq!(
            texts,
            [
                "Page 1",
                "The quick brown fox jumps over the lazy dog. Second line of body text."
            ]
        );
        let body = &paras[1];
        assert_eq!(body.lines, 2);
        assert!((body.leading - 18.0).abs() < 0.01 && (body.left - 72.0).abs() < 0.01);
        assert_eq!(body.sources.len(), body.text.chars().count());
        assert_eq!(
            body.sources.iter().filter(|s| **s == -1).count(),
            1,
            "the line break became a space"
        );
        assert_eq!(body.align, "left");
    }
}
