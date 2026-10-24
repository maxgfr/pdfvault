//! The PDF editor's single write: append blank pages, fill form fields, draw
//! text, shapes, freehand ink and images onto pages, and add comments (note
//! and highlight annotations).
//!
//! Coordinates are in the page's visual space, like the image stamp: points,
//! origin at the bottom-left of the page as it is displayed (after /Rotate).
//! `page` is a 0-based index into the input's pages followed by the blank
//! pages this call appends, so items stay on their page whatever order
//! [`crate::pages::arrange`] puts the pages in afterwards.
//!
//! Redaction is not done here: it rasterizes pages, which needs the renderer
//! (`pdfvault-render`'s `redact`). `redact` items are accepted and skipped so
//! one item list can feed both steps.

use crate::error::{Error, Result};
use crate::forms::{self, FieldValue};
use crate::images;
use crate::pdf::{self, n};
use crate::stamp::{self, cm, parse_color};
use lopdf::{Dictionary, Document, Object, ObjectId, Stream, dictionary};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

/// First baseline of a text item sits this many font sizes below its top.
pub const TEXT_ASCENT: f32 = 0.8;
/// Distance between baselines, in font sizes.
pub const TEXT_LINE: f32 = 1.2;
/// Side of a note's icon, in points.
pub const NOTE_SIZE: f32 = 20.0;

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct EditOptions {
    pub password: Option<String>,
    pub blanks: Vec<Blank>,
    pub fields: BTreeMap<String, FieldValue>,
    /// Burn the form into the pages once filled.
    pub flatten_fields: bool,
    pub items: Vec<Item>,
}

#[derive(Debug, Deserialize, Default, Clone)]
#[serde(rename_all = "camelCase", default)]
pub struct Blank {
    /// "same" (the size of page `like`, default the last page), "a4" or "letter".
    pub size: Option<String>,
    pub like: Option<usize>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Item {
    Text(TextItem),
    Rect(RectItem),
    Ink(InkItem),
    Image(ImageItem),
    Note(NoteItem),
    Highlight(BoxItem),
    Redact(BoxItem),
}

impl Item {
    pub fn page(&self) -> usize {
        match self {
            Item::Text(i) => i.page,
            Item::Rect(i) => i.page,
            Item::Ink(i) => i.page,
            Item::Image(i) => i.page,
            Item::Note(i) => i.page,
            Item::Highlight(i) | Item::Redact(i) => i.page,
        }
    }
}

/// Lines of Helvetica. `x`, `y` is the top-left corner of the text box.
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase", default)]
pub struct TextItem {
    pub page: usize,
    pub x: f32,
    pub y: f32,
    pub size: f32,
    pub color: String,
    pub bold: bool,
    pub text: String,
}

impl Default for TextItem {
    fn default() -> Self {
        TextItem {
            page: 0,
            x: 0.0,
            y: 0.0,
            size: 14.0,
            color: "#000000".into(),
            bold: false,
            text: String::new(),
        }
    }
}

/// A rectangle; `x`, `y` is its bottom-left corner. A white fill is a whiteout.
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase", default)]
pub struct RectItem {
    pub page: usize,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub fill: Option<String>,
    pub stroke: Option<String>,
    pub line_width: f32,
    pub opacity: f32,
}

impl Default for RectItem {
    fn default() -> Self {
        RectItem {
            page: 0,
            x: 0.0,
            y: 0.0,
            w: 0.0,
            h: 0.0,
            fill: None,
            stroke: None,
            line_width: 1.0,
            opacity: 1.0,
        }
    }
}

/// Freehand strokes: each path is a flat `[x0, y0, x1, y1, …]` list.
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase", default)]
pub struct InkItem {
    pub page: usize,
    pub paths: Vec<Vec<f32>>,
    pub color: String,
    pub line_width: f32,
    pub opacity: f32,
}

impl Default for InkItem {
    fn default() -> Self {
        InkItem {
            page: 0,
            paths: vec![],
            color: "#000000".into(),
            line_width: 2.0,
            opacity: 1.0,
        }
    }
}

/// An image input drawn into a box; `image` counts the images after the PDF
/// from 0. `x`, `y` is the bottom-left corner.
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase", default)]
pub struct ImageItem {
    pub page: usize,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub image: usize,
    pub opacity: f32,
}

impl Default for ImageItem {
    fn default() -> Self {
        ImageItem {
            page: 0,
            x: 0.0,
            y: 0.0,
            w: 0.0,
            h: 0.0,
            image: 0,
            opacity: 1.0,
        }
    }
}

/// A sticky note (a /Text annotation); `x`, `y` is the icon's top-left corner.
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase", default)]
pub struct NoteItem {
    pub page: usize,
    pub x: f32,
    pub y: f32,
    pub text: String,
    pub color: String,
}

impl Default for NoteItem {
    fn default() -> Self {
        NoteItem {
            page: 0,
            x: 0.0,
            y: 0.0,
            text: String::new(),
            color: "#f5c518".into(),
        }
    }
}

/// A box for highlights and redactions; `x`, `y` is its bottom-left corner.
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase", default)]
pub struct BoxItem {
    pub page: usize,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub color: String,
}

impl Default for BoxItem {
    fn default() -> Self {
        BoxItem {
            page: 0,
            x: 0.0,
            y: 0.0,
            w: 0.0,
            h: 0.0,
            color: "#ffe14d".into(),
        }
    }
}

impl BoxItem {
    /// Normalized [x0, y0, x1, y1].
    pub fn bounds(&self) -> [f32; 4] {
        norm(self.x, self.y, self.w, self.h)
    }
}

#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct EditReport {
    pub pages: usize,
    pub blanks: usize,
    pub fields: usize,
    pub drawn: usize,
    pub annotations: usize,
}

