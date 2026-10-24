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
        }
    }
}

impl ReplaceItem {
    pub fn bounds(&self) -> [f32; 4] {
        norm(self.x, self.y, self.w, self.h)
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
    /// Lines of existing text rewritten, and how many kept their own font.
    pub replaced: usize,
    pub same_font: usize,
    /// Rewritten lines whose old glyphs could not be taken out, only covered.
    pub covered: usize,
    /// Pages redacted in their content (1-based).
    pub redacted_pages: Vec<usize>,
    /// Pages left for the renderer to turn into images (0-based).
    pub flatten: Vec<usize>,
}

/// Which bundled fonts an edit needs, so the caller can pass just those.
#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct EditPlan {
    pub fonts: Vec<String>,
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

/// How a piece of text gets its font.
#[derive(Debug, Clone)]
#[cfg_attr(not(feature = "typeset"), allow(dead_code))]
enum Choice {
    /// The page font that already draws the text being replaced, with the
    /// codes for each line.
    Original(usize, Vec<Vec<(u32, usize)>>),
    Bundled(String),
    File(FontRef),
}

fn text_choice(it: &TextItem) -> Result<Choice> {
    if let Some(f) = &it.font_file {
        return Ok(Choice::File(f.clone()));
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
) -> Result<(ReplaceItem, Vec<usize>, Choice)> {
    let it = &locate(page, index, it)?;
    let (glyphs, choice) = choose(page, it)?;
    Ok((it.clone(), glyphs, choice))
}

fn choose(page: &Page, it: &ReplaceItem) -> Result<(Vec<usize>, Choice)> {
    let glyphs = textedit::glyphs_in(page, it.bounds());
    if let Some(f) = &it.font_file {
        return Ok((glyphs, Choice::File(f.clone())));
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
    // The original font, unless the item asks for another weight or slant.
    if let Some(f) = original {
        let same_style = it.bold.is_none_or(|b| b == page.fonts[f].bold)
            && it.italic.is_none_or(|i| i == page.fonts[f].italic);
        let lines: Option<Vec<_>> = same_style
            .then(|| {
                it.text
                    .lines()
                    .map(|l| textedit::encode_with(page, f, l))
                    .collect()
            })
            .flatten();
        if let Some(lines) = lines {
            return Ok((glyphs, Choice::Original(f, lines)));
        }
    }
    Ok((
        glyphs,
        style(base.map(|b| b.family).unwrap_or(Family::Sans)),
    ))
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
    for item in &o.items {
        let choice = match item {
            Item::Text(t) if !t.text.trim().is_empty() => text_choice(t)?,
            Item::Replace(r) => {
                let id = page_of(&ids, r.page)?;
                let page = pages
                    .entry(r.page)
                    .or_insert_with(|| interp::interpret(&doc, id));
                replace_choice(page, r.page, r)?.2
            }
            _ => continue,
        };
        if let Choice::Bundled(k) = choice {
            fonts.insert(k);
        }
    }
    Ok(EditPlan {
        fonts: fonts.into_iter().collect(),
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
        let primary = match choice {
            Choice::File(r) => self.slot(r)?,
            Choice::Bundled(k) => {
                let file = *self
                    .bundled
                    .get(k)
                    .ok_or_else(|| Error::option(format!("the bundled font {k} was not given")))?;
                self.slot(&FontRef { file, name: None })?
            }
            Choice::Original(..) => unreachable!("original fonts are not typeset"),
        };
        let mut chain = vec![primary];
        chain.extend(self.fallbacks.iter().copied().filter(|s| *s != primary));
        Ok(chain)
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
            "writing text needs the render engine".into(),
        ))
    }

    fn finish(self, _: &mut Document) -> Result<()> {
        Ok(())
    }
}

/// Text in a font the page already has, from codes that font maps.
fn original_lines(
    doc: &mut Document,
    page: ObjectId,
    info: &Page,
    font: usize,
    lines: &[Vec<(u32, usize)>],
    (x, baseline, size, hscale): (f32, f32, f32, f32),
) -> Result<Vec<u8>> {
    let name = pdf::add_resource(doc, page, b"Font", "PVO", info.font_objs[font].clone())?;
    let f = &info.fonts[font];
    let mut out = vec![];
    for (i, codes) in lines.iter().enumerate() {
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
        out.extend(
            format!("{} {} Td ", n(x), n(baseline - size * TEXT_LINE * i as f32)).into_bytes(),
        );
        crate::content::write_obj(&mut out, &Object::string_literal(bytes));
        out.extend(b" Tj ET\n");
    }
    Ok(out)
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
    let mut replaces: Vec<(usize, Vec<usize>, Choice, ReplaceItem)> = vec![];
    for (i, item) in o.items.iter().enumerate() {
        match item {
            Item::Replace(r) => {
                finite(&[r.x, r.y, r.w, r.h])?;
                let id = page_of(&ids, r.page)?;
                let page = pages
                    .entry(r.page)
                    .or_insert_with(|| interp::interpret(&doc, id));
                let (placed, glyphs, choice) = replace_choice(page, r.page, r)?;
                replaces.push((i, glyphs, choice, placed));
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
                let (_, glyphs, choice, r) = replace_at.next().expect("one entry per replace item");
                let info = &pages[&p];
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
                        Choice::Original(f, lines) => {
                            report.same_font += 1;
                            out.extend(original_lines(
                                &mut doc,
                                page,
                                info,
                                *f,
                                lines,
                                (x, baseline, size, hscale),
                            )?);
                        }
                        other => out.extend(
                            fonts.lines(&mut doc, page, other, &r.text, x, baseline, size)?,
                        ),
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
