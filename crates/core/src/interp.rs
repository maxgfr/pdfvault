//! Where things land on a page: every glyph, image and painted path, in the
//! page's visual space (points, origin at the bottom-left of the page as
//! displayed), tied to the exact operation that drew it so the content can be
//! edited in place. Form XObjects are followed; each drawing of one is its
//! own stream, so a change can apply to one use without touching the others.

use crate::content::{self, Op};
use crate::pdf;
use crate::pdffont::Font;
use crate::stamp;
use lopdf::{Dictionary, Document, Object, ObjectId};
use std::collections::HashMap;

pub type Matrix = [f32; 6];

const IDENTITY: Matrix = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];
/// Form XObjects nested deeper than this are not followed.
const MAX_FORM_DEPTH: usize = 12;
/// Bounds on the work one page can ask for.
const MAX_OPS: usize = 4_000_000;
const MAX_GLYPHS: usize = 2_000_000;

/// `a` then `b` (PDF's row-vector convention: p' = p × a × b).
pub fn mul(a: &Matrix, b: &Matrix) -> Matrix {
    [
        a[0] * b[0] + a[1] * b[2],
        a[0] * b[1] + a[1] * b[3],
        a[2] * b[0] + a[3] * b[2],
        a[2] * b[1] + a[3] * b[3],
        a[4] * b[0] + a[5] * b[2] + b[4],
        a[4] * b[1] + a[5] * b[3] + b[5],
    ]
}

pub fn apply(m: &Matrix, x: f32, y: f32) -> [f32; 2] {
    [m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5]]
}

pub fn invert(m: &Matrix) -> Option<Matrix> {
    let det = m[0] * m[3] - m[1] * m[2];
    if det.abs() < 1e-12 || !det.is_finite() {
        return None;
    }
    Some([
        m[3] / det,
        -m[1] / det,
        -m[2] / det,
        m[0] / det,
        (m[2] * m[5] - m[3] * m[4]) / det,
        (m[1] * m[4] - m[0] * m[5]) / det,
    ])
}

/// Axis-aligned bounds of a box mapped through `m`.
pub fn map_box(m: &Matrix, b: [f32; 4]) -> [f32; 4] {
    let pts = [
        apply(m, b[0], b[1]),
        apply(m, b[2], b[1]),
        apply(m, b[0], b[3]),
        apply(m, b[2], b[3]),
    ];
    bounds(&pts)
}

fn bounds(pts: &[[f32; 2]]) -> [f32; 4] {
    let mut b = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
    for p in pts {
        b[0] = b[0].min(p[0]);
        b[1] = b[1].min(p[1]);
        b[2] = b[2].max(p[0]);
        b[3] = b[3].max(p[1]);
    }
    b
}

pub fn area(b: &[f32; 4]) -> f32 {
    (b[2] - b[0]).max(0.0) * (b[3] - b[1]).max(0.0)
}

pub fn intersect(a: &[f32; 4], b: &[f32; 4]) -> [f32; 4] {
    [
        a[0].max(b[0]),
        a[1].max(b[1]),
        a[2].min(b[2]),
        a[3].min(b[3]),
    ]
}

pub fn contains(outer: &[f32; 4], inner: &[f32; 4]) -> bool {
    inner[0] >= outer[0] && inner[1] >= outer[1] && inner[2] <= outer[2] && inner[3] <= outer[3]
}

/// One content stream as drawn on this page.
#[derive(Debug)]
pub struct Stream {
    pub bytes: Vec<u8>,
    pub ops: Vec<Op>,
    /// The form XObject this stream is, or `None` for the page's own content.
    pub xobject: Option<ObjectId>,
    /// The stream and op (`Do`) that drew this one.
    pub parent: Option<(usize, usize)>,
    pub resources: Dictionary,
    /// The stream could not be read; nothing in it is known.
    pub broken: bool,
}

