//! Drawing on top of pages: text watermarks, page numbers, headers and
//! footers, and image stamps (visual signatures). Text uses the standard
//! Helvetica font, so nothing is embedded and Latin-1 text works everywhere.

use crate::error::{Error, Result};
use crate::images;
use crate::pdf::{self, n};
use lopdf::{Document, Object, ObjectId, dictionary};
use serde::Deserialize;

/// Helvetica advance widths (1/1000 em) for WinAnsi 32..=126.
const HELVETICA: [u16; 95] = [
    278, 278, 355, 556, 556, 889, 667, 191, 333, 333, 389, 584, 278, 333, 278, 278, 556, 556, 556,
    556, 556, 556, 556, 556, 556, 556, 278, 278, 584, 584, 584, 556, 1015, 667, 667, 722, 722, 667,
    611, 778, 722, 278, 500, 667, 556, 833, 722, 778, 667, 778, 722, 667, 611, 722, 667, 944, 667,
    667, 611, 278, 278, 278, 469, 556, 333, 556, 556, 500, 556, 556, 278, 556, 556, 222, 222, 500,
    222, 833, 556, 556, 556, 556, 333, 500, 278, 556, 500, 722, 500, 500, 500, 334, 260, 334, 584,
];

const HELVETICA_BOLD: [u16; 95] = [
    278, 333, 474, 556, 556, 889, 722, 238, 333, 333, 389, 584, 278, 333, 278, 278, 556, 556, 556,
    556, 556, 556, 556, 556, 556, 556, 333, 333, 584, 584, 584, 611, 975, 722, 722, 722, 722, 667,
    611, 778, 722, 278, 556, 722, 611, 833, 722, 778, 667, 778, 722, 667, 611, 722, 667, 944, 667,
    667, 611, 333, 278, 333, 584, 556, 333, 556, 611, 556, 611, 556, 333, 611, 611, 278, 278, 556,
    278, 889, 611, 611, 611, 611, 389, 556, 333, 611, 556, 778, 556, 556, 500, 389, 280, 389, 584,
];

/// Encode text as WinAnsi bytes; characters outside it become `?`.
pub fn win_ansi(s: &str) -> Vec<u8> {
    s.chars()
        .map(|c| match c as u32 {
            0x20..=0x7E | 0xA0..=0xFF => c as u8,
            0x20AC => 0x80,
            0x2018 => 0x91,
            0x2019 => 0x92,
            0x201C => 0x93,
            0x201D => 0x94,
            0x2022 => 0x95,
            0x2013 => 0x96,
            0x2014 => 0x97,
            0x2026 => 0x85,
            0x09 => b' ',
            _ => b'?',
        })
        .collect()
}

/// Width in points of WinAnsi `bytes` at `size`.
pub fn text_width(bytes: &[u8], size: f32, bold: bool) -> f32 {
    let table = if bold { &HELVETICA_BOLD } else { &HELVETICA };
    let units: u32 = bytes
        .iter()
        .map(|&b| match b {
            32..=126 => table[(b - 32) as usize] as u32,
            0xC0..=0xC5 | 0xC8..=0xCB | 0xD2..=0xD6 | 0xD9..=0xDC => 667,
            0xE0..=0xE5 | 0xE8..=0xEB | 0xF2..=0xF6 | 0xF9..=0xFC => 556,
            0xCC..=0xCF | 0xEC..=0xEF => 278,
            _ => 556,
        })
        .sum();
    units as f32 * size / 1000.0
}

fn font_dict(bold: bool) -> Object {
    dictionary! {
        "Type" => "Font", "Subtype" => "Type1",
        "BaseFont" => if bold { "Helvetica-Bold" } else { "Helvetica" },
        "Encoding" => "WinAnsiEncoding",
    }
    .into()
}

fn add_font(doc: &mut Document, page: ObjectId, bold: bool) -> Result<String> {
    let name = pdf::add_resource(
        doc,
        page,
        b"Font",
        if bold { "PVHB" } else { "PVH" },
        font_dict(bold),
    )?;
    Ok(String::from_utf8_lossy(&name).into_owned())
}