fn norm(x: f32, y: f32, w: f32, h: f32) -> [f32; 4] {
    [x.min(x + w), y.min(y + h), x.max(x + w), y.max(y + h)]
}

fn rgb(c: [f32; 3]) -> String {
    format!("{} {} {}", n(c[0]), n(c[1]), n(c[2]))
}

fn finite(values: &[f32]) -> Result<()> {
    if values.iter().all(|v| v.is_finite() && v.abs() < 1e6) {
        Ok(())
    } else {
        Err(Error::option("coordinates must be finite numbers"))
    }
}

fn append_blanks(doc: &mut Document, blanks: &[Blank]) -> Result<()> {
    if blanks.is_empty() {
        return Ok(());
    }
    let mut kids: Vec<Object> = pdf::page_ids(doc).into_iter().map(Object::from).collect();
    let ids = pdf::page_ids(doc);
    for b in blanks {
        let like = b.like.unwrap_or(ids.len() - 1);
        let near = *ids
            .get(like)
            .ok_or_else(|| Error::option(format!("page {} does not exist", like + 1)))?;
        let bx = pdf::page_box(doc, near);
        let (w, h) = match b.size.as_deref().unwrap_or("same") {
            "a4" => (595.28, 841.89),
            "letter" => (612.0, 792.0),
            "same" => {
                let (w, h) = (bx[2] - bx[0], bx[3] - bx[1]);
                // Keep the neighbour's orientation as displayed.
                if pdf::page_rotation(doc, near) % 180 == 90 {
                    (h, w)
                } else {
                    (w, h)
                }
            }
            other => return Err(Error::option(format!("unknown size '{other}'"))),
        };
        let content = doc.add_object(Stream::new(Dictionary::new(), vec![]));
        let page = doc.add_object(dictionary! {
            "Type" => "Page", "MediaBox" => vec![0.into(), 0.into(), w.into(), h.into()],
            "Contents" => content, "Resources" => Dictionary::new(),
        });
        kids.push(page.into());
    }
    crate::pages::rebuild_page_tree(doc, kids)
}

fn push_annot(doc: &mut Document, page: ObjectId, annot: ObjectId) -> Result<()> {
    let existing = doc.get_dictionary(page)?.get(b"Annots").ok().cloned();
    let mut list = match existing {
        Some(Object::Reference(id)) => doc.get_object(id)?.as_array()?.clone(),
        Some(Object::Array(a)) => a,
        _ => vec![],
    };
    list.push(annot.into());
    doc.get_dictionary_mut(page)?.set("Annots", list);
    Ok(())
}

