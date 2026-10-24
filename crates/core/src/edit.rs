//! The PDF editor's single write: append blank pages, fill form fields,
//! change existing text, redact, draw text, shapes, freehand ink and images
//! onto pages, and add comments (note and highlight annotations).
//!
//! Coordinates are in the page's visual space, like the image stamp: points,
//! origin at the bottom-left of the page as it is displayed (after /Rotate).
//! `page` is a 0-based index into the input's pages followed by the blank
//! pages this call appends, so items stay on their page whatever order
//! [`crate::pages::arrange`] puts the pages in afterwards.
//!
//! Content goes first: the glyphs a `replace` item rewrites and whatever lies
//! under `redact` boxes are taken out of the page's content streams
//! ([`crate::scrub`]); then everything is drawn; redaction boxes come last,
//! on top. A page whose redaction cannot be done in the content with
//! certainty is listed in [`EditReport::flatten`] for the renderer to turn
//! into an image (`pdfvault-render`'s `edit` does that and double-checks the
//! others).
//!
//! Text is written with embedded font subsets (feature `typeset`): the
//! bundled Liberation fonts, or font files given as attachments. A `replace`
//! item keeps the original font when it has every character.

use crate::error::{Error, Result};
use crate::forms::{self, FieldValue};
use crate::images;
use crate::interp::{self, Page};
use crate::pdf::{self, n};
use crate::pdffont::Family;
use crate::redact;
use crate::scrub::{self, Removal};
use crate::stamp::{self, cm, parse_color};
use crate::textedit;
use lopdf::{Dictionary, Document, Object, ObjectId, Stream, dictionary};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// First baseline of a text item sits this many font sizes below its top.
pub const TEXT_ASCENT: f32 = 0.8;
/// Distance between baselines, in font sizes.
pub const TEXT_LINE: f32 = 1.2;
/// Side of a note's icon, in points.
pub const NOTE_SIZE: f32 = 20.0;

#[derive(Debug, Deserialize, Default, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Redaction {
    /// Take what is under the boxes out of the content; the rest of the page
    /// stays as it was, text included.
    #[default]
    Remove,
    /// Turn every redacted page into an image.
    Flatten,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct EditOptions {
    pub password: Option<String>,
    pub blanks: Vec<Blank>,
    pub fields: BTreeMap<String, FieldValue>,
    /// Burn the form into the pages once filled.
    pub flatten_fields: bool,
    pub items: Vec<Item>,
    pub redaction: Redaction,
    /// Resolution pages turned into images are rendered at (renderer only).
    pub redact_dpi: Option<f32>,
    /// Bundled fonts ("sans-regular", "serif-bold"…) by attachment index; the
    /// engine fills this in from [`plan`].
    pub bundled: BTreeMap<String, usize>,
    /// Fonts tried in order for characters a text's own font lacks.
    pub fallback_fonts: Vec<FontRef>,
    /// Complete copies of the document's fonts (from this device, say): a
    /// changed line whose embedded subset lacks a character is redrawn in
    /// the copy whose PostScript name matches its font.
    pub substitutes: Vec<Substitute>,
}

/// A font file among the attachments standing for the PDF font `name`
/// (its PostScript name, as in "Georgia-Bold"), which also picks the face
/// in a collection.
#[derive(Debug, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Substitute {
    pub name: String,
    pub file: usize,
}

/// Font names compared the way PDF producers spell them differently.
fn same_name(a: &str, b: &str) -> bool {
    let key = |s: &str| -> String {
        s.chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .map(|c| c.to_ascii_lowercase())
            .collect()
    };
    key(a) == key(b)
}

/// A font file among the attachments; `name` picks a face in a collection.
#[derive(Debug, Deserialize, Clone, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub struct FontRef {
    pub file: usize,
    #[serde(default)]
    pub name: Option<String>,
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
    Replace(ReplaceItem),
    Rect(RectItem),
    Ink(InkItem),
    Image(ImageItem),
    Note(NoteItem),
    Highlight(BoxItem),
    Redact(BoxItem),
    Object(ObjectItem),
}

impl Item {
    pub fn page(&self) -> usize {
        match self {
            Item::Text(i) => i.page,
            Item::Replace(i) => i.page,
            Item::Rect(i) => i.page,
            Item::Ink(i) => i.page,
            Item::Image(i) => i.page,
            Item::Note(i) => i.page,
            Item::Highlight(i) | Item::Redact(i) => i.page,
            Item::Object(i) => i.page,
        }
    }
}

/// Lines of text. `x`, `y` is the top-left corner of the text box.
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase", default)]
pub struct TextItem {
    pub page: usize,
    pub x: f32,
    pub y: f32,
    pub size: f32,
    pub color: String,
    /// "sans", "serif" or "mono" (the bundled Liberation fonts).
    pub font: String,
    pub bold: bool,
    pub italic: bool,
    /// A font file among the attachments, instead of `font`.
    pub font_file: Option<FontRef>,
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
            font: "sans".into(),
            bold: false,
            italic: false,
            font_file: None,
            text: String::new(),
        }
    }
}

/// Existing text rewritten: the glyphs whose centre lies in the box (`x`,
/// `y` its bottom-left corner) go, and `text` is written from `start` on
/// `baseline`. Unset values come from the glyphs removed. With `find`
/// instead of a box, the first line of the page containing it is rewritten
/// with `find` replaced by `text`.
///
/// A paragraph (from `text-runs`) names its `glyphs` and, for each character
/// of `text`, the glyph whose font, size and colour it keeps (`sources`);
/// with `right`, the text reflows between `left` and `right`, `leading`
/// apart, aligned by `align`.
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase", default)]
pub struct ReplaceItem {
    pub page: usize,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub text: String,
    pub find: Option<String>,
    pub start: Option<f32>,
    pub baseline: Option<f32>,
    pub size: Option<f32>,
    pub color: Option<String>,
    /// "auto" (the original font when it has every character, else the
    /// closest bundled one), "sans", "serif" or "mono".
    pub font: String,
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub font_file: Option<FontRef>,
    /// Horizontal scaling, percent.
    pub hscale: Option<f32>,
    /// The page glyphs to take out, instead of those in the box.
    pub glyphs: Option<Vec<usize>>,
    /// Per character of `text`: the glyph whose style it takes (-1: the
    /// previous character's).
    pub sources: Option<Vec<i64>>,
    /// Reflow: lines after the first start at `left` and wrap at `right`.
    pub left: Option<f32>,
    pub right: Option<f32>,
    pub leading: Option<f32>,
    /// "left", "right", "center" or "justify".
    pub align: Option<String>,
}

impl Default for ReplaceItem {
    fn default() -> Self {
        ReplaceItem {
            page: 0,
            x: 0.0,
            y: 0.0,
            w: 0.0,
            h: 0.0,
            text: String::new(),
            find: None,
            start: None,
            baseline: None,
            size: None,
            color: None,
            font: "auto".into(),
            bold: None,
            italic: None,
            font_file: None,
            hscale: None,
            glyphs: None,
            sources: None,
            left: None,
            right: None,
            leading: None,
            align: None,
        }
    }
}

impl ReplaceItem {
    pub fn bounds(&self) -> [f32; 4] {
        norm(self.x, self.y, self.w, self.h)
    }

    /// Written character by character with inherited styles, maybe reflowed.
    fn paragraph(&self) -> bool {
        self.sources.is_some() || self.right.is_some()
    }
}

/// An image, drawing or shape the page already has (`kind` and `index` as
/// `text-runs` lists them): deleted, or moved by `dx`, `dy` and scaled by
/// `sx`, `sy` about the bottom-left corner of its box.
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase", default)]
pub struct ObjectItem {
    pub page: usize,
    /// "image", "form" or "path".
    pub kind: String,
    pub index: usize,
    pub delete: bool,
    pub dx: f32,
    pub dy: f32,
    pub sx: f32,
    pub sy: f32,
}