#[derive(Debug, Clone)]
pub struct Glyph {
    pub stream: usize,
    pub op: usize,
    /// Index in the TJ array (0 for the other show operators).
    pub elem: usize,
    /// Byte range of the code within its string.
    pub start: usize,
    pub len: usize,
    pub code: u32,
    pub font: usize,
    pub text: Option<String>,
    pub bbox: [f32; 4],
    pub origin: [f32; 2],
    pub end: [f32; 2],
    /// Font size as it appears on the page, in points.
    pub size: f32,
    /// Direction of the baseline, in degrees counter-clockwise.
    pub angle: f32,
    pub color: [f32; 3],
    /// Text rendering mode (3 = invisible, as in OCR layers).
    pub mode: i64,
    /// The glyph's advance as a TJ adjustment, so removing it can leave its
    /// neighbours where they were. `None` when the font size is zero.
    pub kern: Option<f32>,
    /// Horizontal scaling (Tz), percent.
    pub hscale: f32,
    /// Innermost marked-content sequence carrying replacement text.
    pub marked: Option<usize>,
}

impl Glyph {
    pub fn center(&self) -> [f32; 2] {
        [
            (self.bbox[0] + self.bbox[2]) / 2.0,
            (self.bbox[1] + self.bbox[3]) / 2.0,
        ]
    }
}

#[derive(Debug, Clone)]
pub struct ImageUse {
    pub stream: usize,
    pub op: usize,
    /// The XObject, or `None` for an inline image.
    pub id: Option<ObjectId>,
    /// Unit square to page.
    pub matrix: Matrix,
    pub bbox: [f32; 4],
}

#[derive(Debug, Clone)]
pub struct PathUse {
    pub stream: usize,
    pub first: usize,
    pub paint: usize,
    pub bbox: [f32; 4],
    /// The path also sets the clip.
    pub clip: bool,
}

#[derive(Debug, Clone)]
pub struct FormUse {
    pub stream: usize,
    pub op: usize,
    pub id: ObjectId,
    /// The stream this drawing became (absent when not followed).
    pub child: Option<usize>,
    pub bbox: [f32; 4],
}

/// A marked-content sequence (BDC) whose properties hold ActualText, Alt or E.
#[derive(Debug, Clone)]
pub struct Marked {
    pub stream: usize,
    pub op: usize,
    /// Properties are written in the op itself rather than named in /Properties.
    pub inline: bool,
}

#[derive(Debug, Default)]
pub struct Page {
    pub streams: Vec<Stream>,
    pub glyphs: Vec<Glyph>,
    pub images: Vec<ImageUse>,
    pub paths: Vec<PathUse>,
    pub forms: Vec<FormUse>,
    pub marked: Vec<Marked>,
    pub fonts: Vec<Font>,
    /// Each font as the resources hold it (a reference, or a direct dictionary).
    pub font_objs: Vec<Object>,
    /// Size of the page as displayed.
    pub width: f32,
    pub height: f32,
    /// User space to visual space.
    pub to_visual: Matrix,
}

impl Page {
    /// Everything on the page is accounted for: no unreadable stream, no font
    /// whose codes or widths were guessed.
    pub fn reliable(&self) -> bool {
        !self.streams.iter().any(|s| s.broken) && !self.fonts.iter().any(|f| f.uncertain)
    }
}

#[derive(Clone)]
struct GState {
    ctm: Matrix,
    fill: [f32; 3],
    fill_comps: usize,
    line_width: f32,
    font: Option<usize>,
    size: f32,
    char_spacing: f32,
    word_spacing: f32,
    hscale: f32,
    leading: f32,
    rise: f32,
    mode: i64,
}

fn rgb_of(args: &[f32]) -> Option<[f32; 3]> {
    match args {
        [g] => Some([*g, *g, *g]),
        [r, g, b] => Some([*r, *g, *b]),
        [c, m, y, k] => Some([
            (1.0 - c) * (1.0 - k),
            (1.0 - m) * (1.0 - k),
            (1.0 - y) * (1.0 - k),
        ]),
        _ => None,
    }
}

