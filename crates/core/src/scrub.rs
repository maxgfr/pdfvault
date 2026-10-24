//! Taking things out of a page's content: glyphs (the text around them stays
//! exactly where it was), painted paths, whole images or the pixels of an
//! image under a box, and whole form XObjects. Streams are edited by
//! splicing bytes; a form XObject that changes is copied for this drawing
//! only, so other pages, or other drawings of it on this page, keep theirs.

use crate::compress;
use crate::content::{self, Op};
use crate::error::{Error, Result};
use crate::interp::{self, Matrix, Page};
use crate::pdf::{self, n};
use crate::raster::{self, Pixels};
use lopdf::{Dictionary, Document, Object, ObjectId, Stream};
use std::collections::{HashMap, HashSet};
use std::ops::Range;

#[derive(Debug, Default)]
pub struct Removal {
    /// Indices into [`Page::glyphs`].
    pub glyphs: HashSet<usize>,
    pub paths: HashSet<usize>,
    pub images: HashSet<usize>,
    pub forms: HashSet<usize>,
    /// Image → boxes (page space) whose pixels turn black.
    pub blackout: HashMap<usize, Vec<[f32; 4]>>,
}

impl Removal {
    pub fn is_empty(&self) -> bool {
        self.glyphs.is_empty()
            && self.paths.is_empty()
            && self.images.is_empty()
            && self.forms.is_empty()
            && self.blackout.is_empty()
    }
}

fn unsupported(msg: impl Into<String>) -> Error {
    Error::Unsupported(msg.into())
}

enum Item {
    Str(Vec<u8>),
    Num(f32),
}

/// A show operator rewritten without the glyphs in `removed`, each replaced by
/// a TJ adjustment of its own advance.
fn rebuild_show(
    page: &Page,
    op: &Op,
    glyphs: &[usize],
    removed: &HashSet<usize>,
) -> Result<Vec<u8>> {
    let elements: Vec<&Object> = match op.name.as_slice() {
        b"TJ" => match op.args.first() {
            Some(Object::Array(a)) => a.iter().collect(),
            _ => return Err(unsupported("TJ without an array")),
        },
        _ => op.args.last().into_iter().collect(),
    };
    let mut items: Vec<Item> = vec![];
    let push_num = |items: &mut Vec<Item>, v: f32| match items.last_mut() {
        Some(Item::Num(x)) => *x += v,
        _ => items.push(Item::Num(v)),
    };
    for (k, el) in elements.iter().enumerate() {
        match el {
            Object::String(s, _) => {
                let mine: Vec<usize> = glyphs
                    .iter()
                    .copied()
                    .filter(|&g| page.glyphs[g].elem == k)
                    .collect();
                let covered: usize = mine.iter().map(|&g| page.glyphs[g].len).sum();
                if covered != s.len() {
                    return Err(unsupported("a string was not fully read"));
                }
                for g in mine {
                    let gl = &page.glyphs[g];
                    if removed.contains(&g) {
                        let kern = gl.kern.ok_or_else(|| unsupported("zero font size"))?;
                        push_num(&mut items, -kern);
                    } else {
                        let bytes = &s[gl.start..gl.start + gl.len];
                        match items.last_mut() {
                            Some(Item::Str(cur)) => cur.extend_from_slice(bytes),
                            _ => items.push(Item::Str(bytes.to_vec())),
                        }
                    }
                }
            }
            other => {
                if let Some(v) = pdf::num(other) {
                    push_num(&mut items, v);
                }
            }
        }
    }
    let mut out = vec![];
    match op.name.as_slice() {
        b"'" => out.extend(b"T* "),
        b"\"" => {
            let (aw, ac) = (op.num(0).unwrap_or(0.0), op.num(1).unwrap_or(0.0));
            out.extend(format!("{} Tw {} Tc T* ", n(aw), n(ac)).into_bytes());
        }
        _ => {}
    }
    out.push(b'[');
    for it in &items {
        match it {
            Item::Str(s) => content::write_obj(&mut out, &Object::string_literal(s.clone())),
            Item::Num(v) if v.abs() > 1e-4 => {
                out.push(b' ');
                out.extend(n(*v).as_bytes());
                out.push(b' ');
            }
            Item::Num(_) => {}
        }
    }
    out.extend(b"] TJ");
    Ok(out)
}