impl Default for ObjectItem {
    fn default() -> Self {
        ObjectItem {
            page: 0,
            kind: "image".into(),
            index: 0,
            delete: false,
            dx: 0.0,
            dy: 0.0,
            sx: 1.0,
            sy: 1.0,
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

/// An image attachment drawn into a box; `image` counts the attachments after
/// the PDF from 0. `x`, `y` is the bottom-left corner.
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
    /// Lines of existing text rewritten; how many kept their own font
    /// entirely, how many needed another font for some characters, and how
    /// many were redrawn in a complete copy of their font.
    pub replaced: usize,
    pub same_font: usize,
    pub mixed: usize,
    pub substituted: usize,
    /// Rewritten lines whose old glyphs could not be taken out, only covered.
    pub covered: usize,
    /// Images, drawings and shapes of the page moved, resized or deleted.
    pub objects: usize,
    /// Rewritten paragraphs now reaching lower than they did: they may run
    /// into what follows, since nothing below moves.
    pub overflow: usize,
    /// Pages redacted in their content (1-based).
    pub redacted_pages: Vec<usize>,
    /// Pages left for the renderer to turn into images (0-based).
    pub flatten: Vec<usize>,
}

/// Which bundled fonts an edit needs, so the caller can pass just those, and
/// which of the document's fonts lack characters the edit writes: a complete
/// copy of one, given as a substitute, keeps its line in that font.
#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct EditPlan {
    pub fonts: Vec<String>,
    pub wanted: Vec<WantedFont>,
}

#[derive(Debug, Serialize, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
pub struct WantedFont {
    /// PostScript name, without the subset tag ("Georgia-Bold").
    pub name: String,
    /// Family as installed ("Georgia").
    pub family: String,
    pub bold: bool,
    pub italic: bool,
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

fn check_size(size: f32) -> Result<()> {
    if (1.0..=500.0).contains(&size) {
        Ok(())
    } else {
        Err(Error::option("font size must be between 1 and 500"))
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

/// A stretch of a rewritten line: codes in the page's own font, or text for
/// another font when that one lacks the characters.
#[derive(Debug, Clone)]
enum Piece {
    Codes(Vec<(u32, usize)>),
    Text(String),
}

/// How a piece of text gets its font.
#[derive(Debug, Clone)]
#[cfg_attr(not(feature = "typeset"), allow(dead_code))]
enum Choice {
    /// The page font that already draws the text being replaced, line by
    /// line; what it lacks is written with `fallback`.
    Original {
        font: usize,
        lines: Vec<Vec<Piece>>,
        fallback: Option<Box<Choice>>,
    },
    Bundled(String),
    /// A font file; the bundled `backup` catches characters it lacks.
    File(FontRef, Option<String>),
    /// A complete copy of the page's font, with a bundled backup.
    Substitute(FontRef, String),
}

impl Choice {
    /// The bundled fonts this choice may draw with.
    fn bundled(&self) -> Vec<String> {
        match self {
            Choice::Bundled(k) | Choice::Substitute(_, k) | Choice::File(_, Some(k)) => {
                vec![k.clone()]
            }
            Choice::Original {
                fallback: Some(f), ..
            } => f.bundled(),
            _ => vec![],
        }
    }
}

/// A line split where `font` lacks a character. `None` when it has none of
/// them (spaces aside).
fn pieces(page: &Page, font: usize, line: &str) -> Option<Vec<Piece>> {
    let codes = textedit::codes_for(page, font, line)?;
    let mut out: Vec<Piece> = vec![];
    let mut any = false;
    for (c, code) in line.chars().zip(codes) {
        match (code, out.last_mut()) {
            (Some(code), Some(Piece::Codes(v))) => v.push(code),
            (Some(code), _) => out.push(Piece::Codes(vec![code])),
            (None, Some(Piece::Text(t))) => t.push(c),
            (None, _) => out.push(Piece::Text(c.to_string())),
        }
        any |= code.is_some() && !c.is_whitespace();
    }
    (any || line.trim().is_empty()).then_some(out)
}

fn text_choice(it: &TextItem) -> Result<Choice> {
    if let Some(f) = &it.font_file {
        return Ok(Choice::File(f.clone(), None));
    }
    let family = textedit::parse_family(&it.font).ok_or_else(|| {
        Error::option(format!("unknown font '{}' (sans, serif or mono)", it.font))
    })?;
    Ok(Choice::Bundled(textedit::bundled_key(
        family, it.bold, it.italic,
    )))
}

/// A replace item with its box: given, or the line holding `find`.
fn locate(page: &Page, index: usize, it: &ReplaceItem) -> Result<ReplaceItem> {
    let Some(find) = it.find.as_deref().filter(|f| !f.is_empty()) else {
        return Ok(it.clone());
    };
    let run = textedit::runs_of(page, index)
        .into_iter()
        .find(|r| r.editable && r.text.contains(find))
        .ok_or_else(|| {
            Error::option(format!("no line of page {} contains \"{find}\"", index + 1))
        })?;
    Ok(ReplaceItem {
        x: run.x,
        y: run.y,
        w: run.w,
        h: run.h,
        text: run.text.replacen(find, &it.text, 1),
        find: None,
        start: it.start.or(Some(run.start)),
        baseline: it.baseline.or(Some(run.baseline)),
        ..it.clone()
    })
}

/// The replace item placed, the glyphs it takes out, and the font it writes with.
fn replace_choice(
    page: &Page,
    index: usize,
    it: &ReplaceItem,
    subs: &[Substitute],
) -> Result<(ReplaceItem, Vec<usize>, Choice)> {
    let it = &locate(page, index, it)?;
    let (glyphs, choice) = choose(page, it, subs)?;
    Ok((it.clone(), glyphs, choice))
}

/// The glyphs a replace item takes out: those it names, or those in its box.
fn replaced_glyphs(page: &Page, it: &ReplaceItem) -> Vec<usize> {
    match &it.glyphs {
        Some(list) => {
            let mut v: Vec<usize> = list
                .iter()
                .copied()
                .filter(|&g| g < page.glyphs.len())
                .collect();
            v.sort_unstable();
            v.dedup();
            v
        }
        None => textedit::glyphs_in(page, it.bounds()),
    }
}

/// How `text` in page font `font` is written, wanting `bold` and `italic`:
/// in that font (or a sibling of its family on the page with the wanted
/// style), the characters it lacks from an installed copy or a bundled font.
fn font_choice(
    page: &Page,
    font: usize,
    bold: bool,
    italic: bool,
    text: &str,
    subs: &[Substitute],
) -> Choice {
    let f = &page.fonts[font];
    let font = if f.bold == bold && f.italic == italic {
        Some(font)
    } else {
        page.fonts.iter().position(|g| {
            g.bold == bold && g.italic == italic && same_name(&g.family_name, &f.family_name)
        })
    };
    let key = textedit::bundled_key(f.family, bold, italic);
    let Some(font) = font else {
        return Choice::Bundled(key);
    };
    let Some(pieces) = pieces(page, font, text) else {
        return Choice::Bundled(key);
    };
    if pieces.iter().all(|p| matches!(p, Piece::Codes(_))) {
        return Choice::Original {
            font,
            lines: vec![pieces],
            fallback: None,
        };
    }
    if let Some(s) = subs
        .iter()
        .find(|s| same_name(&s.name, &page.fonts[font].base))
    {
        return Choice::Substitute(
            FontRef {
                file: s.file,
                name: Some(s.name.clone()),
            },
            key,
        );
    }
    Choice::Original {
        font,
        lines: vec![pieces],
        fallback: Some(Box::new(Choice::Bundled(key))),
    }
}

fn choose(page: &Page, it: &ReplaceItem, subs: &[Substitute]) -> Result<(Vec<usize>, Choice)> {
    let glyphs = replaced_glyphs(page, it);
    if let Some(f) = &it.font_file {
        return Ok((glyphs, Choice::File(f.clone(), None)));
    }
    let original = textedit::dominant_font(page, &glyphs);
    let base = original.map(|f| &page.fonts[f]);
    let style = |family: Family| {
        Choice::Bundled(textedit::bundled_key(
            family,
            it.bold.unwrap_or(base.is_some_and(|b| b.bold)),
            it.italic.unwrap_or(base.is_some_and(|b| b.italic)),
        ))
    };
    if it.font != "auto" {
        let family = textedit::parse_family(&it.font).ok_or_else(|| {
            Error::option(format!(
                "unknown font '{}' (auto, sans, serif or mono)",
                it.font
            ))
        })?;
        return Ok((glyphs, style(family)));
    }
    let backup = || style(base.map(|b| b.family).unwrap_or(Family::Sans));
    // The original font, unless the item asks for another weight or slant.
    if let Some(f) = original {
        let same_style = it.bold.is_none_or(|b| b == page.fonts[f].bold)
            && it.italic.is_none_or(|i| i == page.fonts[f].italic);
        let lines: Option<Vec<Vec<Piece>>> = same_style
            .then(|| it.text.lines().map(|l| pieces(page, f, l)).collect())
            .flatten();
        if let Some(lines) = lines {
            let complete = lines.iter().flatten().all(|p| matches!(p, Piece::Codes(_)));
            if complete {
                return Ok((
                    glyphs,
                    Choice::Original {
                        font: f,
                        lines,
                        fallback: None,
                    },
                ));
            }
            // A complete copy of the font redraws the whole line in it;
            // otherwise only the missing characters change font.
            let key = match backup() {
                Choice::Bundled(k) => k,
                _ => unreachable!("styles are bundled fonts"),
            };
            if let Some(s) = subs
                .iter()
                .find(|s| same_name(&s.name, &page.fonts[f].base))
            {
                let r = FontRef {
                    file: s.file,
                    name: Some(s.name.clone()),
                };
                return Ok((glyphs, Choice::Substitute(r, key)));
            }
            return Ok((
                glyphs,
                Choice::Original {
                    font: f,
                    lines,
                    fallback: Some(Box::new(Choice::Bundled(key))),
                },
            ));
        }
    }
    Ok((glyphs, backup()))
}

/// One character's style in a rewritten paragraph.
#[derive(Debug, Clone, Copy, PartialEq)]
struct CharStyle {
    font: usize,
    size: f32,
    color: [f32; 3],
    hscale: f32,
    bold: bool,
    italic: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum FragKind {
    Word,
    Space,
    Break,
}

/// A stretch of one style between spaces, with the font it is written in.
#[derive(Debug, Clone)]
struct Frag {
    kind: FragKind,
    text: String,
    style: CharStyle,
    choice: Choice,
}

/// What a replace item writes: one style per line, or a paragraph of styled pieces.
#[derive(Debug, Clone)]
enum Rewrite {
    Line(Choice),
    Paragraph(Vec<Frag>),
}

impl Rewrite {
    fn choices(&self) -> Vec<&Choice> {
        match self {
            Rewrite::Line(c) => vec![c],
            Rewrite::Paragraph(frags) => frags.iter().map(|f| &f.choice).collect(),
        }
    }
}

/// The style of each character of a paragraph's new text.
fn char_styles(page: &Page, it: &ReplaceItem, glyphs: &[usize]) -> Result<Vec<CharStyle>> {
    let first = glyphs
        .first()
        .map(|&g| &page.glyphs[g])
        .ok_or_else(|| Error::option("the paragraph has no glyphs to rewrite"))?;
    let of = |g: &crate::interp::Glyph| CharStyle {
        font: g.font,
        size: g.size,
        color: g.color,
        hscale: g.hscale,
        bold: page.fonts[g.font].bold,
        italic: page.fonts[g.font].italic,
    };
    let base = of(first);
    let sources = it.sources.clone().unwrap_or_default();
    let color = it.color.as_deref().map(parse_color).transpose()?;
    let scale = it.size.map(|s| s / base.size).unwrap_or(1.0);
    let mut out = Vec::with_capacity(it.text.chars().count());
    let mut prev = base;
    for (i, _) in it.text.chars().enumerate() {
        let mut st = match sources.get(i) {
            Some(&s) if s >= 0 && (s as usize) < page.glyphs.len() => of(&page.glyphs[s as usize]),
            _ => prev,
        };
        prev = st;
        st.size *= scale;
        if let Some(c) = color {
            st.color = c;
        }
        if let Some(h) = it.hscale {
            st.hscale = h;
        }
        st.bold = it.bold.unwrap_or(st.bold);
        st.italic = it.italic.unwrap_or(st.italic);
        out.push(st);
    }
    Ok(out)
}

/// A paragraph's text cut into words, spaces and line breaks of one style each.
fn fragments(
    page: &Page,
    it: &ReplaceItem,
    glyphs: &[usize],
    subs: &[Substitute],
) -> Result<Vec<Frag>> {
    let styles = char_styles(page, it, glyphs)?;
    let explicit = match it.font.as_str() {
        "auto" => None,
        f => Some(textedit::parse_family(f).ok_or_else(|| {
            Error::option(format!("unknown font '{f}' (auto, sans, serif or mono)"))
        })?),
    };
    let mut out: Vec<Frag> = vec![];
    for (c, st) in it.text.chars().zip(styles) {
        let kind = match c {
            '\n' => FragKind::Break,
            c if c.is_whitespace() => FragKind::Space,
            _ => FragKind::Word,
        };
        match out.last_mut() {
            Some(last) if last.kind == kind && kind != FragKind::Break && last.style == st => {
                last.text.push(c)
            }
            _ => out.push(Frag {
                kind,
                text: c.to_string(),
                style: st,
                choice: Choice::Bundled(String::new()),
            }),
        }
    }
    for f in &mut out {
        f.choice = if let Some(file) = &it.font_file {
            Choice::File(file.clone(), None)
        } else if let Some(family) = explicit {
            Choice::Bundled(textedit::bundled_key(family, f.style.bold, f.style.italic))
        } else if f.kind == FragKind::Break {
            Choice::Bundled(textedit::bundled_key(
                page.fonts[f.style.font].family,
                f.style.bold,
                f.style.italic,
            ))
        } else {
            font_choice(
                page,
                f.style.font,
                f.style.bold,
                f.style.italic,
                &f.text,
                subs,
            )
        };
    }
    Ok(out)
}

/// What a replace item takes out and writes.
fn rewrite(
    page: &Page,
    index: usize,
    it: &ReplaceItem,
    subs: &[Substitute],
) -> Result<(ReplaceItem, Vec<usize>, Rewrite)> {
    if !it.paragraph() {
        let (placed, glyphs, choice) = replace_choice(page, index, it, subs)?;
        return Ok((placed, glyphs, Rewrite::Line(choice)));
    }
    let glyphs = replaced_glyphs(page, it);
    let frags = fragments(page, it, &glyphs, subs)?;
    Ok((it.clone(), glyphs, Rewrite::Paragraph(frags)))
}

/// What moving, resizing or deleting one of the page's objects does to its content.
fn object_edit(page: &Page, ob: &ObjectItem, r: &mut Removal) -> Result<()> {
    let missing = || {
        Error::option(format!(
            "page {} has no {} {}",
            ob.page + 1,
            ob.kind,
            ob.index
        ))
    };
    let (stream, first, last, ctm, bbox) = match ob.kind.as_str() {
        "image" => {
            let im = page.images.get(ob.index).ok_or_else(missing)?;
            if ob.delete {
                r.images.insert(ob.index);
                return Ok(());
            }
            (im.stream, im.op, im.op, im.matrix, im.bbox)
        }
        "form" => {
            let f = page.forms.get(ob.index).ok_or_else(missing)?;
            if ob.delete {
                r.forms.insert(ob.index);
                return Ok(());
            }
            (f.stream, f.op, f.op, f.ctm, f.bbox)
        }
        "path" => {
            let pu = page.paths.get(ob.index).ok_or_else(missing)?;
            if ob.delete {
                r.paths.insert(ob.index);
                return Ok(());
            }
            (pu.stream, pu.first, pu.paint, pu.ctm, pu.bbox)
        }
        other => {
            return Err(Error::option(format!(
                "unknown object kind '{other}' (image, form or path)"
            )));
        }
    };
    // In page space: scale about the box's corner, then move. In the user
    // space the operation runs in, that is ctm · t · ctm⁻¹.
    let (x0, y0) = (bbox[0], bbox[1]);
    let t = [
        ob.sx,
        0.0,
        0.0,
        ob.sy,
        x0 - x0 * ob.sx + ob.dx,
        y0 - y0 * ob.sy + ob.dy,
    ];
    let inv = interp::invert(&ctm)
        .ok_or_else(|| Error::Unsupported("a flattened object cannot move".into()))?;
    let m = interp::mul(&interp::mul(&ctm, &t), &inv);
    r.transforms.push((stream, first, last, m));
    Ok(())
}

fn page_of(ids: &[ObjectId], p: usize) -> Result<ObjectId> {
    ids.get(p).copied().ok_or_else(|| {
        Error::option(format!(
            "page {} does not exist ({} pages)",
            p + 1,
            ids.len()
        ))
    })
}

/// The bundled fonts `o` needs (to be passed as attachments, listed in
/// [`EditOptions::bundled`]).
pub fn plan(pdf_bytes: &[u8], o: &EditOptions) -> Result<EditPlan> {
    let mut doc = pdf::load(pdf_bytes, o.password.as_deref())?;
    append_blanks(&mut doc, &o.blanks)?;
    let ids = pdf::page_ids(&doc);
    let mut pages: HashMap<usize, Page> = HashMap::new();
    let mut fonts = BTreeSet::new();
    let mut wanted = BTreeSet::new();
    for item in &o.items {
        let choice = match item {
            Item::Text(t) if !t.text.trim().is_empty() => text_choice(t)?,
            Item::Replace(r) => {
                let id = page_of(&ids, r.page)?;
                let page = pages
                    .entry(r.page)
                    .or_insert_with(|| interp::interpret(&doc, id));
                let (_, _, written) = rewrite(page, r.page, r, &o.substitutes)?;
                for choice in written.choices() {
                    if let Choice::Original {
                        font,
                        fallback: Some(_),
                        ..
                    } = choice
                    {
                        let f = &page.fonts[*font];
                        wanted.insert(WantedFont {
                            name: f.base.clone(),
                            family: f.family_name.clone(),
                            bold: f.bold,
                            italic: f.italic,
                        });
                    }
                    fonts.extend(choice.bundled());
                }
                continue;
            }
            _ => continue,
        };
        fonts.extend(choice.bundled());
    }
    Ok(EditPlan {
        fonts: fonts.into_iter().collect(),
        wanted: wanted.into_iter().collect(),
    })
}

#[cfg(feature = "typeset")]
struct Fonts<'a> {
    ts: crate::typeset::Typesetter<'a>,
    slots: HashMap<FontRef, usize>,
    attachments: &'a [Vec<u8>],
    bundled: &'a BTreeMap<String, usize>,
    fallbacks: Vec<usize>,
}

#[cfg(feature = "typeset")]
impl<'a> Fonts<'a> {
    fn new(attachments: &'a [Vec<u8>], o: &'a EditOptions) -> Result<Self> {
        let mut f = Fonts {
            ts: crate::typeset::Typesetter::new(),
            slots: HashMap::new(),
            attachments,
            bundled: &o.bundled,
            fallbacks: vec![],
        };
        for r in &o.fallback_fonts {
            let slot = f.slot(r)?;
            f.fallbacks.push(slot);
        }
        Ok(f)
    }

    fn slot(&mut self, r: &FontRef) -> Result<usize> {
        if let Some(&s) = self.slots.get(r) {
            return Ok(s);
        }
        let data = self.attachments.get(r.file).ok_or_else(|| {
            Error::option(format!(
                "font file {} was not given ({} attachments)",
                r.file + 1,
                self.attachments.len()
            ))
        })?;
        let index = r
            .name
            .as_deref()
            .map(|nm| crate::typeset::face_index(data, nm))
            .unwrap_or(0);
        let s = self.ts.add_face(data, index)?;
        self.slots.insert(r.clone(), s);
        Ok(s)
    }

    /// The faces to try for `choice`, in order.
    fn chain(&mut self, choice: &Choice) -> Result<Vec<usize>> {
        let (primary, backup) = match choice {
            Choice::File(r, backup) => (self.slot(r)?, backup.clone()),
            Choice::Substitute(r, backup) => (self.slot(r)?, Some(backup.clone())),
            Choice::Bundled(k) => (self.bundled_slot(k)?, None),
            Choice::Original { .. } => unreachable!("original fonts are not typeset"),
        };
        let mut chain = vec![primary];
        chain.extend(self.fallbacks.iter().copied().filter(|s| *s != primary));
        if let Some(k) = backup {
            let s = self.bundled_slot(&k)?;
            if !chain.contains(&s) {
                chain.push(s);
            }
        }
        Ok(chain)
    }

    fn bundled_slot(&mut self, key: &str) -> Result<usize> {
        let file = *self
            .bundled
            .get(key)
            .ok_or_else(|| Error::option(format!("the bundled font {key} was not given")))?;
        self.slot(&FontRef { file, name: None })
    }

    /// The width of `text` written with `choice`.
    fn measure(&mut self, choice: &Choice, text: &str, size: f32) -> Result<f32> {
        let chain = self.chain(choice)?;
        Ok(self.ts.layout(&chain, text, size)?.width)
    }

    /// One stretch of text drawn from (`x`, `y`); returns the operators and its width.
    #[allow(clippy::too_many_arguments)]
    fn piece(
        &mut self,
        doc: &mut Document,
        page: ObjectId,
        choice: &Choice,
        text: &str,
        x: f32,
        y: f32,
        size: f32,
    ) -> Result<(Vec<u8>, f32)> {
        let chain = self.chain(choice)?;
        let laid = self.ts.layout(&chain, text, size)?;
        Ok((self.ts.draw(doc, page, &laid, x, y, size)?, laid.width))
    }

    #[allow(clippy::too_many_arguments)]
    fn lines(
        &mut self,
        doc: &mut Document,
        page: ObjectId,
        choice: &Choice,
        text: &str,
        x: f32,
        first_baseline: f32,
        size: f32,
    ) -> Result<Vec<u8>> {
        let chain = self.chain(choice)?;
        let mut out = vec![];
        for (i, line) in text.lines().enumerate() {
            let laid = self.ts.layout(&chain, line, size)?;
            let y = first_baseline - size * TEXT_LINE * i as f32;
            out.extend(self.ts.draw(doc, page, &laid, x, y, size)?);
        }
        Ok(out)
    }

    fn finish(self, doc: &mut Document) -> Result<()> {
        self.ts.finish(doc)
    }
}

#[cfg(not(feature = "typeset"))]
struct Fonts;

#[cfg(not(feature = "typeset"))]
impl Fonts {
    fn new(_: &[Vec<u8>], _: &EditOptions) -> Result<Self> {
        Ok(Fonts)
    }

    #[allow(clippy::too_many_arguments)]
    fn lines(
        &mut self,
        _: &mut Document,
        _: ObjectId,
        _: &Choice,
        _: &str,
        _: f32,
        _: f32,
        _: f32,
    ) -> Result<Vec<u8>> {
        Err(Error::Unsupported(
            "writing text needs the typeset feature".into(),
        ))
    }

    fn measure(&mut self, _: &Choice, _: &str, _: f32) -> Result<f32> {
        Err(Error::Unsupported(
            "writing text needs the typeset feature".into(),
        ))
    }

    #[allow(clippy::too_many_arguments)]
    fn piece(
        &mut self,
        _: &mut Document,
        _: ObjectId,
        _: &Choice,
        _: &str,
        _: f32,
        _: f32,
        _: f32,
    ) -> Result<(Vec<u8>, f32)> {
        Err(Error::Unsupported(
            "writing text needs the typeset feature".into(),
        ))
    }

    fn finish(self, _: &mut Document) -> Result<()> {
        Ok(())
    }
}

/// Text in a font the page already has, from codes that font maps, with the
/// characters it lacks written by `fallback`.
#[allow(clippy::too_many_arguments)]
fn original_lines(
    doc: &mut Document,
    page: ObjectId,
    info: &Page,
    font: usize,
    lines: &[Vec<Piece>],
    fallback: Option<&Choice>,
    fonts: &mut Fonts,
    (x, baseline, size, hscale): (f32, f32, f32, f32),
) -> Result<Vec<u8>> {
    let name = pdf::add_resource(doc, page, b"Font", "PVO", info.font_objs[font].clone())?;
    let f = &info.fonts[font];
    let mut out = vec![];
    for (i, line) in lines.iter().enumerate() {
        let y = baseline - size * TEXT_LINE * i as f32;
        let mut pen = x;
        for piece in line {
            match piece {
                Piece::Codes(codes) => {
                    let bytes: Vec<u8> = codes
                        .iter()
                        .flat_map(|&(c, len)| f.code_bytes(c, len))
                        .collect();
                    out.extend(b"BT /");
                    out.extend(&name);
                    out.extend(format!(" {} Tf ", n(size)).into_bytes());
                    if (hscale - 100.0).abs() > 0.01 {
                        out.extend(format!("{} Tz ", n(hscale)).into_bytes());
                    }
                    out.extend(format!("{} {} Td ", n(pen), n(y)).into_bytes());
                    crate::content::write_obj(&mut out, &Object::string_literal(bytes));
                    out.extend(b" Tj ET\n");
                    let w: f32 = codes.iter().map(|&(c, len)| f.width(c, len)).sum();
                    pen += w * size * hscale / 100.0;
                }
                Piece::Text(text) => {
                    let choice = fallback
                        .ok_or_else(|| Error::Unsupported("no font for these characters".into()))?;
                    let (ops, width) = fonts.piece(doc, page, choice, text, pen, y, size)?;
                    out.extend(ops);
                    pen += width;
                }
            }
        }
    }
    Ok(out)
}

/// Where a rewritten paragraph goes.
struct ParagraphLayout<'a> {
    start: f32,
    left: f32,
    right: f32,
    baseline: f32,
    leading: f32,
    align: &'a str,
}

/// One fragment drawn (or only measured, without `doc`) from (`x`, `y`).
#[allow(clippy::too_many_arguments)]
fn draw_frag(
    doc: Option<(&mut Document, ObjectId)>,
    info: &Page,
    frag: &Frag,
    x: f32,
    y: f32,
    fonts: &mut Fonts,
) -> Result<(Vec<u8>, f32)> {
    let st = &frag.style;
    match &frag.choice {
        Choice::Original {
            font,
            lines,
            fallback,
        } => {
            let f = &info.fonts[*font];
            let k = st.size * st.hscale / 100.0;
            match doc {
                None => {
                    let mut w = 0.0;
                    for piece in lines.iter().flatten() {
                        w += match piece {
                            Piece::Codes(codes) => {
                                codes.iter().map(|&(c, len)| f.width(c, len)).sum::<f32>() * k
                            }
                            Piece::Text(t) => {
                                let choice = fallback.as_deref().ok_or_else(|| {
                                    Error::Unsupported("no font for these characters".into())
                                })?;
                                fonts.measure(choice, t, st.size)?
                            }
                        };
                    }
                    Ok((vec![], w))
                }
                Some((doc, page)) => {
                    let ops = original_lines(
                        doc,
                        page,
                        info,
                        *font,
                        lines,
                        fallback.as_deref(),
                        fonts,
                        (x, y, st.size, st.hscale),
                    )?;
                    let (_, w) = draw_frag(None, info, frag, x, y, fonts)?;
                    Ok((ops, w))
                }
            }
        }
        choice => match doc {
            None => Ok((vec![], fonts.measure(choice, &frag.text, st.size)?)),
            Some((doc, page)) => fonts.piece(doc, page, choice, &frag.text, x, y, st.size),
        },
    }
}

/// A paragraph reflowed into lines and drawn, each fragment in its colour;
/// returns the operators and how many lines it took.
fn draw_paragraph(
    doc: &mut Document,
    page: ObjectId,
    info: &Page,
    frags: &[Frag],
    l: &ParagraphLayout,
    fonts: &mut Fonts,
) -> Result<(Vec<u8>, usize)> {
    let mut widths = Vec::with_capacity(frags.len());
    for f in frags {
        widths.push(match f.kind {
            FragKind::Break => 0.0,
            _ => draw_frag(None, info, f, 0.0, 0.0, fonts)?.1,
        });
    }
    // Lines of word groups: a word is a run of fragments without a space.
    struct LineOut {
        items: Vec<(Vec<usize>, f32)>,
        forced: bool,
    }
    let mut lines: Vec<LineOut> = vec![];
    let mut cur: Vec<(Vec<usize>, f32)> = vec![];
    let mut word: Vec<usize> = vec![];
    let mut space = 0.0f32;
    let mut used = 0.0f32;
    let limit = |line: usize| l.right - if line == 0 { l.start } else { l.left };
    let flush_word = |cur: &mut Vec<(Vec<usize>, f32)>,
                      lines: &mut Vec<LineOut>,
                      word: &mut Vec<usize>,
                      space: &mut f32,
                      used: &mut f32| {
        if word.is_empty() {
            return;
        }
        let w: f32 = word.iter().map(|&i| widths[i]).sum();
        let gap = if cur.is_empty() { 0.0 } else { *space };
        if !cur.is_empty() && *used + gap + w > limit(lines.len()) + 0.01 {
            lines.push(LineOut {
                items: std::mem::take(cur),
                forced: false,
            });
            *used = 0.0;
            cur.push((std::mem::take(word), w));
            *used = w;
        } else {
            *used += gap + w;
            cur.push((std::mem::take(word), w));
        }
        *space = 0.0;
    };
    for (i, f) in frags.iter().enumerate() {
        match f.kind {
            FragKind::Word => word.push(i),
            FragKind::Space => {
                flush_word(&mut cur, &mut lines, &mut word, &mut space, &mut used);
                space += widths[i];
            }
            FragKind::Break => {
                flush_word(&mut cur, &mut lines, &mut word, &mut space, &mut used);
                lines.push(LineOut {
                    items: std::mem::take(&mut cur),
                    forced: true,
                });
                used = 0.0;
                space = 0.0;
            }
        }
    }
    flush_word(&mut cur, &mut lines, &mut word, &mut space, &mut used);
    lines.push(LineOut {
        items: cur,
        forced: true,
    });

    // The space between words, in the style of the paragraph's first space.
    let gap = frags
        .iter()
        .zip(&widths)
        .find(|(f, _)| f.kind == FragKind::Space)
        .map(|(f, w)| *w / f.text.chars().count().max(1) as f32)
        .unwrap_or(0.0);
    let mut out = vec![];
    let count = lines.len();
    for (n, line) in lines.iter().enumerate() {
        let x0 = if n == 0 { l.start } else { l.left };
        let content: f32 = line.items.iter().map(|(_, w)| w).sum::<f32>()
            + gap * line.items.len().saturating_sub(1) as f32;
        let free = (limit(n) - content).max(0.0);
        let free = if free.is_finite() { free } else { 0.0 };
        let gaps = line.items.len().saturating_sub(1);
        let (offset, extra) = match l.align {
            "right" => (free, 0.0),
            "center" => (free / 2.0, 0.0),
            "justify" if !line.forced && n + 1 < count && gaps > 0 => (0.0, free / gaps as f32),
            _ => (0.0, 0.0),
        };
        let y = l.baseline - l.leading * n as f32;
        let mut x = x0 + offset;
        for (k, (word, w)) in line.items.iter().enumerate() {
            if k > 0 {
                x += gap + extra;
            }
            let mut pen = x;
            for &i in word {
                let f = &frags[i];
                out.extend(format!("q {} rg\n", rgb(f.style.color)).into_bytes());
                let (ops, fw) = draw_frag(Some((doc, page)), info, f, pen, y, fonts)?;
                out.extend(ops);
                out.extend(b"Q\n");
                pen += fw;
            }
            x += w;
        }
    }
    Ok((out, count))
}

/// Apply `o` to the PDF in `inputs[0]`; `inputs[1..]` are the attachments
/// items refer to (images, font files, the bundled fonts).
pub fn edit(inputs: &[Vec<u8>], o: &EditOptions) -> Result<(Vec<u8>, EditReport)> {
    let (pdf_bytes, attachments) = inputs
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

    // 1. What leaves the content, decided on the pages as they were.
    let mut pages: BTreeMap<usize, Page> = BTreeMap::new();
    let mut boxes: BTreeMap<usize, Vec<[f32; 4]>> = BTreeMap::new();
    let mut replaces: Vec<(usize, Vec<usize>, Rewrite, ReplaceItem)> = vec![];
    let mut objects: BTreeMap<usize, Vec<&ObjectItem>> = BTreeMap::new();
    for (i, item) in o.items.iter().enumerate() {
        match item {
            Item::Replace(r) => {
                finite(&[r.x, r.y, r.w, r.h])?;
                let id = page_of(&ids, r.page)?;
                let page = pages
                    .entry(r.page)
                    .or_insert_with(|| interp::interpret(&doc, id));
                let (placed, glyphs, written) = rewrite(page, r.page, r, &o.substitutes)?;
                replaces.push((i, glyphs, written, placed));
            }
            Item::Object(ob) => {
                finite(&[ob.dx, ob.dy, ob.sx, ob.sy])?;
                if ob.sx <= 0.0 || ob.sy <= 0.0 {
                    return Err(Error::option("an object's scale must be positive"));
                }
                let id = page_of(&ids, ob.page)?;
                pages
                    .entry(ob.page)
                    .or_insert_with(|| interp::interpret(&doc, id));
                objects.entry(ob.page).or_default().push(ob);
            }
            Item::Redact(b) => {
                finite(&[b.x, b.y, b.w, b.h])?;
                let id = page_of(&ids, b.page)?;
                pages
                    .entry(b.page)
                    .or_insert_with(|| interp::interpret(&doc, id));
                boxes.entry(b.page).or_default().push(b.bounds());
            }
            _ => {}
        }
    }
    // Replace items whose old glyphs could only be covered, not removed.
    let mut covered: BTreeSet<usize> = BTreeSet::new();
    for (&p, page) in &pages {
        let id = ids[p];
        let mut text_glyphs: Removal = Removal::default();
        for (i, glyphs, _, _) in replaces.iter().filter(|r| o.items[r.0].page() == p) {
            let removable = glyphs.iter().all(|&g| {
                let gl = &page.glyphs[g];
                !page.fonts[gl.font].uncertain && !page.streams[gl.stream].broken
            });
            if removable {
                text_glyphs.glyphs.extend(glyphs);
            } else {
                covered.insert(*i);
            }
        }
        let redaction = match boxes.get(&p) {
            Some(b) if o.redaction == Redaction::Remove => match redact::removal(page, b) {
                Some(r) => Some(r),
                None => {
                    report.flatten.push(p);
                    None
                }
            },
            Some(_) => {
                report.flatten.push(p);
                None
            }
            None => None,
        };
        let redacting = redaction.is_some();
        let mut all = redaction.unwrap_or_default();
        all.glyphs.extend(text_glyphs.glyphs.iter().copied());
        for ob in objects.get(&p).into_iter().flatten() {
            object_edit(page, ob, &mut all)?;
            report.objects += 1;
        }
        match scrub::apply(&mut doc, id, page, &all) {
            Ok(()) => {
                if redacting {
                    redact::remove_annotations(&mut doc, id, &page.to_visual, &boxes[&p])?;
                    report.redacted_pages.push(p + 1);
                }
            }
            Err(_) => {
                // Leave the redaction to the renderer and try the text alone.
                if redacting {
                    report.flatten.push(p);
                }
                if !redacting || scrub::apply(&mut doc, id, page, &text_glyphs).is_err() {
                    for (i, _, _, _) in replaces.iter().filter(|r| o.items[r.0].page() == p) {
                        covered.insert(*i);
                    }
                }
            }
        }
    }
    report.flatten.sort_unstable();
    report.flatten.dedup();

    // 2. Everything drawn, page by page in item order; redaction boxes last.
    let mut fonts = Fonts::new(attachments, o)?;
    let mut ops: BTreeMap<usize, Vec<u8>> = BTreeMap::new();
    let mut black: BTreeMap<usize, Vec<u8>> = BTreeMap::new();
    let mut embedded: HashMap<usize, (ObjectId, u32, u32)> = HashMap::new();
    let mut replace_at = replaces.iter();
    for (index, item) in o.items.iter().enumerate() {
        let p = item.page();
        let page = page_of(&ids, p)?;
        let mut out = Vec::new();
        let alpha = |doc: &mut Document, opacity: f32| -> Result<String> {
            Ok(stamp::add_alpha(doc, page, opacity.clamp(0.0, 1.0))?
                .map(|gs| format!("/{gs} gs\n"))
                .unwrap_or_default())
        };
        match item {
            Item::Text(t) => {
                finite(&[t.x, t.y])?;
                check_size(t.size)?;
                if t.text.trim().is_empty() {
                    continue;
                }
                let choice = text_choice(t)?;
                let color = rgb(parse_color(&t.color)?);
                let body = fonts.lines(
                    &mut doc,
                    page,
                    &choice,
                    &t.text,
                    t.x,
                    t.y - t.size * TEXT_ASCENT,
                    t.size,
                )?;
                out.extend(format!("q {color} rg\n").into_bytes());
                out.extend(body);
                out.extend(b"Q\n");
            }
            Item::Replace(_) => {
                let (_, glyphs, written, r) =
                    replace_at.next().expect("one entry per replace item");
                let info = &pages[&p];
                if let Rewrite::Paragraph(frags) = written {
                    out.extend(b"q\n");
                    if covered.contains(&index) {
                        report.covered += 1;
                        let b = r.bounds();
                        out.extend(
                            format!(
                                "1 1 1 rg {} {} {} {} re f\n",
                                n(b[0] - 1.0),
                                n(b[1] - 1.0),
                                n(b[2] - b[0] + 2.0),
                                n(b[3] - b[1] + 2.0)
                            )
                            .into_bytes(),
                        );
                    }
                    let first = glyphs.first().map(|&g| &info.glyphs[g]);
                    let start = r.start.or(first.map(|g| g.origin[0])).unwrap_or(r.x);
                    let baseline = r.baseline.or(first.map(|g| g.origin[1])).unwrap_or(r.y);
                    let size = frags.first().map(|f| f.style.size).unwrap_or(12.0);
                    let layout = ParagraphLayout {
                        start,
                        left: r.left.unwrap_or(start),
                        right: r.right.unwrap_or(f32::INFINITY),
                        baseline,
                        leading: r.leading.unwrap_or(size * TEXT_LINE),
                        align: r.align.as_deref().unwrap_or("left"),
                    };
                    finite(&[layout.start, layout.left, layout.baseline, layout.leading])?;
                    let (body, lines) =
                        draw_paragraph(&mut doc, page, info, frags, &layout, &mut fonts)?;
                    out.extend(body);
                    out.extend(b"Q\n");
                    let old_last = glyphs
                        .iter()
                        .map(|&g| info.glyphs[g].origin[1])
                        .fold(f32::MAX, f32::min);
                    let new_last =
                        layout.baseline - layout.leading * lines.saturating_sub(1) as f32;
                    if old_last.is_finite() && new_last < old_last - layout.leading * 0.5 {
                        report.overflow += 1;
                    }
                    let choices: Vec<&Choice> = frags
                        .iter()
                        .filter(|f| f.kind == FragKind::Word)
                        .map(|f| &f.choice)
                        .collect();
                    if choices.iter().any(|c| matches!(c, Choice::Substitute(..))) {
                        report.substituted += 1;
                    } else if choices
                        .iter()
                        .all(|c| matches!(c, Choice::Original { fallback: None, .. }))
                    {
                        report.same_font += 1;
                    } else {
                        report.mixed += 1;
                    }
                    report.replaced += 1;
                    ops.entry(p).or_default().extend(out);
                    continue;
                }
                let Rewrite::Line(choice) = written else {
                    unreachable!()
                };
                let first = glyphs.first().map(|&g| &info.glyphs[g]);
                let b = r.bounds();
                let size = r.size.or(first.map(|g| g.size)).unwrap_or(b[3] - b[1]);
                check_size(size)?;
                let color = match &r.color {
                    Some(c) => parse_color(c)?,
                    None => first.map(|g| g.color).unwrap_or([0.0; 3]),
                };
                let x = r.start.or(first.map(|g| g.origin[0])).unwrap_or(b[0]);
                let baseline = r
                    .baseline
                    .or(first.map(|g| g.origin[1]))
                    .unwrap_or(b[1] + (b[3] - b[1]) * 0.25);
                let hscale = r.hscale.or(first.map(|g| g.hscale)).unwrap_or(100.0);
                finite(&[size, x, baseline, hscale])?;
                out.extend(b"q\n");
                if covered.contains(&index) {
                    report.covered += 1;
                    out.extend(
                        format!(
                            "1 1 1 rg {} {} {} {} re f\n",
                            n(b[0] - 1.0),
                            n(b[1] - 1.0),
                            n(b[2] - b[0] + 2.0),
                            n(b[3] - b[1] + 2.0)
                        )
                        .into_bytes(),
                    );
                }
                if !r.text.trim().is_empty() {
                    out.extend(format!("{} rg\n", rgb(color)).into_bytes());
                    match choice {
                        Choice::Original {
                            font,
                            lines,
                            fallback,
                        } => {
                            if fallback.is_some() {
                                report.mixed += 1;
                            } else {
                                report.same_font += 1;
                            }
                            out.extend(original_lines(
                                &mut doc,
                                page,
                                info,
                                *font,
                                lines,
                                fallback.as_deref(),
                                &mut fonts,
                                (x, baseline, size, hscale),
                            )?);
                        }
                        other => {
                            if matches!(other, Choice::Substitute(..)) {
                                report.substituted += 1;
                            }
                            out.extend(
                                fonts.lines(&mut doc, page, other, &r.text, x, baseline, size)?,
                            )
                        }
                    }
                }
                out.extend(b"Q\n");
                report.replaced += 1;
                ops.entry(p).or_default().extend(out);
                continue;
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
                let bytes = attachments.get(im.image).ok_or_else(|| {
                    Error::option(format!(
                        "image {} was not given ({} attachments)",
                        im.image + 1,
                        attachments.len()
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
            Item::Object(_) => continue,
            Item::Redact(b) => {
                let r = b.bounds();
                black.entry(p).or_default().extend(
                    format!(
                        "0 0 0 rg {} {} {} {} re f\n",
                        n(r[0]),
                        n(r[1]),
                        n(r[2] - r[0]),
                        n(r[3] - r[1])
                    )
                    .into_bytes(),
                );
                continue;
            }
        }
        report.drawn += 1;
        ops.entry(p).or_default().extend(out);
    }
    for (p, body) in black {
        ops.entry(p).or_default().extend(body);
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
    fonts.finish(&mut doc)?;
    let out = pdf::save(&mut doc)?;
    if o.flatten_fields {
        return Ok((forms::flatten(&out, None)?.0, report));
    }
    Ok((out, report))
}

#[cfg(all(test, feature = "typeset"))]
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

    /// Text as the content interpreter reads it (embedded fonts included).
    fn read(bytes: &[u8], page: usize) -> String {
        let doc = pdf::load(bytes, None).unwrap();
        textedit::text_runs(&doc, &[page])
            .into_iter()
            .map(|r| r.text)
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// The PDF followed by the bundled fonts `plan` asks for.
    fn with_fonts(pdf: Vec<u8>, extra: Vec<Vec<u8>>, o: &mut EditOptions) -> Vec<Vec<u8>> {
        let p = plan(&pdf, o).unwrap();
        let mut inputs = vec![pdf];
        inputs.extend(extra);
        for key in p.fonts {
            let (fam, style) = key.split_once('-').unwrap();
            let file = format!(
                "Liberation{}-{}.ttf",
                match fam {
                    "sans" => "Sans",
                    "serif" => "Serif",
                    _ => "Mono",
                },
                match style {
                    "regular" => "Regular",
                    "bold" => "Bold",
                    "italic" => "Italic",
                    _ => "BoldItalic",
                }
            );
            o.bundled.insert(key, inputs.len() - 1);
            inputs.push(fixtures::bundled_font(&file));
        }
        inputs
    }

    #[test]
    fn draws_every_kind_of_item() {
        let sig = fixtures::png(&image::DynamicImage::ImageRgba8(fixtures::with_alpha(
            &fixtures::line_art(200, 80),
        )));
        let mut o = parse(
            r##"{ "items": [
                { "type": "text", "page": 0, "x": 72, "y": 700, "text": "Hello\nsecond line", "size": 18, "color": "#1a73e8", "bold": true },
                { "type": "rect", "page": 0, "x": 60, "y": 60, "w": 200, "h": 40, "fill": "#ffffff" },
                { "type": "rect", "page": 1, "x": 60, "y": 60, "w": -50, "h": 40, "stroke": "#ff0000", "lineWidth": 2, "opacity": 0.5 },
                { "type": "ink", "page": 1, "paths": [[10, 10, 50, 60, 90, 20], [200, 200]], "color": "#000000" },
                { "type": "image", "page": 1, "x": 300, "y": 100, "w": 150, "h": 60, "image": 0 },
                { "type": "image", "page": 0, "x": 300, "y": 100, "w": 150, "h": 60, "image": 0, "opacity": 0.8 },
                { "type": "note", "page": 0, "x": 500, "y": 750, "text": "Vérifier ce point" },
                { "type": "highlight", "page": 1, "x": 72, "y": 690, "w": 200, "h": 16 },
                { "type": "text", "page": 1, "x": 72, "y": 300, "text": "Ωμέγα – Привет – ça", "font": "serif", "italic": true }
            ] }"##,
        );
        let inputs = with_fonts(fixtures::text_pdf(2), vec![sig], &mut o);
        let (out, rep) = edit(&inputs, &o).unwrap();
        assert_eq!((rep.drawn, rep.annotations), (7, 2));
        qpdf_check(&out);
        let t = read(&out, 0);
        assert!(t.contains("Hello") && t.contains("second line"), "{t}");
        assert!(read(&out, 1).contains("Ωμέγα – Привет – ça"));
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
        let mut o = parse(
            r#"{ "blanks": [{ "size": "a4" }, {}],
                 "items": [{ "type": "text", "page": 3, "x": 50, "y": 800, "text": "On a blank page" }] }"#,
        );
        let inputs = with_fonts(fixtures::text_pdf(2), vec![], &mut o);
        let (out, rep) = edit(&inputs, &o).unwrap();
        assert_eq!((rep.pages, rep.blanks), (4, 2));
        qpdf_check(&out);
        assert!(read(&out, 3).contains("On a blank page"));
        let doc = pdf::load(&out, None).unwrap();
        let a4 = pdf::page_box(&doc, pdf::page_ids(&doc)[2]);
        assert!((a4[2] - 595.28).abs() < 0.01);

        let bad = parse(r#"{ "items": [{ "type": "text", "page": 5, "text": "x" }] }"#);
        assert!(edit(&[fixtures::text_pdf(2)], &bad).is_err());
        let no_img = parse(r#"{ "items": [{ "type": "image", "page": 0, "w": 10, "h": 10 }] }"#);
        assert!(edit(&[fixtures::text_pdf(1)], &no_img).is_err());
        let no_font = parse(r#"{ "items": [{ "type": "text", "page": 0, "text": "x" }] }"#);
        assert!(
            edit(&[fixtures::text_pdf(1)], &no_font).is_err(),
            "bundled font missing"
        );
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
        let mut o = parse(
            r#"{ "items": [
                { "type": "text", "page": 0, "x": 40, "y": 580, "text": "Upright" },
                { "type": "highlight", "page": 0, "x": 40, "y": 40, "w": 100, "h": 20 }
            ] }"#,
        );
        let inputs = with_fonts(rotated, vec![], &mut o);
        let (out, _) = edit(&inputs, &o).unwrap();
        qpdf_check(&out);
        let doc = pdf::load(&out, None).unwrap();
        let page = pdf::page_ids(&doc)[0];
        let upright = interp::interpret(&doc, page)
            .glyphs
            .into_iter()
            .find(|g| g.text.as_deref() == Some("U"))
            .unwrap();
        assert!(
            upright.angle.abs() < 0.01,
            "added text reads upright on the turned page"
        );
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

    fn run_box(bytes: &[u8], text: &str) -> textedit::TextRun {
        let doc = pdf::load(bytes, None).unwrap();
        textedit::text_runs(&doc, &[0])
            .into_iter()
            .find(|r| r.text == text)
            .unwrap()
    }

    #[test]
    fn replaces_text_in_its_own_font() {
        let pdf = fixtures::text_pdf(1);
        let r = run_box(&pdf, "Second line of body text.");
        let json = format!(
            r#"{{ "items": [{{ "type": "replace", "page": 0, "x": {}, "y": {}, "w": {}, "h": {}, "text": "Ligne modifiée, été 2026" }}] }}"#,
            r.x, r.y, r.w, r.h
        );
        let mut o = parse(&json);
        assert!(
            plan(&pdf, &o).unwrap().fonts.is_empty(),
            "Helvetica has every letter"
        );
        let inputs = with_fonts(pdf, vec![], &mut o);
        let (out, rep) = edit(&inputs, &o).unwrap();
        assert_eq!((rep.replaced, rep.same_font, rep.covered), (1, 1, 0));
        qpdf_check(&out);
        let t = text_of(&out, 1);
        assert!(
            t.contains("Ligne modifiée, été 2026") && !t.contains("Second line"),
            "{t}"
        );
        let after = run_box(&out, "Ligne modifiée, été 2026");
        assert!((after.start - r.start).abs() < 0.01 && (after.baseline - r.baseline).abs() < 0.01);
        assert!((after.size - r.size).abs() < 0.01);
    }

    #[test]
    fn replaces_text_beyond_the_font_with_a_bundled_one() {
        let pdf = fixtures::text_pdf(1);
        let r = run_box(&pdf, "Page 1");
        let json = format!(
            r#"{{ "items": [{{ "type": "replace", "page": 0, "x": {}, "y": {}, "w": {}, "h": {}, "text": "Страница 1", "bold": true }}] }}"#,
            r.x, r.y, r.w, r.h
        );
        let mut o = parse(&json);
        assert_eq!(plan(&pdf, &o).unwrap().fonts, vec!["sans-bold".to_string()]);
        let inputs = with_fonts(pdf, vec![], &mut o);
        let (out, rep) = edit(&inputs, &o).unwrap();
        assert_eq!((rep.replaced, rep.same_font), (1, 0));
        qpdf_check(&out);
        let after = run_box(&out, "Страница 1");
        assert!(after.bold && (after.size - 24.0).abs() < 0.01);
        assert!(!read(&out, 0).contains("Page 1"));
    }

    /// A page whose only text is "Rapport 2025" in an embedded subset of
    /// Liberation Sans, as PDF producers write fonts.
    fn subset_pdf() -> Vec<u8> {
        let mut o = parse(
            r#"{ "items": [{ "type": "text", "page": 0, "x": 72, "y": 700, "text": "Rapport 2025", "size": 20 }] }"#,
        );
        let inputs = with_fonts(fixtures::text_pdf(1), vec![], &mut o);
        let (out, _) = edit(&inputs, &o).unwrap();
        let mut doc = pdf::load(&out, None).unwrap();
        let page = pdf::page_ids(&doc)[0];
        // Keep only the added line: drop the fixture's Helvetica text.
        let p = interp::interpret(&doc, page);
        let mut r = crate::scrub::Removal::default();
        r.glyphs.extend(
            p.glyphs
                .iter()
                .enumerate()
                .filter(|(_, g)| !p.fonts[g.font].base.contains("Liberation"))
                .map(|(i, _)| i),
        );
        crate::scrub::apply(&mut doc, page, &p, &r).unwrap();
        pdf::save(&mut doc).unwrap()
    }

    #[test]
    fn only_the_characters_a_subset_lacks_change_font() {
        let pdf = subset_pdf();
        let mut o = parse(
            r#"{ "items": [{ "type": "replace", "page": 0, "find": "2025", "text": "2052" }] }"#,
        );
        assert!(
            plan(&pdf, &o).unwrap().wanted.is_empty(),
            "every digit is there"
        );
        let inputs = with_fonts(pdf.clone(), vec![], &mut o);
        let (_, rep) = edit(&inputs, &o).unwrap();
        assert_eq!((rep.same_font, rep.mixed), (1, 0));

        let mut o = parse(
            r#"{ "items": [{ "type": "replace", "page": 0, "find": "2025", "text": "2026" }] }"#,
        );
        let p = plan(&pdf, &o).unwrap();
        assert_eq!(p.wanted.len(), 1);
        assert_eq!(p.wanted[0].name, "LiberationSans");
        assert_eq!(p.wanted[0].family, "Liberation Sans");
        let inputs = with_fonts(pdf, vec![], &mut o);
        let (out, rep) = edit(&inputs, &o).unwrap();
        assert_eq!((rep.same_font, rep.mixed, rep.substituted), (0, 1, 0));
        qpdf_check(&out);
        assert_eq!(read(&out, 0).replace(' ', ""), "Rapport2026");
        // "Rapport 202" is still drawn with the page's own subset.
        let doc = pdf::load(&out, None).unwrap();
        let q = interp::interpret(&doc, pdf::page_ids(&doc)[0]);
        let fonts: Vec<&str> = q
            .glyphs
            .iter()
            .map(|g| q.fonts[g.font].base.as_str())
            .collect();
        assert_eq!(fonts.len(), 12);
        assert!(fonts[..11].iter().all(|f| *f == fonts[0]));
        assert_eq!(q.glyphs[11].text.as_deref(), Some("6"));
        let ends: Vec<f32> = q.glyphs.iter().map(|g| g.origin[0]).collect();
        assert!(
            ends.windows(2).all(|w| w[1] > w[0]),
            "glyphs run left to right without overlap"
        );
    }

    #[test]
    fn a_complete_copy_of_the_font_redraws_the_line() {
        let pdf = subset_pdf();
        let mut o = parse(
            r#"{ "substitutes": [{ "name": "LiberationSans", "file": 0 }],
                 "items": [{ "type": "replace", "page": 0, "find": "2025", "text": "2026" }] }"#,
        );
        assert!(plan(&pdf, &o).unwrap().wanted.is_empty());
        let inputs = with_fonts(
            pdf,
            vec![fixtures::bundled_font("LiberationSans-Regular.ttf")],
            &mut o,
        );
        let (out, rep) = edit(&inputs, &o).unwrap();
        assert_eq!(rep.substituted, 1);
        qpdf_check(&out);
        let doc = pdf::load(&out, None).unwrap();
        let q = interp::interpret(&doc, pdf::page_ids(&doc)[0]);
        let text: String = q.glyphs.iter().filter_map(|g| g.text.clone()).collect();
        assert_eq!(text, "Rapport 2026");
        let first = q.glyphs[0].font;
        assert!(
            q.glyphs.iter().all(|g| g.font == first),
            "one font for the whole line"
        );
    }

    /// A page with a two-line paragraph whose first words are bold.
    fn styled_pdf() -> Vec<u8> {
        use lopdf::dictionary;
        let mut doc = lopdf::Document::with_version("1.7");
        let regular = doc.add_object(dictionary! {
            "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica", "Encoding" => "WinAnsiEncoding",
        });
        let bold = doc.add_object(dictionary! {
            "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica-Bold", "Encoding" => "WinAnsiEncoding",
        });
        let content = doc.add_object(Stream::new(
            Dictionary::new(),
            b"BT /B 12 Tf 72 700 Td (Montant total :) Tj /R 12 Tf ( 4 250 euros, regle le 3 mars) Tj ET\n\
              BT /R 12 Tf 72 685 Td (par virement bancaire.) Tj ET"
                .to_vec(),
        ));
        fixtures::single_page(
            &mut doc,
            content,
            dictionary! { "Font" => dictionary! { "R" => regular, "B" => bold } },
        );
        pdf::save(&mut doc).unwrap()
    }

    fn paragraph_of(bytes: &[u8], starts: &str) -> crate::paragraphs::Paragraph {
        let doc = pdf::load(bytes, None).unwrap();
        let page = interp::interpret(&doc, pdf::page_ids(&doc)[0]);
        crate::paragraphs::paragraphs_of(&page, 0)
            .into_iter()
            .find(|p| p.text.starts_with(starts))
            .unwrap()
    }

    /// The sources of `new` when `old` became it by changing its end: the
    /// common start keeps its glyphs, the rest takes the previous one's.
    fn sources_for(p: &crate::paragraphs::Paragraph, new: &str) -> Vec<i64> {
        let old: Vec<char> = p.text.chars().collect();
        let common = new.chars().zip(&old).take_while(|(a, b)| a == *b).count();
        new.chars()
            .enumerate()
            .map(|(i, _)| if i < common { p.sources[i] } else { -1 })
            .collect()
    }

    #[test]
    fn a_paragraph_reflows_and_keeps_its_bold_words() {
        let pdf = styled_pdf();
        let p = paragraph_of(&pdf, "Montant total");
        assert_eq!(
            p.text,
            "Montant total : 4 250 euros, regle le 3 mars par virement bancaire."
        );
        assert_eq!((p.lines, p.leading), (2, 15.0));
        let text = "Montant total : 4 250 euros, regle le 3 mars par virement bancaire, avec un supplement de livraison.";
        let item = serde_json::json!({
            "type": "replace", "page": 0, "x": p.x, "y": p.y, "w": p.w, "h": p.h,
            "glyphs": p.glyphs, "sources": sources_for(&p, text), "text": text,
            "start": p.start, "left": p.left, "right": p.left + 250.0,
            "baseline": p.baseline, "leading": p.leading, "align": "left",
        });
        let mut o: EditOptions =
            serde_json::from_value(serde_json::json!({ "items": [item] })).unwrap();
        let inputs = with_fonts(pdf, vec![], &mut o);
        let (out, rep) = edit(&inputs, &o).unwrap();
        assert_eq!(
            (rep.replaced, rep.same_font, rep.overflow),
            (1, 1, 1),
            "one more line than before"
        );
        qpdf_check(&out);
        let after = paragraph_of(&out, "Montant total");
        assert_eq!(after.text, text);
        assert!(after.lines >= 3, "{} lines", after.lines);
        assert!(after.right <= p.left + 250.5 && (after.left - p.left).abs() < 0.01);
        assert!((after.leading - 15.0).abs() < 0.01 && (after.baseline - p.baseline).abs() < 0.01);
        let doc = pdf::load(&out, None).unwrap();
        let page = interp::interpret(&doc, pdf::page_ids(&doc)[0]);
        let fonts: Vec<(String, &str)> = page
            .glyphs
            .iter()
            .map(|g| {
                (
                    g.text.clone().unwrap_or_default(),
                    page.fonts[g.font].base.as_str(),
                )
            })
            .collect();
        assert!(
            fonts.iter().take(13).all(|(_, f)| *f == "Helvetica-Bold"),
            "{fonts:?}"
        );
        assert!(
            fonts.iter().skip(14).all(|(_, f)| *f == "Helvetica"),
            "{fonts:?}"
        );
    }

    #[test]
    fn a_justified_paragraph_stays_justified() {
        let pdf = fixtures::text_pdf(1);
        let p = paragraph_of(&pdf, "The quick");
        let text = "The quick brown fox jumps over the lazy dog, then naps in the warm afternoon sun while the farmer reads.";
        let item = serde_json::json!({
            "type": "replace", "page": 0, "x": p.x, "y": p.y, "w": p.w, "h": p.h,
            "glyphs": p.glyphs, "sources": sources_for(&p, text), "text": text,
            "start": p.start, "left": p.left, "right": p.left + 200.0,
            "baseline": p.baseline, "leading": p.leading, "align": "justify",
        });
        let mut o: EditOptions =
            serde_json::from_value(serde_json::json!({ "items": [item] })).unwrap();
        let inputs = with_fonts(pdf, vec![], &mut o);
        let (out, _) = edit(&inputs, &o).unwrap();
        let doc = pdf::load(&out, None).unwrap();
        let page = interp::interpret(&doc, pdf::page_ids(&doc)[0]);
        let lines = crate::paragraphs::lines_of(&page);
        let body: Vec<_> = lines.iter().filter(|l| l.size < 13.0).collect();
        assert!(body.len() >= 3);
        for l in &body[..body.len() - 1] {
            assert!(
                (l.end - (p.left + 200.0)).abs() < 0.5,
                "line ends at {}",
                l.end
            );
        }
        assert!(
            body.last().unwrap().end < p.left + 199.0,
            "the last line is not stretched"
        );
    }

    #[test]
    fn the_page_own_images_and_shapes_move_and_go() {
        // photo.pdf: one image on page 1, at (36, 36), 360 × 240 points.
        let o = parse(
            r#"{ "items": [{ "type": "object", "page": 0, "kind": "image", "index": 0, "dx": 100, "dy": 50, "sx": 0.5, "sy": 0.5 }] }"#,
        );
        let (out, rep) = edit(&[fixtures::photo_pdf()], &o).unwrap();
        assert_eq!(rep.objects, 1);
        qpdf_check(&out);
        let doc = pdf::load(&out, None).unwrap();
        let page = interp::interpret(&doc, pdf::page_ids(&doc)[0]);
        let b = page.images[0].bbox;
        for (got, want) in b.iter().zip([136.0, 86.0, 316.0, 206.0]) {
            assert!((got - want).abs() < 0.01, "{b:?}");
        }

        let mut d = lopdf::Document::with_version("1.7");
        let content = d.add_object(Stream::new(
            Dictionary::new(),
            b"1 0 0 rg 100 100 50 50 re f 0 0 1 rg 300 300 40 40 re f".to_vec(),
        ));
        fixtures::single_page(&mut d, content, Dictionary::new());
        let shapes = pdf::save(&mut d).unwrap();
        let listed = textedit::objects_of(&interp::interpret(&d, pdf::page_ids(&d)[0]), 0);
        assert_eq!(listed.len(), 2);
        let o = parse(
            r#"{ "items": [{ "type": "object", "page": 0, "kind": "path", "index": 0, "delete": true }] }"#,
        );
        let (out, _) = edit(&[shapes], &o).unwrap();
        let doc = pdf::load(&out, None).unwrap();
        let page = interp::interpret(&doc, pdf::page_ids(&doc)[0]);
        assert_eq!(page.paths.len(), 1);
        assert_eq!(page.paths[0].bbox, [300.0, 300.0, 340.0, 340.0]);
        let bad = parse(
            r#"{ "items": [{ "type": "object", "page": 0, "kind": "image", "index": 7, "delete": true }] }"#,
        );
        assert!(edit(&[fixtures::photo_pdf()], &bad).is_err());
    }

    #[test]
    fn replaces_a_line_found_by_its_text() {
        let pdf = fixtures::text_pdf(1);
        let mut o = parse(
            r#"{ "items": [{ "type": "replace", "page": 0, "find": "quick brown", "text": "slow red" }] }"#,
        );
        let inputs = with_fonts(pdf.clone(), vec![], &mut o);
        let (out, rep) = edit(&inputs, &o).unwrap();
        assert_eq!(rep.same_font, 1);
        let t = text_of(&out, 1);
        assert!(
            t.contains("The slow red fox jumps over the lazy dog.") && !t.contains("quick"),
            "{t}"
        );
        let missing = parse(
            r#"{ "items": [{ "type": "replace", "page": 0, "find": "absent", "text": "x" }] }"#,
        );
        assert!(edit(&[pdf], &missing).is_err());
    }

    #[test]
    fn redaction_keeps_the_rest_of_the_page_selectable() {
        let pdf = fixtures::text_pdf(1);
        let r = run_box(&pdf, "The quick brown fox jumps over the lazy dog.");
        let json = format!(
            r#"{{ "items": [{{ "type": "redact", "page": 0, "x": {}, "y": {}, "w": {}, "h": {} }}] }}"#,
            r.x,
            r.y,
            r.w * 0.45,
            r.h
        );
        let o = parse(&json);
        let (out, rep) = edit(&[pdf], &o).unwrap();
        assert_eq!(rep.redacted_pages, vec![1]);
        assert!(rep.flatten.is_empty());
        qpdf_check(&out);
        let t = text_of(&out, 1);
        assert!(!t.contains("quick") && !t.contains("brown"), "{t}");
        assert!(
            t.contains("lazy dog") && t.contains("Page 1") && t.contains("Second line"),
            "{t}"
        );
        let flat: EditOptions = serde_json::from_str(
            &json.replace("\"items\"", "\"redaction\": \"flatten\", \"items\""),
        )
        .unwrap();
        let (_, rep) = edit(&[fixtures::text_pdf(1)], &flat).unwrap();
        assert_eq!(rep.flatten, vec![0]);
    }
}