fn sub_dict<'a>(doc: &'a Document, res: &'a Dictionary, cat: &[u8]) -> Option<&'a Dictionary> {
    res.get(cat)
        .ok()
        .and_then(|o| pdf::resolve(doc, o).as_dict().ok())
}

fn colorspace_comps(doc: &Document, res: &Dictionary, name: &[u8]) -> usize {
    match name {
        b"DeviceGray" | b"CalGray" | b"G" => 1,
        b"DeviceRGB" | b"CalRGB" | b"RGB" | b"Lab" => 3,
        b"DeviceCMYK" | b"CMYK" => 4,
        b"Pattern" => 0,
        other => {
            let cs = sub_dict(doc, res, b"ColorSpace")
                .and_then(|d| d.get(other).ok())
                .map(|o| pdf::resolve(doc, o));
            match cs {
                Some(Object::Array(a)) => match a.first().and_then(|o| o.as_name().ok()) {
                    Some(b"ICCBased") => a
                        .get(1)
                        .and_then(|o| pdf::resolve(doc, o).as_stream().ok())
                        .and_then(|s| s.dict.get(b"N").ok().and_then(pdf::num))
                        .unwrap_or(3.0) as usize,
                    Some(b"Indexed" | b"I" | b"Separation" | b"DeviceN") => 1,
                    Some(b"Pattern") => 0,
                    Some(n) => colorspace_comps(doc, res, n),
                    None => 3,
                },
                Some(Object::Name(n)) => colorspace_comps(doc, res, n),
                _ => 3,
            }
        }
    }
}

/// Bytes of a page's content: all its streams, joined.
pub fn page_content(doc: &Document, page: ObjectId) -> Option<Vec<u8>> {
    let dict = doc.get_dictionary(page).ok()?;
    let refs: Vec<Object> = match dict.get(b"Contents").ok()? {
        Object::Array(a) => a.clone(),
        Object::Reference(id) => match doc.get_object(*id).ok()? {
            Object::Array(a) => a.clone(),
            _ => vec![Object::Reference(*id)],
        },
        _ => return Some(vec![]),
    };
    let mut out = vec![];
    for r in refs {
        let s = pdf::resolve(doc, &r).as_stream().ok()?;
        let data = if s.dict.has(b"Filter") {
            s.decompressed_content().ok()?
        } else {
            s.content.clone()
        };
        out.extend(data);
        // Streams split at token boundaries; keep them apart.
        out.push(b'\n');
    }
    Some(out)
}

struct Walker<'a> {
    doc: &'a Document,
    page: Page,
    font_ids: HashMap<ObjectId, usize>,
    ops_seen: usize,
}

impl<'a> Walker<'a> {
    fn font(&mut self, stream: usize, name: &[u8]) -> Option<usize> {
        let doc = self.doc;
        let fonts = sub_dict(doc, &self.page.streams[stream].resources, b"Font")?;
        let obj = fonts.get(name).ok()?;
        if let Object::Reference(id) = obj {
            if let Some(&i) = self.font_ids.get(id) {
                return Some(i);
            }
            let dict = doc.get_dictionary(*id).ok()?;
            let f = Font::load(doc, dict);
            self.page.fonts.push(f);
            self.page.font_objs.push(Object::Reference(*id));
            self.font_ids.insert(*id, self.page.fonts.len() - 1);
            return Some(self.page.fonts.len() - 1);
        }
        let dict = obj.as_dict().ok()?;
        self.page.fonts.push(Font::load(doc, dict));
        self.page.font_objs.push(Object::Dictionary(dict.clone()));
        Some(self.page.fonts.len() - 1)
    }