fn add_alpha(doc: &mut Document, page: ObjectId, opacity: f32) -> Result<Option<String>> {
    if opacity >= 0.999 {
        return Ok(None);
    }
    let gs: Object = dictionary! { "Type" => "ExtGState", "ca" => opacity, "CA" => opacity }.into();
    let name = pdf::add_resource(doc, page, b"ExtGState", "PVGS", gs)?;
    Ok(Some(String::from_utf8_lossy(&name).into_owned()))
}

/// Parse `#rrggbb` into 0..1 components.
pub fn parse_color(s: &str) -> Result<[f32; 3]> {
    let h = s.trim().trim_start_matches('#');
    if h.len() != 6 || !h.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(Error::option(format!(
            "color must look like #rrggbb, got '{s}'"
        )));
    }
    let c = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).unwrap() as f32 / 255.0;
    Ok([c(0), c(2), c(4)])
}

/// Matrix that maps "visual" page space (origin bottom-left of the page as it
/// is displayed, after /Rotate) onto PDF user space, plus the visual size.
pub(crate) fn visual_frame(doc: &Document, page: ObjectId) -> ([f32; 6], f32, f32) {
    let b = pdf::page_box(doc, page);
    let (w, h) = (b[2] - b[0], b[3] - b[1]);
    match pdf::page_rotation(doc, page) {
        90 => ([0.0, 1.0, -1.0, 0.0, b[2], b[1]], h, w),
        180 => ([-1.0, 0.0, 0.0, -1.0, b[2], b[3]], w, h),
        270 => ([0.0, -1.0, 1.0, 0.0, b[0], b[3]], h, w),
        _ => ([1.0, 0.0, 0.0, 1.0, b[0], b[1]], w, h),
    }
}

fn cm(m: &[f32; 6]) -> String {
    format!(
        "{} {} {} {} {} {} cm\n",
        n(m[0]),
        n(m[1]),
        n(m[2]),
        n(m[3]),
        n(m[4]),
        n(m[5])
    )
}