/// A marked-content opening stripped of its replacement text (ActualText,
/// Alt, E), keeping the MCID that ties it to the structure tree.
fn strip_marked(doc: &Document, page: &Page, stream: usize, op: &Op) -> Vec<u8> {
    let props = match op.args.get(1) {
        Some(Object::Dictionary(d)) => Some(d.clone()),
        Some(Object::Name(nm)) => page.streams[stream]
            .resources
            .get(b"Properties")
            .ok()
            .and_then(|o| pdf::resolve(doc, o).as_dict().ok())
            .and_then(|d| d.get(nm).ok())
            .and_then(|o| pdf::resolve(doc, o).as_dict().ok())
            .cloned(),
        _ => None,
    };
    let mut out = vec![];
    content::write_obj(
        &mut out,
        op.args.first().unwrap_or(&Object::Name(b"Span".to_vec())),
    );
    match props.and_then(|d| d.get(b"MCID").ok().and_then(|v| v.as_i64().ok())) {
        Some(mcid) => out.extend(format!(" <</MCID {mcid}>> BDC").into_bytes()),
        None => out.extend(b" BMC"),
    }
    out
}

/// An inline image's dictionary with its abbreviations spelled out.
fn inline_stream(doc: &Document, res: &Dictionary, dict: &Dictionary, data: &[u8]) -> Stream {
    fn long_name(n: &[u8]) -> Vec<u8> {
        match n {
            b"G" => b"DeviceGray".to_vec(),
            b"RGB" => b"DeviceRGB".to_vec(),
            b"CMYK" => b"DeviceCMYK".to_vec(),
            b"I" => b"Indexed".to_vec(),
            b"AHx" => b"ASCIIHexDecode".to_vec(),
            b"A85" => b"ASCII85Decode".to_vec(),
            b"LZW" => b"LZWDecode".to_vec(),
            b"Fl" => b"FlateDecode".to_vec(),
            b"RL" => b"RunLengthDecode".to_vec(),
            b"CCF" => b"CCITTFaxDecode".to_vec(),
            b"DCT" => b"DCTDecode".to_vec(),
            other => other.to_vec(),
        }
    }
    fn expand(o: &Object) -> Object {
        match o {
            Object::Name(n) => Object::Name(long_name(n)),
            Object::Array(a) => Object::Array(a.iter().map(expand).collect()),
            other => other.clone(),
        }
    }
    let mut d = Dictionary::new();
    d.set("Type", "XObject");
    d.set("Subtype", "Image");
    for (k, v) in dict.iter() {
        let key: &[u8] = match k.as_slice() {
            b"W" => b"Width",
            b"H" => b"Height",
            b"BPC" => b"BitsPerComponent",
            b"CS" => b"ColorSpace",
            b"D" => b"Decode",
            b"DP" => b"DecodeParms",
            b"F" => b"Filter",
            b"IM" => b"ImageMask",
            b"I" => b"Interpolate",
            other => other,
        };
        let mut v = expand(v);
        if key == b"ColorSpace"
            && let Object::Name(nm) = &v
            && !matches!(
                nm.as_slice(),
                b"DeviceGray" | b"DeviceRGB" | b"DeviceCMYK" | b"Indexed"
            )
            && let Some(cs) = res
                .get(b"ColorSpace")
                .ok()
                .and_then(|o| pdf::resolve(doc, o).as_dict().ok())
                .and_then(|c| c.get(nm).ok())
        {
            v = pdf::resolve(doc, cs).clone();
        }
        d.set(key, v);
    }
    Stream::new(d, data.to_vec())
}

