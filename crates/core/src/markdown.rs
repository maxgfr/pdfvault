//! Positioned text (from the PDF's own text or from OCR) → Markdown.
//! Headings come from font sizes, paragraphs from line spacing; running
//! headers and footers repeated across pages are dropped. No AI model here:
//! a page GLM-OCR already wrote as Markdown is passed through as it is.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A run of text in points, origin at the top-left of the displayed page.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Span {
    pub text: String,
    pub x: f32,
    pub y: f32,
    #[serde(default)]
    pub w: f32,
    pub size: f32,
    #[serde(default)]
    pub bold: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct PageText {
    pub spans: Vec<Span>,
    /// The page already written as Markdown (GLM-OCR): emitted verbatim,
    /// its spans ignored.
    #[serde(default)]
    pub markdown: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct MarkdownOptions {
    /// Put a horizontal rule between pages.
    pub page_breaks: bool,
    /// Keep running headers and footers.
    pub keep_running_heads: bool,
    /// Plain text: no heading marks and no Markdown escaping.
    pub plain: bool,
}

#[derive(Debug, Clone)]
struct Line {
    text: String,
    y: f32,
    size: f32,
    bold: bool,
}

fn lines_of(page: &PageText) -> Vec<Line> {
    let mut spans: Vec<&Span> = page
        .spans
        .iter()
        .filter(|s| !s.text.trim().is_empty() && s.size > 0.0)
        .collect();
    spans.sort_by(|a, b| a.y.total_cmp(&b.y).then(a.x.total_cmp(&b.x)));
    let mut rows: Vec<Vec<&Span>> = vec![];
    for s in spans {
        match rows.last_mut() {
            Some(row) if (row[0].y - s.y).abs() < row[0].size.min(s.size) * 0.5 => row.push(s),
            _ => rows.push(vec![s]),
        }
    }
    rows.into_iter()
        .map(|mut row| {
            row.sort_by(|a, b| a.x.total_cmp(&b.x));
            let mut text = String::new();
            let mut end: Option<f32> = None;
            for s in &row {
                if let Some(e) = end {
                    let gap = s.x - e;
                    if gap > s.size * 0.15 && !text.ends_with(' ') && !s.text.starts_with(' ') {
                        text.push(' ');
                    }
                }
                text.push_str(&s.text);
                end = Some(if s.w > 0.0 {
                    s.x + s.w
                } else {
                    s.x + s.size * 0.5 * s.text.chars().count() as f32
                });
            }
            let chars: usize = row
                .iter()
                .map(|s| s.text.chars().count())
                .sum::<usize>()
                .max(1);
            let size = row
                .iter()
                .map(|s| s.size * s.text.chars().count() as f32)
                .sum::<f32>()
                / chars as f32;
            let bold = row.iter().all(|s| s.bold);
            Line {
                text: collapse(&text),
                y: row[0].y,
                size,
                bold,
            }
        })
        .filter(|l| !l.text.is_empty())
        .collect()
}

fn collapse(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn key(size: f32) -> i32 {
    (size * 2.0).round() as i32
}

fn normalize(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_ascii_digit() { '#' } else { c })
        .collect::<String>()
        .to_lowercase()
}

fn list_marker(s: &str) -> Option<(bool, &str)> {
    for b in ["• ", "- ", "– ", "* ", "· ", "▪ "] {
        if let Some(rest) = s.strip_prefix(b) {
            return Some((false, rest));
        }
    }
    let digits = s.chars().take_while(|c| c.is_ascii_digit()).count();
    if digits > 0 && digits < 4 {
        let rest = &s[digits..];
        if let Some(r) = rest.strip_prefix(". ").or_else(|| rest.strip_prefix(") ")) {
            return Some((true, r));
        }
    }
    None
}

fn escape(s: &str) -> String {
    let mut out = s
        .replace('\\', "\\\\")
        .replace('*', "\\*")
        .replace('_', "\\_");
    if out.starts_with('#') || out.starts_with('>') || out.starts_with('+') || out.starts_with('=')
    {
        out.insert(0, '\\');
    }
    out
}

/// Plain text from Markdown a model wrote: heading and emphasis marks,
/// inline code ticks and table rules dropped, table cells spaced apart.
fn strip_markdown(md: &str) -> String {
    md.lines()
        .filter(|l| {
            let t = l.trim();
            !(t.contains('-') && t.chars().all(|c| matches!(c, '|' | '-' | ':' | ' ')))
        })
        .map(|l| {
            let t = l.trim_start();
            let t = match t.trim_start_matches('#') {
                rest if rest.len() < t.len() && rest.starts_with(' ') => rest.trim_start(),
                _ => t,
            };
            let t = t.strip_prefix("> ").unwrap_or(t);
            let t = t.replace("**", "").replace("__", "").replace('`', "");
            if t.trim_start().starts_with('|') {
                let cells: Vec<&str> = t
                    .trim()
                    .trim_matches('|')
                    .split('|')
                    .map(str::trim)
                    .collect();
                cells.join("  ")
            } else {
                t
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn to_markdown(pages: &[PageText], o: &MarkdownOptions) -> String {
    let escape = |s: &str| if o.plain { s.to_string() } else { escape(s) };
    let written: Vec<Option<&str>> = pages.iter().map(|p| p.markdown.as_deref()).collect();
    let mut pages: Vec<Vec<Line>> = pages
        .iter()
        .map(|p| {
            if p.markdown.is_some() {
                vec![]
            } else {
                lines_of(p)
            }
        })
        .collect();

    // Running heads: a first or last line set apart from the body by a wide
    // gap, repeated (digits aside) on many pages.
    if !o.keep_running_heads && pages.len() >= 3 {
        let detached = |p: &[Line], first: bool| -> Option<String> {
            let (edge, next) = if first {
                (p.first()?, p.get(1)?)
            } else {
                (p.last()?, p.get(p.len().checked_sub(2)?)?)
            };
            ((edge.y - next.y).abs() > edge.size.max(next.size) * 2.5)
                .then(|| normalize(&edge.text))
        };
        let mut counts: HashMap<String, usize> = HashMap::new();
        for p in &pages {
            for key in [detached(p, true), detached(p, false)]
                .into_iter()
                .flatten()
            {
                *counts.entry(key).or_default() += 1;
            }
        }
        let threshold = (pages.len() / 2).max(3);
        for p in &mut pages {
            let repeated = |first: bool, p: &[Line]| {
                detached(p, first)
                    .is_some_and(|k| counts.get(&k).copied().unwrap_or(0) >= threshold)
            };
            if repeated(false, p) {
                p.pop();
            }
            if repeated(true, p) {
                p.remove(0);
            }
        }
    }

    // Body size: the size carrying the most characters.
    let mut weight: HashMap<i32, usize> = HashMap::new();
    for l in pages.iter().flatten() {
        *weight.entry(key(l.size)).or_default() += l.text.chars().count();
    }
    let body = weight
        .iter()
        .max_by_key(|(k, v)| (**v, -**k))
        .map(|(k, _)| *k)
        .unwrap_or(24);
    let mut larger: Vec<i32> = weight
        .keys()
        .copied()
        .filter(|k| *k as f32 >= body as f32 * 1.15)
        .collect();
    larger.sort_unstable_by(|a, b| b.cmp(a));
    larger.truncate(3);
    let level = |l: &Line| -> Option<usize> {
        if l.text.chars().count() > 120 {
            return None;
        }
        if let Some(i) = larger.iter().position(|k| *k == key(l.size)) {
            return Some(i + 1);
        }
        // Bold body-size lines standing alone read as minor headings.
        (l.bold && key(l.size) == body && l.text.chars().count() < 80 && !l.text.ends_with('.'))
            .then_some(larger.len() + 1)
    };

    let mut out: Vec<String> = vec![];
    for (pi, lines) in pages.iter().enumerate() {
        if pi > 0 && o.page_breaks {
            out.push(if o.plain {
                "\u{c}".into()
            } else {
                "---".into()
            });
        }
        if let Some(md) = written[pi] {
            let md = if o.plain {
                strip_markdown(md)
            } else {
                md.to_string()
            };
            if !md.trim().is_empty() {
                out.push(md.trim().to_string());
            }
            continue;
        }
        let mut para = String::new();
        let mut prev: Option<&Line> = None;
        let flush = |para: &mut String, out: &mut Vec<String>| {
            if !para.is_empty() {
                out.push(std::mem::take(para));
            }
        };
        for l in lines {
            if let Some(h) = level(l) {
                flush(&mut para, &mut out);
                out.push(if o.plain {
                    l.text.clone()
                } else {
                    format!("{} {}", "#".repeat(h.min(6)), l.text)
                });
                prev = None;
                continue;
            }
            if let Some((ordered, rest)) = list_marker(&l.text) {
                flush(&mut para, &mut out);
                para = if ordered {
                    let num: String = l.text.chars().take_while(|c| c.is_ascii_digit()).collect();
                    format!("{num}. {}", escape(rest))
                } else {
                    format!("- {}", escape(rest))
                };
                prev = Some(l);
                continue;
            }
            let joins = prev.is_some_and(|p| {
                let gap = l.y - p.y;
                gap > 0.0 && gap < p.size.max(l.size) * 1.8 && key(p.size) == key(l.size)
            });
            if joins && !para.is_empty() {
                if para.ends_with('-') && l.text.chars().next().is_some_and(char::is_lowercase) {
                    para.pop();
                } else {
                    para.push(' ');
                }
                para.push_str(&escape(&l.text));
            } else {
                flush(&mut para, &mut out);
                para = escape(&l.text);
            }
            prev = Some(l);
        }
        flush(&mut para, &mut out);
    }
    let mut md = out.join("\n\n");
    md.push('\n');
    md
}

#[cfg(test)]
mod tests {
    use super::*;

    fn span(text: &str, y: f32, size: f32) -> Span {
        Span {
            text: text.into(),
            x: 72.0,
            y,
            w: 0.0,
            size,
            bold: false,
        }
    }

    #[test]
    fn headings_paragraphs_lists() {
        let page = PageText {
            spans: vec![
                span("Annual report", 60.0, 24.0),
                span("Revenue grew in every re-", 100.0, 11.0),
                span("gion this year.", 114.0, 11.0),
                span("Next paragraph starts here.", 150.0, 11.0),
                span("Details", 190.0, 16.0),
                span("• first item", 220.0, 11.0),
                span("2. second item", 234.0, 11.0),
            ],
            ..Default::default()
        };
        let md = to_markdown(&[page], &MarkdownOptions::default());
        assert_eq!(
            md,
            "# Annual report\n\nRevenue grew in every region this year.\n\nNext paragraph starts here.\n\n## Details\n\n- first item\n\n2. second item\n"
        );
    }

    #[test]
    fn plain_text() {
        let page = PageText {
            spans: vec![
                span("Title *x*", 10.0, 20.0),
                span("Body_text.", 50.0, 10.0),
            ],
            ..Default::default()
        };
        let o = MarkdownOptions {
            plain: true,
            ..Default::default()
        };
        assert_eq!(to_markdown(&[page], &o), "Title *x*\n\nBody_text.\n");
    }

    #[test]
    fn running_heads_dropped() {
        let pages: Vec<PageText> = (1..=4)
            .map(|i| PageText {
                spans: vec![
                    span("ACME Corp — confidential", 20.0, 9.0),
                    span(&format!("Body text of page {i} goes on."), 100.0, 11.0),
                    span(&format!("Page {i}"), 780.0, 9.0),
                ],
                ..Default::default()
            })
            .collect();
        let md = to_markdown(&pages, &MarkdownOptions::default());
        assert!(!md.contains("ACME") && !md.contains("Page 2"), "{md}");
        assert!(md.contains("Body text of page 3"));
    }

    #[test]
    fn written_markdown_is_verbatim() {
        let glm = "# Invoice\n\n| Item | Total |\n| --- | --- |\n| **Tea** | 4_50 |\n\n$x^2$";
        let pages = vec![
            PageText {
                spans: vec![span("Typed page.", 100.0, 11.0)],
                ..Default::default()
            },
            PageText {
                // Spans are ignored when the page comes as Markdown.
                spans: vec![span("ignored", 100.0, 30.0)],
                markdown: Some(format!("{glm}\n\n")),
            },
        ];
        let md = to_markdown(
            &pages,
            &MarkdownOptions {
                page_breaks: true,
                ..Default::default()
            },
        );
        assert_eq!(md, format!("Typed page.\n\n---\n\n{glm}\n"));

        let plain = to_markdown(
            &pages[1..],
            &MarkdownOptions {
                plain: true,
                ..Default::default()
            },
        );
        assert_eq!(plain, "Invoice\n\nItem  Total\nTea  4_50\n\n$x^2$\n");
    }

    #[test]
    fn spans_on_one_line_join() {
        let page = PageText {
            spans: vec![
                Span {
                    text: "Hello".into(),
                    x: 72.0,
                    y: 100.0,
                    w: 30.0,
                    size: 12.0,
                    bold: false,
                },
                Span {
                    text: "world".into(),
                    x: 106.0,
                    y: 100.5,
                    w: 30.0,
                    size: 12.0,
                    bold: false,
                },
            ],
            ..Default::default()
        };
        assert_eq!(
            to_markdown(&[page], &MarkdownOptions::default()),
            "Hello world\n"
        );
    }
}