    #[allow(clippy::too_many_arguments)]
    fn show(
        &mut self,
        stream: usize,
        op: usize,
        elem: usize,
        bytes: &[u8],
        gs: &GState,
        tm: &mut Matrix,
        marked: Option<usize>,
    ) {
        let Some(fi) = gs.font else { return };
        let th = gs.hscale / 100.0;
        let tfs = gs.size;
        let codes = self.page.fonts[fi].codes(bytes);
        for (code, start, len) in codes {
            if self.page.glyphs.len() >= MAX_GLYPHS {
                return;
            }
            let f = &self.page.fonts[fi];
            let vertical = f.vertical();
            let w = f.width(code, len);
            let adv = f.advance(code, len);
            let tw = if Font::is_word_space(code, len) {
                gs.word_spacing
            } else {
                0.0
            };
            let trm = mul(&mul(&[tfs * th, 0.0, 0.0, tfs, 0.0, gs.rise], tm), &gs.ctm);
            let gbox = if vertical {
                [-w / 2.0, adv.min(0.0), w / 2.0, 0.0]
            } else {
                [0.0, f.descent, w, f.ascent]
            };
            let bbox = map_box(&trm, gbox);
            let origin = apply(&trm, 0.0, 0.0);
            let (tx, ty) = if vertical {
                (0.0, adv * tfs + gs.char_spacing + tw)
            } else {
                ((w * tfs + gs.char_spacing + tw) * th, 0.0)
            };
            let kern = (tfs.abs() > 1e-6 && th.abs() > 1e-6).then(|| {
                if vertical {
                    ty / tfs * 1000.0
                } else {
                    tx / (tfs * th) * 1000.0
                }
            });
            *tm = mul(&[1.0, 0.0, 0.0, 1.0, tx, ty], tm);
            let end = apply(&mul(&[tfs * th, 0.0, 0.0, tfs, 0.0, gs.rise], tm), 0.0, 0.0);
            let end = apply(&gs.ctm, end[0], end[1]);
            let up = apply(&trm, 0.0, 1.0);
            let size = ((up[0] - origin[0]).powi(2) + (up[1] - origin[1]).powi(2)).sqrt();
            let angle = trm[1].atan2(trm[0]).to_degrees();
            let text = f.text(code, len);
            self.page.glyphs.push(Glyph {
                stream,
                op,
                elem,
                start,
                len,
                code,
                font: fi,
                text,
                bbox,
                origin,
                end,
                size,
                angle,
                color: gs.fill,
                mode: gs.mode,
                kern,
                hscale: gs.hscale,
                marked,
            });
        }
    }

    fn adjust(&self, n: f32, gs: &GState, tm: &mut Matrix) {
        let vertical = gs
            .font
            .map(|f| self.page.fonts[f].vertical())
            .unwrap_or(false);
        let d = -n / 1000.0 * gs.size;
        let (tx, ty) = if vertical {
            (0.0, d)
        } else {
            (d * gs.hscale / 100.0, 0.0)
        };
        *tm = mul(&[1.0, 0.0, 0.0, 1.0, tx, ty], tm);
    }

    fn has_replacement_text(&self, stream: usize, op: &Op) -> Option<bool> {
        const KEYS: [&[u8]; 3] = [b"ActualText", b"Alt", b"E"];
        match op.args.get(1)? {
            Object::Dictionary(d) => KEYS.iter().any(|k| d.has(k)).then_some(true),
            Object::Name(n) => {
                let props = sub_dict(
                    self.doc,
                    &self.page.streams[stream].resources,
                    b"Properties",
                )?;
                let d = pdf::resolve(self.doc, props.get(n).ok()?).as_dict().ok()?;
                KEYS.iter().any(|k| d.has(k)).then_some(false)
            }
            _ => None,
        }
    }