/// Pixel rectangles of `img` (w × h) covered by page boxes, through `m`.
fn pixel_boxes(m: &Matrix, w: u32, h: u32, boxes: &[[f32; 4]]) -> Result<Vec<[u32; 4]>> {
    let inv = interp::invert(m).ok_or_else(|| unsupported("flat image"))?;
    Ok(boxes
        .iter()
        .map(|b| {
            let u = interp::map_box(&inv, *b);
            let (u0, v0) = (u[0].clamp(0.0, 1.0), u[1].clamp(0.0, 1.0));
            let (u1, v1) = (u[2].clamp(0.0, 1.0), u[3].clamp(0.0, 1.0));
            // Image rows run top-down; round outwards so no sliver survives.
            [
                ((u0 * w as f32).floor() as u32).min(w),
                (((1.0 - v1) * h as f32).floor() as u32).min(h),
                ((u1 * w as f32).ceil() as u32).min(w),
                (((1.0 - v0) * h as f32).ceil() as u32).min(h),
            ]
        })
        .collect())
}

fn paint(px: &mut Pixels, boxes: &[[u32; 4]], value: u8) {
    for b in boxes {
        for y in b[1]..b[3] {
            for x in b[0]..b[2] {
                match px {
                    Pixels::Gray(g) => g.put_pixel(x, y, image::Luma([value])),
                    Pixels::Rgb(c) => c.put_pixel(x, y, image::Rgb([value; 3])),
                }
            }
        }
    }
}

fn encode_image(
    old: &Dictionary,
    px: &Pixels,
    keep_space: Option<&Object>,
    jpeg: bool,
) -> Result<Stream> {
    let mut d = compress::base_dict(old, px.width(), px.height());
    if jpeg {
        let data = raster::encode_jpeg(px, 92)?;
        let space: Object = match (keep_space, px) {
            (Some(k), _) => k.clone(),
            (None, Pixels::Gray(_)) => "DeviceGray".into(),
            (None, Pixels::Rgb(_)) => "DeviceRGB".into(),
        };
        d.set("ColorSpace", space);
        d.set("BitsPerComponent", 8);
        d.set("Filter", "DCTDecode");
        return Ok(Stream::new(d, data));
    }
    let (data, enc, _) = compress::encode_lossless(px, keep_space);
    for (k, v) in enc.iter() {
        d.set(k.clone(), v.clone());
    }
    Ok(Stream::new(d, data))
}

/// The image with the pixels under `boxes` black (and opaque, when it has a soft mask).
fn blackout(
    doc: &Document,
    s: &Stream,
    m: &Matrix,
    boxes: &[[f32; 4]],
) -> Result<(Stream, Option<Stream>)> {
    let dec = compress::decode_image(doc, s).map_err(|e| unsupported(format!("image: {e}")))?;
    let mut px = dec.px;
    let rects = pixel_boxes(m, px.width(), px.height(), boxes)?;
    paint(&mut px, &rects, 0);
    let jpeg = matches!(s.dict.get(b"Filter"), Ok(Object::Name(n)) if n == b"DCTDecode")
        || matches!(s.dict.get(b"Filter"), Ok(Object::Array(a)) if a.last().and_then(|o| o.as_name().ok()) == Some(b"DCTDecode"));
    let mut img = encode_image(&s.dict, &px, dec.keep_space.as_ref(), jpeg)?;
    let mut mask = None;
    if let Ok(sm) = s.dict.get(b"SMask") {
        let sm = pdf::resolve(doc, sm)
            .as_stream()
            .map_err(|_| unsupported("soft mask"))?;
        let dec =
            compress::decode_image(doc, sm).map_err(|e| unsupported(format!("soft mask: {e}")))?;
        let mut a = dec.px;
        let rects = pixel_boxes(m, a.width(), a.height(), boxes)?;
        paint(&mut a, &rects, 255);
        mask = Some(encode_image(&sm.dict, &a, None, false)?);
        img.dict.remove(b"SMask");
    }
    Ok((img, mask))
}

fn unique_name(taken: &HashSet<Vec<u8>>, prefix: &str) -> Vec<u8> {
    (0..)
        .map(|i| format!("{prefix}{i}").into_bytes())
        .find(|c| !taken.contains(c))
        .unwrap()
}