/// A visual box mapped to user space (still axis-aligned: /Rotate is a multiple of 90).
fn user_rect(frame: &[f32; 6], b: [f32; 4]) -> [f32; 4] {
    forms::transform_rect(frame, b)
}

fn rect_obj(r: [f32; 4]) -> Object {
    Object::Array(r.iter().map(|v| (*v).into()).collect())
}

fn note_annot(
    doc: &mut Document,
    page: ObjectId,
    frame: &[f32; 6],
    it: &NoteItem,
) -> Result<ObjectId> {
    let c = parse_color(&it.color)?;
    let r = user_rect(frame, [it.x, it.y - NOTE_SIZE, it.x + NOTE_SIZE, it.y]);
    let s = NOTE_SIZE;
    // A folded-corner card with three lines: legible in any viewer.
    let ops = format!(
        "{c} rg 0.2 0.2 0.2 RG 0.6 w\n1 1 m {a} 1 l {a} {b} l {b} {a} l 1 {a} l h B\n\
         1 1 1 RG 1.2 w 4 {l1} m {e} {l1} l S 4 {l2} m {e} {l2} l S 4 {l3} m {f} {l3} l S\n",
        c = rgb(c),
        a = n(s - 1.0),
        b = n(s - 6.0),
        e = n(s - 4.0),
        f = n(s - 8.0),
        l1 = n(s * 0.7),
        l2 = n(s * 0.5),
        l3 = n(s * 0.3),
    );
    let ap = doc.add_object(Stream::new(
        dictionary! {
            "Type" => "XObject", "Subtype" => "Form",
            "BBox" => vec![0.into(), 0.into(), s.into(), s.into()],
        },
        ops.into_bytes(),
    ));
    Ok(doc.add_object(dictionary! {
        "Type" => "Annot", "Subtype" => "Text", "Rect" => rect_obj(r),
        "Contents" => forms::text_string(&it.text), "Name" => "Comment",
        "C" => vec![c[0].into(), c[1].into(), c[2].into()],
        "F" => 4, "P" => page, "AP" => dictionary! { "N" => ap },
    }))
}

fn highlight_annot(
    doc: &mut Document,
    page: ObjectId,
    frame: &[f32; 6],
    it: &BoxItem,
) -> Result<ObjectId> {
    let c = parse_color(&it.color)?;
    let r = user_rect(frame, it.bounds());
    let ops = format!(
        "/GS0 gs {} rg {} {} {} {} re f\n",
        rgb(c),
        n(r[0]),
        n(r[1]),
        n(r[2] - r[0]),
        n(r[3] - r[1])
    );
    let ap = doc.add_object(Stream::new(
        dictionary! {
            "Type" => "XObject", "Subtype" => "Form", "BBox" => rect_obj(r),
            "Resources" => dictionary! { "ExtGState" => dictionary! {
                "GS0" => dictionary! { "Type" => "ExtGState", "BM" => "Multiply" },
            } },
        },
        ops.into_bytes(),
    ));
    // Upper-left, upper-right, lower-left, lower-right (PDF 32000 12.5.6.10).
    let quad = [r[0], r[3], r[2], r[3], r[0], r[1], r[2], r[1]];
    Ok(doc.add_object(dictionary! {
        "Type" => "Annot", "Subtype" => "Highlight", "Rect" => rect_obj(r),
        "QuadPoints" => Object::Array(quad.iter().map(|v| (*v).into()).collect()),
        "C" => vec![c[0].into(), c[1].into(), c[2].into()],
        "F" => 4, "P" => page, "AP" => dictionary! { "N" => ap },
    }))
}