    fn run(&mut self, stream: usize, mut gs: GState, depth: usize, ancestors: &mut Vec<ObjectId>) {
        let ops = std::mem::take(&mut self.page.streams[stream].ops);
        let mut stack: Vec<GState> = vec![];
        let mut tm = IDENTITY;
        let mut tlm = IDENTITY;
        let mut path: Vec<[f32; 2]> = vec![];
        let mut path_start: Option<usize> = None;
        let mut clip = false;
        let mut marks: Vec<Option<usize>> = vec![];
        for (i, op) in ops.iter().enumerate() {
            self.ops_seen += 1;
            if self.ops_seen > MAX_OPS {
                self.page.streams[stream].broken = true;
                break;
            }
            let marked = marks.iter().rev().find_map(|m| *m);
            match op.name.as_slice() {
                b"q" => stack.push(gs.clone()),
                b"Q" => {
                    if let Some(s) = stack.pop() {
                        gs = s;
                    }
                }
                b"cm" => {
                    if let Some(m) = op.nums::<6>() {
                        gs.ctm = mul(&m, &gs.ctm);
                    }
                }
                b"w" => gs.line_width = op.num(0).unwrap_or(gs.line_width),
                b"g" | b"rg" | b"k" => {
                    let v: Vec<f32> = op.args.iter().filter_map(pdf::num).collect();
                    if let Some(c) = rgb_of(&v) {
                        gs.fill = c;
                        gs.fill_comps = v.len();
                    }
                }
                b"cs" => {
                    let res = &self.page.streams[stream].resources;
                    gs.fill_comps = op
                        .name_arg(0)
                        .map(|n| colorspace_comps(self.doc, res, n))
                        .unwrap_or(1);
                    gs.fill = [0.0; 3];
                }
                b"sc" | b"scn" => {
                    let v: Vec<f32> = op.args.iter().filter_map(pdf::num).collect();
                    if gs.fill_comps > 0 && v.len() == gs.fill_comps {
                        if let Some(c) = rgb_of(&v) {
                            gs.fill = c;
                        }
                    } else if v.len() == 1 {
                        // Separation or indexed: a tint, shown as grey.
                        gs.fill = [1.0 - v[0].clamp(0.0, 1.0); 3];
                    }
                }
                b"BT" => {
                    tm = IDENTITY;
                    tlm = IDENTITY;
                }
                b"Tf" => {
                    gs.font = op.name_arg(0).and_then(|n| self.font(stream, n));
                    gs.size = op.num(1).unwrap_or(0.0);
                }
                b"Tc" => gs.char_spacing = op.num(0).unwrap_or(0.0),
                b"Tw" => gs.word_spacing = op.num(0).unwrap_or(0.0),
                b"Tz" => gs.hscale = op.num(0).unwrap_or(100.0),
                b"TL" => gs.leading = op.num(0).unwrap_or(0.0),
                b"Ts" => gs.rise = op.num(0).unwrap_or(0.0),
                b"Tr" => gs.mode = op.num(0).unwrap_or(0.0) as i64,
                b"Td" | b"TD" => {
                    if let Some([x, y]) = op.nums::<2>() {
                        if op.is("TD") {
                            gs.leading = -y;
                        }
                        tlm = mul(&[1.0, 0.0, 0.0, 1.0, x, y], &tlm);
                        tm = tlm;
                    }
                }
                b"Tm" => {
                    if let Some(m) = op.nums::<6>() {
                        tlm = m;
                        tm = m;
                    }
                }
                b"T*" => {
                    tlm = mul(&[1.0, 0.0, 0.0, 1.0, 0.0, -gs.leading], &tlm);
                    tm = tlm;
                }
                b"Tj" | b"'" | b"\"" => {
                    if op.is("'") || op.is("\"") {
                        if op.is("\"") {
                            gs.word_spacing = op.num(0).unwrap_or(gs.word_spacing);
                            gs.char_spacing = op.num(1).unwrap_or(gs.char_spacing);
                        }
                        tlm = mul(&[1.0, 0.0, 0.0, 1.0, 0.0, -gs.leading], &tlm);
                        tm = tlm;
                    }
                    if let Some(Object::String(s, _)) = op.args.last() {
                        self.show(stream, i, 0, s, &gs, &mut tm, marked);
                    }
                }
                b"TJ" => {
                    if let Some(Object::Array(items)) = op.args.first() {
                        for (k, item) in items.iter().enumerate() {
                            match item {
                                Object::String(s, _) => {
                                    self.show(stream, i, k, s, &gs, &mut tm, marked)
                                }
                                other => {
                                    if let Some(n) = pdf::num(other) {
                                        self.adjust(n, &gs, &mut tm);
                                    }
                                }
                            }
                        }
                    }
                }
                b"BMC" => marks.push(None),
                b"BDC" => {
                    let m = self.has_replacement_text(stream, op).map(|inline| {
                        self.page.marked.push(Marked {
                            stream,
                            op: i,
                            inline,
                        });
                        self.page.marked.len() - 1
                    });
                    marks.push(m);
                }
                b"EMC" => {
                    marks.pop();
                }
                b"m" | b"l" | b"c" | b"v" | b"y" | b"re" | b"h" => {
                    path_start.get_or_insert(i);
                    let v: Vec<f32> = op.args.iter().filter_map(pdf::num).collect();
                    if op.is("re") && v.len() == 4 {
                        let (x, y, w, h) = (v[0], v[1], v[2], v[3]);
                        for (px, py) in [(x, y), (x + w, y), (x, y + h), (x + w, y + h)] {
                            path.push(apply(&gs.ctm, px, py));
                        }
                    } else {
                        for p in v.as_chunks::<2>().0 {
                            path.push(apply(&gs.ctm, p[0], p[1]));
                        }
                    }
                }
                b"W" | b"W*" => clip = true,
                b"S" | b"s" | b"f" | b"F" | b"f*" | b"B" | b"B*" | b"b" | b"b*" | b"n" => {
                    if let Some(first) = path_start.take()
                        && !op.is("n")
                        && !path.is_empty()
                    {
                        let mut bbox = bounds(&path);
                        let strokes = matches!(
                            op.name.as_slice(),
                            b"S" | b"s" | b"B" | b"B*" | b"b" | b"b*"
                        );
                        if strokes {
                            let k = (gs.ctm[0] * gs.ctm[3] - gs.ctm[1] * gs.ctm[2]).abs().sqrt();
                            let half = gs.line_width.max(1.0) * k / 2.0;
                            bbox = [
                                bbox[0] - half,
                                bbox[1] - half,
                                bbox[2] + half,
                                bbox[3] + half,
                            ];
                        }
                        self.page.paths.push(PathUse {
                            stream,
                            first,
                            paint: i,
                            bbox,
                            clip,
                        });
                    }
                    path.clear();
                    path_start = None;
                    clip = false;
                }
                b"BI" => {
                    let m = gs.ctm;
                    self.page.images.push(ImageUse {
                        stream,
                        op: i,
                        id: None,
                        matrix: m,
                        bbox: map_box(&m, [0.0, 0.0, 1.0, 1.0]),
                    });
                }
                b"Do" => {
                    let Some(name) = op.name_arg(0) else { continue };
                    let doc = self.doc;
                    let Some(xobjects) =
                        sub_dict(doc, &self.page.streams[stream].resources, b"XObject")
                    else {
                        continue;
                    };
                    let Ok(Object::Reference(id)) = xobjects.get(name) else {
                        continue;
                    };
                    let id = *id;
                    let Ok(x) = doc.get_object(id).and_then(Object::as_stream) else {
                        continue;
                    };
                    match x.dict.get(b"Subtype").and_then(Object::as_name) {
                        Ok(b"Image") => {
                            let m = gs.ctm;
                            self.page.images.push(ImageUse {
                                stream,
                                op: i,
                                id: Some(id),
                                matrix: m,
                                bbox: map_box(&m, [0.0, 0.0, 1.0, 1.0]),
                            });
                        }
                        Ok(b"Form") => {
                            let fm: Matrix = x
                                .dict
                                .get(b"Matrix")
                                .ok()
                                .and_then(|o| o.as_array().ok())
                                .map(|a| {
                                    a.iter()
                                        .filter_map(|o| pdf::num(pdf::resolve(doc, o)))
                                        .collect::<Vec<_>>()
                                })
                                .and_then(|v| <[f32; 6]>::try_from(v).ok())
                                .unwrap_or(IDENTITY);
                            let ctm = mul(&fm, &gs.ctm);
                            let bb: Vec<f32> = x
                                .dict
                                .get(b"BBox")
                                .ok()
                                .and_then(|o| pdf::resolve(doc, o).as_array().ok())
                                .map(|a| {
                                    a.iter()
                                        .filter_map(|o| pdf::num(pdf::resolve(doc, o)))
                                        .collect()
                                })
                                .unwrap_or_default();
                            let bbox = if bb.len() == 4 {
                                map_box(
                                    &ctm,
                                    [
                                        bb[0].min(bb[2]),
                                        bb[1].min(bb[3]),
                                        bb[0].max(bb[2]),
                                        bb[1].max(bb[3]),
                                    ],
                                )
                            } else {
                                [f32::MIN, f32::MIN, f32::MAX, f32::MAX]
                            };
                            let mut use_ = FormUse {
                                stream,
                                op: i,
                                id,
                                child: None,
                                bbox,
                            };
                            if depth < MAX_FORM_DEPTH && !ancestors.contains(&id) {
                                let data = x
                                    .decompressed_content()
                                    .unwrap_or_else(|_| x.content.clone());
                                let resources = x
                                    .dict
                                    .get(b"Resources")
                                    .ok()
                                    .and_then(|o| pdf::resolve(doc, o).as_dict().ok())
                                    .cloned()
                                    .unwrap_or_else(|| self.page.streams[stream].resources.clone());
                                let (ops, broken) = match content::parse(&data) {
                                    Ok(ops) => (ops, false),
                                    Err(_) => (vec![], true),
                                };
                                self.page.streams.push(Stream {
                                    bytes: data,
                                    ops,
                                    xobject: Some(id),
                                    parent: Some((stream, i)),
                                    resources,
                                    broken,
                                });
                                let child = self.page.streams.len() - 1;
                                use_.child = Some(child);
                                let mut inner = gs.clone();
                                inner.ctm = ctm;
                                ancestors.push(id);
                                self.run(child, inner, depth + 1, ancestors);
                                ancestors.pop();
                            }
                            self.page.forms.push(use_);
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }
        self.page.streams[stream].ops = ops;
    }
}

/// Interpret one page.
pub fn interpret(doc: &Document, page: ObjectId) -> Page {
    let (frame, width, height) = stamp::visual_frame(doc, page);
    let to_visual = invert(&frame).unwrap_or(IDENTITY);
    let resources = pdf::inherited(doc, page, b"Resources")
        .and_then(|o| pdf::resolve(doc, &o).as_dict().ok().cloned())
        .unwrap_or_default();
    let (bytes, ops, broken) = match page_content(doc, page) {
        Some(b) => match content::parse(&b) {
            Ok(ops) => (b, ops, false),
            Err(_) => (b, vec![], true),
        },
        None => (vec![], vec![], true),
    };
    let mut w = Walker {
        doc,
        page: Page {
            width,
            height,
            to_visual,
            ..Default::default()
        },
        font_ids: HashMap::new(),
        ops_seen: 0,
    };
    w.page.streams.push(Stream {
        bytes,
        ops,
        xobject: None,
        parent: None,
        resources,
        broken,
    });
    let gs = GState {
        ctm: to_visual,
        fill: [0.0; 3],
        fill_comps: 1,
        line_width: 1.0,
        font: None,
        size: 0.0,
        char_spacing: 0.0,
        word_spacing: 0.0,
        hscale: 100.0,
        leading: 0.0,
        rise: 0.0,
        mode: 0,
    };
    w.run(0, gs, 0, &mut vec![]);
    w.page
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures;
    use lopdf::dictionary;

    #[test]
    fn places_glyphs_like_the_text_layer() {
        let doc = pdf::load(&fixtures::text_pdf(1), None).unwrap();
        let page = pdf::page_ids(&doc)[0];
        let p = interpret(&doc, page);
        assert!(p.reliable());
        let text: String = p.glyphs.iter().filter_map(|g| g.text.clone()).collect();
        assert!(text.contains("Page 1"), "{text}");
        let first = &p.glyphs[0];
        assert!(first.size > 1.0 && first.angle.abs() < 0.01);
        // Glyphs advance left to right without overlapping.
        for pair in p.glyphs.windows(2) {
            if pair[0].op == pair[1].op {
                assert!(pair[1].origin[0] >= pair[0].origin[0]);
                assert!((pair[1].origin[0] - pair[0].end[0]).abs() < 0.01);
            }
        }
    }

    #[test]
    fn rotated_pages_report_visual_positions() {
        let rotated = crate::pages::rotate(&fixtures::text_pdf(1), 90, None, None).unwrap();
        let doc = pdf::load(&rotated, None).unwrap();
        let page = pdf::page_ids(&doc)[0];
        let p = interpret(&doc, page);
        let plain = interpret(
            &pdf::load(&fixtures::text_pdf(1), None).unwrap(),
            page_of(&fixtures::text_pdf(1)),
        );
        assert_eq!((p.width, p.height), (plain.height, plain.width));
        // /Rotate 90 turns the page clockwise: the text now runs downwards.
        assert!(
            (p.glyphs[0].angle + 90.0).abs() < 0.01,
            "{}",
            p.glyphs[0].angle
        );
        assert!(p.glyphs[0].bbox[2] <= p.width && p.glyphs[0].bbox[3] <= p.height);
    }

    fn page_of(bytes: &[u8]) -> ObjectId {
        pdf::page_ids(&pdf::load(bytes, None).unwrap())[0]
    }

    #[test]
    fn follows_forms_and_finds_images_and_paths() {
        let mut doc = lopdf::Document::with_version("1.7");
        let font = doc.add_object(lopdf::dictionary! {
            "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica",
        });
        let form = doc.add_object(lopdf::Stream::new(
            lopdf::dictionary! {
                "Type" => "XObject", "Subtype" => "Form",
                "BBox" => vec![0.into(), 0.into(), 100.into(), 100.into()],
                "Matrix" => vec![1.into(), 0.into(), 0.into(), 1.into(), 50.into(), 0.into()],
                "Resources" => lopdf::dictionary! { "Font" => lopdf::dictionary! { "F" => font } },
            },
            b"BT /F 10 Tf 0 0 Td (Hi) Tj ET".to_vec(),
        ));
        let content = doc.add_object(lopdf::Stream::new(
            lopdf::Dictionary::new(),
            b"q 10 10 50 20 re f Q /X Do /X Do BI /W 1 /H 1 /CS /G /BPC 8 ID \x80EI".to_vec(),
        ));
        let page = fixtures::single_page(
            &mut doc,
            content,
            lopdf::dictionary! {
                "XObject" => lopdf::dictionary! { "X" => form },
            },
        );
        let p = interpret(&doc, page);
        assert_eq!(
            p.streams.len(),
            3,
            "each drawing of the form is its own stream"
        );
        assert_eq!(p.glyphs.len(), 4);
        assert!((p.glyphs[0].origin[0] - 50.0).abs() < 0.01);
        assert_eq!(p.glyphs[0].stream, 1);
        assert_eq!(p.glyphs[2].stream, 2);
        assert_eq!(p.paths.len(), 1);
        assert_eq!(p.paths[0].bbox, [10.0, 10.0, 60.0, 30.0]);
        assert_eq!(p.images.len(), 1);
        assert!(p.images[0].id.is_none());
    }
}