/// Apply `r` to the page `page_id` that `page` describes. On error nothing
/// is changed, so the caller can fall back (to rasterizing, for redaction).
pub fn apply(doc: &mut Document, page_id: ObjectId, page: &Page, r: &Removal) -> Result<()> {
    if r.is_empty() {
        return Ok(());
    }
    let n_streams = page.streams.len();
    let mut edits: Vec<Vec<(Range<usize>, Vec<u8>)>> = vec![vec![]; n_streams];
    let mut edited_ops: HashSet<(usize, usize)> = HashSet::new();

    // Streams whose drawing is removed altogether need no edits of their own.
    let mut dropped_do: HashSet<(usize, usize)> = HashSet::new();
    for &i in &r.forms {
        let f = &page.forms[i];
        dropped_do.insert((f.stream, f.op));
    }
    for &i in &r.images {
        let im = &page.images[i];
        dropped_do.insert((im.stream, im.op));
    }
    let mut dead = vec![false; n_streams];
    for s in 0..n_streams {
        if let Some((ps, op)) = page.streams[s].parent {
            dead[s] = dead[ps] || dropped_do.contains(&(ps, op));
        }
    }
    let touches_broken = r
        .glyphs
        .iter()
        .map(|&g| page.glyphs[g].stream)
        .chain(r.paths.iter().map(|&p| page.paths[p].stream))
        .chain(r.blackout.keys().map(|&i| page.images[i].stream))
        .chain(dropped_do.iter().map(|(s, _)| *s))
        .any(|s| page.streams[s].broken);
    if touches_broken {
        return Err(unsupported("unreadable content stream"));
    }

    // Glyphs: rebuild each show operator that loses some.
    let mut by_op: HashMap<(usize, usize), Vec<usize>> = HashMap::new();
    for (i, g) in page.glyphs.iter().enumerate() {
        by_op.entry((g.stream, g.op)).or_default().push(i);
    }
    let mut touched_ops: Vec<(usize, usize)> = r
        .glyphs
        .iter()
        .map(|&g| (page.glyphs[g].stream, page.glyphs[g].op))
        .collect::<HashSet<_>>()
        .into_iter()
        .filter(|(s, _)| !dead[*s])
        .collect();
    touched_ops.sort();
    for (s, o) in touched_ops {
        let op = &page.streams[s].ops[o];
        let bytes = rebuild_show(page, op, &by_op[&(s, o)], &r.glyphs)?;
        edits[s].push((op.span.clone(), bytes));
        edited_ops.insert((s, o));
    }
    let mut marked: Vec<usize> = r
        .glyphs
        .iter()
        .filter_map(|&g| page.glyphs[g].marked)
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    marked.sort();
    for m in marked {
        let mk = &page.marked[m];
        if dead[mk.stream] || !edited_ops.insert((mk.stream, mk.op)) {
            continue;
        }
        let op = &page.streams[mk.stream].ops[mk.op];
        edits[mk.stream].push((op.span.clone(), strip_marked(doc, page, mk.stream, op)));
    }

    for &p in &r.paths {
        let pu = &page.paths[p];
        if dead[pu.stream] {
            continue;
        }
        let ops = &page.streams[pu.stream].ops;
        edits[pu.stream].push((ops[pu.first].span.start..ops[pu.paint].span.end, vec![]));
    }
    for &(s, o) in &dropped_do {
        if dead[s] || !edited_ops.insert((s, o)) {
            continue;
        }
        edits[s].push((page.streams[s].ops[o].span.clone(), vec![]));
    }

    // Images partly under a box: new pixels, decided before anything changes.
    let mut new_images: Vec<(usize, usize, Stream, Option<Stream>)> = vec![];
    let mut blackouts: Vec<_> = r.blackout.iter().collect();
    blackouts.sort_by_key(|(i, _)| **i);
    for (&i, boxes) in blackouts {
        let im = &page.images[i];
        if dead[im.stream] || dropped_do.contains(&(im.stream, im.op)) {
            continue;
        }
        let source = match im.id {
            Some(id) => doc.get_object(id)?.as_stream()?.clone(),
            None => {
                let st = &page.streams[im.stream];
                let op = &st.ops[im.op];
                let data = content::inline_data(&st.bytes, op)
                    .ok_or_else(|| unsupported("inline image"))?;
                let dict = op
                    .args
                    .first()
                    .and_then(|o| o.as_dict().ok())
                    .ok_or_else(|| unsupported("inline image"))?;
                inline_stream(doc, &st.resources, dict, data)
            }
        };
        let (img, mask) = blackout(doc, &source, &im.matrix, boxes)?;
        new_images.push((im.stream, im.op, img, mask));
        edited_ops.insert((im.stream, im.op));
    }

    // From here on the document changes; nothing below can fail halfway
    // except on a malformed page tree.
    let mut pending: Vec<Vec<(usize, ObjectId)>> = vec![vec![]; n_streams];
    for (s, o, mut img, mask) in new_images {
        if let Some(mask) = mask {
            let mid = doc.add_object(mask);
            img.dict.set("SMask", mid);
        }
        pending[s].push((o, doc.add_object(img)));
    }
    for s in (0..n_streams).rev() {
        if dead[s] || (edits[s].is_empty() && pending[s].is_empty()) {
            continue;
        }
        let stream = &page.streams[s];
        let mut res = stream.resources.clone();
        let mut xobjects = res
            .get(b"XObject")
            .ok()
            .and_then(|o| pdf::resolve(doc, o).as_dict().ok())
            .cloned()
            .unwrap_or_default();
        let mut taken: HashSet<Vec<u8>> = xobjects.iter().map(|(k, _)| k.clone()).collect();
        let mut named = false;
        let mut list = std::mem::take(&mut edits[s]);
        for (o, id) in std::mem::take(&mut pending[s]) {
            let name = if s == 0 {
                pdf::add_resource(doc, page_id, b"XObject", "PVX", Object::Reference(id))?
            } else {
                let name = unique_name(&taken, "PVX");
                taken.insert(name.clone());
                xobjects.set(name.clone(), Object::Reference(id));
                named = true;
                name
            };
            let mut bytes = vec![];
            content::write_obj(&mut bytes, &Object::Name(name));
            bytes.extend(b" Do");
            list.retain(|(r, _)| *r != stream.ops[o].span);
            list.push((stream.ops[o].span.clone(), bytes));
        }
        let body = content::splice(&stream.bytes, list);
        let mut out = Stream::new(Dictionary::new(), body);
        if s == 0 {
            let _ = out.compress();
            let id = doc.add_object(out);
            doc.get_dictionary_mut(page_id)?.set("Contents", id);
            continue;
        }
        let (Some(xid), Some((ps, pop))) = (stream.xobject, stream.parent) else {
            continue;
        };
        let mut dict = doc.get_object(xid)?.as_stream()?.dict.clone();
        for k in [&b"Filter"[..], b"DecodeParms", b"Length"] {
            dict.remove(k);
        }
        // A form without /Resources used its parent's: the copy states them.
        if named || !dict.has(b"Resources") {
            res.set("XObject", xobjects);
            dict.set("Resources", res);
        }
        out.dict = dict;
        let _ = out.compress();
        let id = doc.add_object(out);
        pending[ps].push((pop, id));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures;
    use crate::testutil::qpdf_check;
    use lopdf::dictionary;

    fn texts(doc: &Document, page: ObjectId) -> Vec<(String, [f32; 2])> {
        interp::interpret(doc, page)
            .glyphs
            .iter()
            .map(|g| (g.text.clone().unwrap_or_default(), g.origin))
            .collect()
    }

    #[test]
    fn removes_glyphs_and_keeps_the_rest_in_place() {
        let mut doc = pdf::load(&fixtures::text_pdf(1), None).unwrap();
        let page = pdf::page_ids(&doc)[0];
        let p = interp::interpret(&doc, page);
        let line: String = p.glyphs.iter().filter_map(|g| g.text.clone()).collect();
        let at = line.find("quick").unwrap();
        let mut r = Removal::default();
        r.glyphs.extend(at..at + 5);
        apply(&mut doc, page, &p, &r).unwrap();
        let out = pdf::save(&mut doc).unwrap();
        qpdf_check(&out);
        let doc = pdf::load(&out, None).unwrap();
        let after = texts(&doc, pdf::page_ids(&doc)[0]);
        let before: Vec<_> = p
            .glyphs
            .iter()
            .enumerate()
            .filter(|(i, _)| !(at..at + 5).contains(i))
            .map(|(_, g)| (g.text.clone().unwrap_or_default(), g.origin))
            .collect();
        assert_eq!(after.len(), before.len());
        for (a, b) in after.iter().zip(&before) {
            assert_eq!(a.0, b.0);
            assert!(
                (a.1[0] - b.1[0]).abs() < 0.01 && (a.1[1] - b.1[1]).abs() < 0.01,
                "{a:?} vs {b:?}"
            );
        }
        let all: String = after.iter().map(|t| t.0.as_str()).collect();
        assert!(all.contains("The  brown fox"), "{all}");
    }

    #[test]
    fn copies_a_shared_form_and_blacks_out_image_pixels() {
        let mut b = fixtures::Builder::new();
        let img = b.flate_image(&fixtures::line_art(100, 100), None);
        let font = b.font_id;
        let form = b.doc.add_object(Stream::new(
            dictionary! {
                "Type" => "XObject", "Subtype" => "Form",
                "BBox" => vec![0.into(), 0.into(), 300.into(), 50.into()],
                "Resources" => dictionary! { "Font" => dictionary! { "F1" => font } },
            },
            b"BT /F1 20 Tf 0 10 Td (Secret) Tj ET".to_vec(),
        ));
        let ops = vec![
            lopdf::content::Operation::new("q", vec![]),
            lopdf::content::Operation::new(
                "cm",
                vec![
                    100.into(),
                    0.into(),
                    0.into(),
                    100.into(),
                    0.into(),
                    0.into(),
                ],
            ),
            lopdf::content::Operation::new("Do", vec!["Im0".into()]),
            lopdf::content::Operation::new("Q", vec![]),
            lopdf::content::Operation::new("Do", vec!["Im1".into()]),
        ];
        b.page((300.0, 300.0), ops, vec![img, form]);
        b.page(
            (300.0, 300.0),
            vec![lopdf::content::Operation::new("Do", vec!["Im1".into()])],
            vec![img, form],
        );
        let bytes = b.finish();
        let mut doc = pdf::load(&bytes, None).unwrap();
        let page = pdf::page_ids(&doc)[0];
        let _ = page;
        let p = interp::interpret(&doc, page);
        let mut r = Removal::default();
        r.glyphs.extend(0..p.glyphs.len());
        r.blackout.insert(0, vec![[0.0, 50.0, 50.0, 100.0]]);
        apply(&mut doc, page, &p, &r).unwrap();
        let out = pdf::save(&mut doc).unwrap();
        qpdf_check(&out);
        let doc = pdf::load(&out, None).unwrap();
        let ids = pdf::page_ids(&doc);
        let first: String = texts(&doc, ids[0]).into_iter().map(|t| t.0).collect();
        let second: String = texts(&doc, ids[1]).into_iter().map(|t| t.0).collect();
        assert_eq!(first, "");
        assert_eq!(second, "Secret", "the other page keeps the shared form");
        let q = interp::interpret(&doc, ids[0]);
        let im = q.images[0].id.unwrap();
        let s = doc.get_object(im).unwrap().as_stream().unwrap();
        let px = compress::decode_image(&doc, s).unwrap().px;
        let Pixels::Rgb(rgb) = px else { panic!("rgb") };
        assert_eq!(
            rgb.get_pixel(10, 10).0,
            [0, 0, 0],
            "top-left quarter is black"
        );
        assert_ne!(rgb.get_pixel(90, 90).0, [0, 0, 0]);
    }
}