fn push_text(ops: &mut Vec<u8>, font: &str, size: f32, x: f32, y: f32, bytes: &[u8]) {
    ops.extend(format!("BT /{font} {} Tf {} {} Td ", n(size), n(x), n(y)).into_bytes());
    ops.extend(pdf::pdf_string(bytes));
    ops.extend(b" Tj ET\n");
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct WatermarkOptions {
    pub text: String,
    pub font_size: f32,
    pub color: String,
    pub opacity: f32,
    /// Degrees counter-clockwise; ignored when `position` is "diagonal".
    pub rotation: f32,
    /// "diagonal", "center", "top", "bottom" or "tile".
    pub position: String,
    pub bold: bool,
    pub pages: Option<String>,
    pub password: Option<String>,
}

impl Default for WatermarkOptions {
    fn default() -> Self {
        WatermarkOptions {
            text: String::new(),
            font_size: 64.0,
            color: "#c0392b".into(),
            opacity: 0.25,
            rotation: 0.0,
            position: "diagonal".into(),
            bold: true,
            pages: None,
            password: None,
        }
    }
}

pub fn watermark(input: &[u8], o: &WatermarkOptions) -> Result<Vec<u8>> {
    if o.text.trim().is_empty() {
        return Err(Error::option("watermark text is empty"));
    }
    if !(1.0..=500.0).contains(&o.font_size) {
        return Err(Error::option("font size must be between 1 and 500"));
    }
    let color = parse_color(&o.color)?;
    let mut doc = pdf::load(input, o.password.as_deref())?;
    let ids = pdf::page_ids(&doc);
    let bytes = win_ansi(&o.text);
    for p in pdf::parse_ranges(o.pages.as_deref().unwrap_or(""), ids.len())? {
        let id = ids[p];
        let font = add_font(&mut doc, id, o.bold)?;
        let gs = add_alpha(&mut doc, id, o.opacity.clamp(0.0, 1.0))?;
        let (frame, w, h) = visual_frame(&doc, id);
        let tw = text_width(&bytes, o.font_size, o.bold);
        let mut ops = Vec::new();
        ops.extend(b"q\n");
        ops.extend(cm(&frame).into_bytes());
        if let Some(gs) = &gs {
            ops.extend(format!("/{gs} gs\n").into_bytes());
        }
        ops.extend(format!("{} {} {} rg\n", n(color[0]), n(color[1]), n(color[2])).into_bytes());
        let placements: Vec<(f32, f32, f32)> = match o.position.as_str() {
            "diagonal" => vec![(w / 2.0, h / 2.0, h.atan2(w).to_degrees())],
            "center" => vec![(w / 2.0, h / 2.0, o.rotation)],
            "top" => vec![(w / 2.0, h - o.font_size * 1.5, o.rotation)],
            "bottom" => vec![(w / 2.0, o.font_size * 0.8, o.rotation)],
            "tile" => {
                let step_x = (tw + o.font_size * 2.0).max(40.0);
                let step_y = o.font_size * 4.0;
                let mut v = vec![];
                let mut y = step_y / 2.0;
                let mut row = 0;
                while y < h + step_y {
                    let mut x = if row % 2 == 0 { step_x / 2.0 } else { step_x };
                    while x < w + step_x {
                        v.push((x, y, 30.0));
                        x += step_x;
                    }
                    y += step_y;
                    row += 1;
                }
                v
            }
            other => return Err(Error::option(format!("unknown position '{other}'"))),
        };
        for (cx, cy, deg) in placements {
            let (s, c) = deg.to_radians().sin_cos();
            ops.extend(b"q\n");
            ops.extend(cm(&[c, s, -s, c, cx, cy]).into_bytes());
            push_text(
                &mut ops,
                &font,
                o.font_size,
                -tw / 2.0,
                -o.font_size * 0.35,
                &bytes,
            );
            ops.extend(b"Q\n");
        }
        ops.extend(b"Q\n");
        pdf::append_content(&mut doc, id, ops)?;
    }
    pdf::save(&mut doc)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct HeaderFooterOptions {
    /// Text for each slot. `{n}` is the page number, `{total}` the page
    /// count, `{date}` the date passed in `date`.
    pub header_left: String,
    pub header_center: String,
    pub header_right: String,
    pub footer_left: String,
    pub footer_center: String,
    pub footer_right: String,
    pub font_size: f32,
    pub color: String,
    pub margin: f32,
    /// Number shown on the first selected page.
    pub start: i64,
    pub date: String,
    pub pages: Option<String>,
    /// Skip the first selected page (title pages).
    pub skip_first: bool,
    pub password: Option<String>,
}

impl Default for HeaderFooterOptions {
    fn default() -> Self {
        HeaderFooterOptions {
            header_left: String::new(),
            header_center: String::new(),
            header_right: String::new(),
            footer_left: String::new(),
            footer_center: String::new(),
            footer_right: String::new(),
            font_size: 10.0,
            color: "#333333".into(),
            margin: 28.0,
            start: 1,
            date: String::new(),
            pages: None,
            skip_first: false,
            password: None,
        }
    }
}

/// Headers and footers; page numbering is the special case of a footer.
pub fn header_footer(input: &[u8], o: &HeaderFooterOptions) -> Result<Vec<u8>> {
    let slots = [
        (&o.header_left, 0, true),
        (&o.header_center, 1, true),
        (&o.header_right, 2, true),
        (&o.footer_left, 0, false),
        (&o.footer_center, 1, false),
        (&o.footer_right, 2, false),
    ];
    if slots.iter().all(|s| s.0.trim().is_empty()) {
        return Err(Error::option(
            "nothing to write: every header and footer slot is empty",
        ));
    }
    if !(1.0..=200.0).contains(&o.font_size) {
        return Err(Error::option("font size must be between 1 and 200"));
    }
    let color = parse_color(&o.color)?;
    let mut doc = pdf::load(input, o.password.as_deref())?;
    let ids = pdf::page_ids(&doc);
    let selected = pdf::parse_ranges(o.pages.as_deref().unwrap_or(""), ids.len())?;
    let total = selected.len() as i64 + o.start - 1;
    for (i, p) in selected.iter().enumerate() {
        if o.skip_first && i == 0 {
            continue;
        }
        let id = ids[*p];
        let font = add_font(&mut doc, id, false)?;
        let (frame, w, h) = visual_frame(&doc, id);
        let mut ops = Vec::new();
        ops.extend(b"q\n");
        ops.extend(cm(&frame).into_bytes());
        ops.extend(format!("{} {} {} rg\n", n(color[0]), n(color[1]), n(color[2])).into_bytes());
        let number = o.start + i as i64;
        for (text, col, top) in slots {
            if text.trim().is_empty() {
                continue;
            }
            let t = text
                .replace("{n}", &number.to_string())
                .replace("{total}", &total.to_string())
                .replace("{date}", &o.date);
            let bytes = win_ansi(&t);
            let tw = text_width(&bytes, o.font_size, false);
            let x = match col {
                0 => o.margin,
                1 => (w - tw) / 2.0,
                _ => w - o.margin - tw,
            };
            let y = if top {
                h - o.margin - o.font_size * 0.8
            } else {
                o.margin
            };
            push_text(&mut ops, &font, o.font_size, x, y, &bytes);
        }
        ops.extend(b"Q\n");
        pdf::append_content(&mut doc, id, ops)?;
    }
    pdf::save(&mut doc)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct NumberOptions {
    /// `{n}` and `{total}` placeholders, e.g. "Page {n} of {total}".
    pub format: String,
    /// "bottom-center", "bottom-right", "bottom-left", "top-center", "top-right", "top-left".
    pub position: String,
    pub font_size: f32,
    pub color: String,
    pub margin: f32,
    pub start: i64,
    pub pages: Option<String>,
    pub skip_first: bool,
    pub password: Option<String>,
}

impl Default for NumberOptions {
    fn default() -> Self {
        NumberOptions {
            format: "{n}".into(),
            position: "bottom-center".into(),
            font_size: 10.0,
            color: "#333333".into(),
            margin: 28.0,
            start: 1,
            pages: None,
            skip_first: false,
            password: None,
        }
    }
}

pub fn number(input: &[u8], o: &NumberOptions) -> Result<Vec<u8>> {
    let mut hf = HeaderFooterOptions {
        font_size: o.font_size,
        color: o.color.clone(),
        margin: o.margin,
        start: o.start,
        pages: o.pages.clone(),
        skip_first: o.skip_first,
        password: o.password.clone(),
        ..Default::default()
    };
    let slot = match o.position.as_str() {
        "bottom-center" => &mut hf.footer_center,
        "bottom-right" => &mut hf.footer_right,
        "bottom-left" => &mut hf.footer_left,
        "top-center" => &mut hf.header_center,
        "top-right" => &mut hf.header_right,
        "top-left" => &mut hf.header_left,
        other => return Err(Error::option(format!("unknown position '{other}'"))),
    };
    *slot = o.format.clone();
    header_footer(input, &hf)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct StampOptions {
    /// Position of the image's bottom-left corner, in points from the
    /// bottom-left of the displayed page.
    pub x: f32,
    pub y: f32,
    /// Width in points; the height follows the image's aspect ratio.
    pub width: f32,
    pub opacity: f32,
    pub pages: Option<String>,
    pub password: Option<String>,
}

impl Default for StampOptions {
    fn default() -> Self {
        StampOptions {
            x: 360.0,
            y: 72.0,
            width: 160.0,
            opacity: 1.0,
            pages: Some("last".into()),
            password: None,
        }
    }
}

/// Place an image (PNG with transparency, or JPEG) on pages: a visual signature.
pub fn stamp_image(input: &[u8], image: &[u8], o: &StampOptions) -> Result<Vec<u8>> {
    if o.width <= 0.0 {
        return Err(Error::option("width must be positive"));
    }
    let mut doc = pdf::load(input, o.password.as_deref())?;
    let (xobj, iw, ih) = images::embed_image(&mut doc, image)?;
    let height = o.width * ih as f32 / iw as f32;
    let ids = pdf::page_ids(&doc);
    for p in pdf::parse_ranges(o.pages.as_deref().unwrap_or("last"), ids.len())? {
        let id = ids[p];
        let name = pdf::add_resource(&mut doc, id, b"XObject", "PVIm", Object::Reference(xobj))?;
        let gs = add_alpha(&mut doc, id, o.opacity.clamp(0.0, 1.0))?;
        let (frame, _, _) = visual_frame(&doc, id);
        let mut ops = String::from("q\n");
        ops.push_str(&cm(&frame));
        if let Some(gs) = gs {
            ops.push_str(&format!("/{gs} gs\n"));
        }
        ops.push_str(&cm(&[o.width, 0.0, 0.0, height, o.x, o.y]));
        ops.push_str(&format!("/{} Do\nQ\n", String::from_utf8_lossy(&name)));
        pdf::append_content(&mut doc, id, ops.into_bytes())?;
    }
    pdf::save(&mut doc)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures;
    use crate::testutil::qpdf_check;

    fn text_of(pdf_bytes: &[u8], page: u32) -> String {
        pdf::load(pdf_bytes, None)
            .unwrap()
            .extract_text(&[page])
            .unwrap_or_default()
    }

    #[test]
    fn widths() {
        assert_eq!(text_width(b"A", 1000.0, false), 667.0);
        assert!(text_width(b"WWW", 10.0, true) > text_width(b"iii", 10.0, true));
        assert_eq!(win_ansi("é€✓"), vec![0xE9, 0x80, b'?']);
    }

    #[test]
    fn watermark_every_page() {
        let o = WatermarkOptions {
            text: "CONFIDENTIEL".into(),
            ..Default::default()
        };
        let out = watermark(&fixtures::text_pdf(2), &o).unwrap();
        assert!(text_of(&out, 2).contains("CONFIDENTIEL"));
        qpdf_check(&out);
        let tile = WatermarkOptions {
            position: "tile".into(),
            text: "DRAFT".into(),
            ..Default::default()
        };
        qpdf_check(&watermark(&fixtures::text_pdf(1), &tile).unwrap());
        assert!(watermark(&fixtures::text_pdf(1), &WatermarkOptions::default()).is_err());
    }

    #[test]
    fn numbers_with_total() {
        let o = NumberOptions {
            format: "Page {n} of {total}".into(),
            ..Default::default()
        };
        let out = number(&fixtures::text_pdf(3), &o).unwrap();
        assert!(text_of(&out, 2).contains("Page 2 of 3"));
        qpdf_check(&out);
    }

    #[test]
    fn header_footer_slots() {
        let o = HeaderFooterOptions {
            header_left: "ACME".into(),
            footer_right: "{n}/{total}".into(),
            date: "2026-01-01".into(),
            header_right: "{date}".into(),
            skip_first: true,
            ..Default::default()
        };
        let out = header_footer(&fixtures::text_pdf(3), &o).unwrap();
        assert!(!text_of(&out, 1).contains("ACME"));
        let p2 = text_of(&out, 2);
        assert!(
            p2.contains("ACME") && p2.contains("2/3") && p2.contains("2026-01-01"),
            "{p2}"
        );
        qpdf_check(&out);
    }

    #[test]
    fn stamp_png_signature() {
        let sig = fixtures::png(&image::DynamicImage::ImageRgba8(fixtures::with_alpha(
            &fixtures::line_art(200, 80),
        )));
        let out = stamp_image(&fixtures::text_pdf(2), &sig, &StampOptions::default()).unwrap();
        let doc = pdf::load(&out, None).unwrap();
        let has_smask = doc
            .objects
            .values()
            .filter_map(|o| o.as_stream().ok())
            .any(|s| s.dict.has(b"SMask"));
        assert!(has_smask);
        qpdf_check(&out);
    }

    #[test]
    fn rotated_pages_get_upright_text() {
        let rotated = crate::pages::rotate(&fixtures::text_pdf(1), 90, None, None).unwrap();
        let out = watermark(
            &rotated,
            &WatermarkOptions {
                text: "X".into(),
                ..Default::default()
            },
        )
        .unwrap();
        qpdf_check(&out);
    }
}