/// Apply `o` to the PDF in `inputs[0]`; `inputs[1..]` are the images items refer to.
pub fn edit(inputs: &[Vec<u8>], o: &EditOptions) -> Result<(Vec<u8>, EditReport)> {
    let (pdf_bytes, image_inputs) = inputs
        .split_first()
        .ok_or_else(|| Error::option("edit needs a PDF"))?;
    let mut doc = pdf::load(pdf_bytes, o.password.as_deref())?;
    append_blanks(&mut doc, &o.blanks)?;
    let ids = pdf::page_ids(&doc);
    let mut report = EditReport {
        pages: ids.len(),
        blanks: o.blanks.len(),
        fields: forms::fill(&mut doc, &o.fields)?,
        ..Default::default()
    };

    let mut ops: BTreeMap<usize, Vec<u8>> = BTreeMap::new();
    let mut embedded: HashMap<usize, (ObjectId, u32, u32)> = HashMap::new();
    for item in &o.items {
        let p = item.page();
        let page = *ids.get(p).ok_or_else(|| {
            Error::option(format!(
                "page {} does not exist ({} pages)",
                p + 1,
                ids.len()
            ))
        })?;
        let mut out = Vec::new();
        let alpha = |doc: &mut Document, opacity: f32| -> Result<String> {
            Ok(stamp::add_alpha(doc, page, opacity.clamp(0.0, 1.0))?
                .map(|gs| format!("/{gs} gs\n"))
                .unwrap_or_default())
        };
        match item {
            Item::Text(t) => {
                finite(&[t.x, t.y])?;
                if !(1.0..=500.0).contains(&t.size) {
                    return Err(Error::option("font size must be between 1 and 500"));
                }
                if t.text.trim().is_empty() {
                    continue;
                }
                let font = stamp::add_font(&mut doc, page, t.bold)?;
                out.extend(format!("q {} rg\n", rgb(parse_color(&t.color)?)).into_bytes());
                for (i, line) in t.text.lines().enumerate() {
                    let y = t.y - t.size * (TEXT_ASCENT + TEXT_LINE * i as f32);
                    stamp::push_text(&mut out, &font, t.size, t.x, y, &stamp::win_ansi(line));
                }
                out.extend(b"Q\n");
            }
            Item::Rect(r) => {
                finite(&[r.x, r.y, r.w, r.h, r.line_width])?;
                let b = norm(r.x, r.y, r.w, r.h);
                let paint = match (&r.fill, &r.stroke) {
                    (Some(_), Some(_)) => "B",
                    (Some(_), None) => "f",
                    (None, Some(_)) => "S",
                    (None, None) => {
                        return Err(Error::option("a rectangle needs a fill or a stroke"));
                    }
                };
                out.extend(b"q\n");
                out.extend(alpha(&mut doc, r.opacity)?.into_bytes());
                if let Some(f) = &r.fill {
                    out.extend(format!("{} rg\n", rgb(parse_color(f)?)).into_bytes());
                }
                if let Some(s) = &r.stroke {
                    out.extend(
                        format!(
                            "{} RG {} w\n",
                            rgb(parse_color(s)?),
                            n(r.line_width.max(0.0))
                        )
                        .into_bytes(),
                    );
                }
                out.extend(
                    format!(
                        "{} {} {} {} re {paint}\nQ\n",
                        n(b[0]),
                        n(b[1]),
                        n(b[2] - b[0]),
                        n(b[3] - b[1])
                    )
                    .into_bytes(),
                );
            }
            Item::Ink(k) => {
                finite(&[k.line_width])?;
                out.extend(b"q\n");
                out.extend(alpha(&mut doc, k.opacity)?.into_bytes());
                out.extend(
                    format!(
                        "{} RG {} w 1 J 1 j\n",
                        rgb(parse_color(&k.color)?),
                        n(k.line_width.max(0.1))
                    )
                    .into_bytes(),
                );
                for path in &k.paths {
                    finite(path)?;
                    let pts = path.as_chunks::<2>().0;
                    let Some(first) = pts.first() else {
                        continue;
                    };
                    out.extend(format!("{} {} m\n", n(first[0]), n(first[1])).into_bytes());
                    // A single point still shows as a dot thanks to the round cap.
                    for pt in if pts.len() == 1 { pts } else { &pts[1..] } {
                        out.extend(format!("{} {} l\n", n(pt[0]), n(pt[1])).into_bytes());
                    }
                    out.extend(b"S\n");
                }
                out.extend(b"Q\n");
            }
            Item::Image(im) => {
                finite(&[im.x, im.y, im.w, im.h])?;
                if im.w <= 0.0 || im.h <= 0.0 {
                    return Err(Error::option("an image needs a positive width and height"));
                }
                let bytes = image_inputs.get(im.image).ok_or_else(|| {
                    Error::option(format!(
                        "image {} was not given ({} images)",
                        im.image + 1,
                        image_inputs.len()
                    ))
                })?;
                let (xobj, _, _) = match embedded.get(&im.image) {
                    Some(e) => *e,
                    None => {
                        let e = images::embed_image(&mut doc, bytes)?;
                        embedded.insert(im.image, e);
                        e
                    }
                };
                let name =
                    pdf::add_resource(&mut doc, page, b"XObject", "PVIm", Object::Reference(xobj))?;
                out.extend(b"q\n");
                out.extend(alpha(&mut doc, im.opacity)?.into_bytes());
                out.extend(cm(&[im.w, 0.0, 0.0, im.h, im.x, im.y]).into_bytes());
                out.extend(format!("/{} Do\nQ\n", String::from_utf8_lossy(&name)).into_bytes());
            }
            Item::Note(nt) => {
                finite(&[nt.x, nt.y])?;
                let (frame, _, _) = stamp::visual_frame(&doc, page);
                let annot = note_annot(&mut doc, page, &frame, nt)?;
                push_annot(&mut doc, page, annot)?;
                report.annotations += 1;
                continue;
            }
            Item::Highlight(h) => {
                finite(&[h.x, h.y, h.w, h.h])?;
                let (frame, _, _) = stamp::visual_frame(&doc, page);
                let annot = highlight_annot(&mut doc, page, &frame, h)?;
                push_annot(&mut doc, page, annot)?;
                report.annotations += 1;
                continue;
            }
            Item::Redact(_) => continue,
        }
        report.drawn += 1;
        ops.entry(p).or_default().extend(out);
    }
    for (p, body) in ops {
        let id = ids[p];
        let (frame, _, _) = stamp::visual_frame(&doc, id);
        let mut content = b"q\n".to_vec();
        content.extend(cm(&frame).into_bytes());
        content.extend(body);
        content.extend(b"Q\n");
        pdf::append_content(&mut doc, id, content)?;
    }
    let out = pdf::save(&mut doc)?;
    if o.flatten_fields {
        return Ok((forms::flatten(&out, None)?.0, report));
    }
    Ok((out, report))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures;
    use crate::testutil::qpdf_check;

    fn parse(json: &str) -> EditOptions {
        serde_json::from_str(json).unwrap()
    }

    fn text_of(bytes: &[u8], page: u32) -> String {
        pdf::load(bytes, None)
            .unwrap()
            .extract_text(&[page])
            .unwrap_or_default()
    }

    #[test]
    fn draws_every_kind_of_item() {
        let sig = fixtures::png(&image::DynamicImage::ImageRgba8(fixtures::with_alpha(
            &fixtures::line_art(200, 80),
        )));
        let o = parse(
            r##"{ "items": [
                { "type": "text", "page": 0, "x": 72, "y": 700, "text": "Hello\nsecond line", "size": 18, "color": "#1a73e8", "bold": true },
                { "type": "rect", "page": 0, "x": 60, "y": 60, "w": 200, "h": 40, "fill": "#ffffff" },
                { "type": "rect", "page": 1, "x": 60, "y": 60, "w": -50, "h": 40, "stroke": "#ff0000", "lineWidth": 2, "opacity": 0.5 },
                { "type": "ink", "page": 1, "paths": [[10, 10, 50, 60, 90, 20], [200, 200]], "color": "#000000" },
                { "type": "image", "page": 1, "x": 300, "y": 100, "w": 150, "h": 60, "image": 0 },
                { "type": "image", "page": 0, "x": 300, "y": 100, "w": 150, "h": 60, "image": 0, "opacity": 0.8 },
                { "type": "note", "page": 0, "x": 500, "y": 750, "text": "Vérifier ce point" },
                { "type": "highlight", "page": 1, "x": 72, "y": 690, "w": 200, "h": 16 },
                { "type": "redact", "page": 1, "x": 0, "y": 0, "w": 10, "h": 10 }
            ] }"##,
        );
        let (out, rep) = edit(&[fixtures::text_pdf(2), sig], &o).unwrap();
        assert_eq!((rep.drawn, rep.annotations), (6, 2));
        qpdf_check(&out);
        let t = text_of(&out, 1);
        assert!(t.contains("Hello") && t.contains("second line"), "{t}");
        let doc = pdf::load(&out, None).unwrap();
        let ids = pdf::page_ids(&doc);
        let subtypes = |p: ObjectId| -> Vec<Vec<u8>> {
            let annots = doc
                .get_dictionary(p)
                .unwrap()
                .get(b"Annots")
                .unwrap()
                .as_array()
                .unwrap();
            annots
                .iter()
                .map(|a| {
                    let d = doc.get_dictionary(a.as_reference().unwrap()).unwrap();
                    d.get(b"Subtype").unwrap().as_name().unwrap().to_vec()
                })
                .collect()
        };
        assert_eq!(subtypes(ids[0]), vec![b"Text".to_vec()]);
        assert_eq!(subtypes(ids[1]), vec![b"Highlight".to_vec()]);
        // The image is embedded once and shared by both pages.
        let images = doc
            .objects
            .values()
            .filter_map(|o| o.as_stream().ok())
            .filter(|s| s.dict.get(b"Subtype").and_then(Object::as_name).ok() == Some(b"Image"))
            .count();
        assert_eq!(images, 2, "one image plus its soft mask");
    }

    #[test]
    fn blanks_and_out_of_range() {
        let o = parse(
            r#"{ "blanks": [{ "size": "a4" }, {}],
                 "items": [{ "type": "text", "page": 3, "x": 50, "y": 800, "text": "On a blank page" }] }"#,
        );
        let (out, rep) = edit(&[fixtures::text_pdf(2)], &o).unwrap();
        assert_eq!((rep.pages, rep.blanks), (4, 2));
        qpdf_check(&out);
        assert!(text_of(&out, 4).contains("On a blank page"));
        let doc = pdf::load(&out, None).unwrap();
        let a4 = pdf::page_box(&doc, pdf::page_ids(&doc)[2]);
        assert!((a4[2] - 595.28).abs() < 0.01);

        let bad = parse(r#"{ "items": [{ "type": "text", "page": 5, "text": "x" }] }"#);
        assert!(edit(&[fixtures::text_pdf(2)], &bad).is_err());
        let no_img = parse(r#"{ "items": [{ "type": "image", "page": 0, "w": 10, "h": 10 }] }"#);
        assert!(edit(&[fixtures::text_pdf(1)], &no_img).is_err());
    }

    #[test]
    fn fills_and_flattens_forms() {
        let o = parse(
            r#"{ "fields": { "name": "Grace Hopper", "agree": false }, "flattenFields": true }"#,
        );
        let (out, rep) = edit(&[fixtures::form_pdf()], &o).unwrap();
        assert_eq!(rep.fields, 2);
        qpdf_check(&out);
        let doc = pdf::load(&out, None).unwrap();
        assert!(!doc.catalog().unwrap().has(b"AcroForm"));
        assert!(fixtures::any_stream_contains(&out, b"(Grace Hopper) Tj"));
    }

    #[test]
    fn rotated_page_items_follow_the_display() {
        let rotated = crate::pages::rotate(&fixtures::text_pdf(1), 90, None, None).unwrap();
        let o = parse(
            r#"{ "items": [
                { "type": "text", "page": 0, "x": 40, "y": 580, "text": "Upright" },
                { "type": "highlight", "page": 0, "x": 40, "y": 40, "w": 100, "h": 20 }
            ] }"#,
        );
        let (out, _) = edit(&[rotated], &o).unwrap();
        qpdf_check(&out);
        let doc = pdf::load(&out, None).unwrap();
        let page = pdf::page_ids(&doc)[0];
        let annot = doc
            .get_dictionary(page)
            .unwrap()
            .get(b"Annots")
            .unwrap()
            .as_array()
            .unwrap()[0]
            .as_reference()
            .unwrap();
        let r: Vec<f32> = doc
            .get_dictionary(annot)
            .unwrap()
            .get(b"Rect")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .filter_map(pdf::num)
            .collect();
        // 90° clockwise: visual x runs up the user-space y axis.
        assert_eq!(r, vec![552.0, 40.0, 572.0, 140.0]);
    }
}
