//! PDF compression: lossless cleanup, then image downsampling and re-encoding.
//! The `extreme` preset rasterizes pages; that part needs the renderer and is
//! driven by the engine through [`rebuild_from_page_images`].

use crate::error::{Error, Result};
use crate::pdf::{self, num, resolve};
use crate::raster::{self, Pixels};
use image::{GrayImage, RgbImage};
use lopdf::content::Content;
use lopdf::{Dictionary, Document, Object, ObjectId, Stream, dictionary};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::hash::{Hash, Hasher};

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Preset {
    Lossless,
    #[default]
    Balanced,
    Strong,
    Extreme,
}

impl Preset {
    /// (target DPI, JPEG quality) for the image pass; lossless has none.
    pub fn image_settings(self) -> Option<(f32, u8)> {
        match self {
            Preset::Lossless => None,
            Preset::Balanced => Some((150.0, 80)),
            Preset::Strong => Some((110.0, 65)),
            Preset::Extreme => Some((96.0, 55)),
        }
    }
    pub fn raster_settings(self) -> (f32, u8) {
        (110.0, 60)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CompressOptions {
    pub preset: Preset,
    /// Target resolution for images, overriding the preset.
    pub dpi: Option<f32>,
    /// JPEG quality 1-100, overriding the preset.
    pub quality: Option<u8>,
    pub grayscale: bool,
    pub strip_metadata: bool,
    pub remove_attachments: bool,
    pub password: Option<String>,
}

impl Default for CompressOptions {
    fn default() -> Self {
        CompressOptions {
            preset: Preset::Balanced,
            dpi: None,
            quality: None,
            grayscale: false,
            strip_metadata: true,
            remove_attachments: false,
            password: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ImageReport {
    pub object: String,
    pub width: u32,
    pub height: u32,
    pub new_width: u32,
    pub new_height: u32,
    /// Effective resolution where the image is drawn largest, if it is drawn.
    pub dpi: Option<f32>,
    pub bytes_before: usize,
    pub bytes_after: usize,
    /// jpeg, flate, indexed, kept or skipped.
    pub action: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CompressReport {
    pub preset: Option<Preset>,
    pub input_bytes: usize,
    pub output_bytes: usize,
    pub saved_bytes: usize,
    /// output / input, at most 1.
    pub ratio: f64,
    /// True when no smaller result was found and the input is returned as is.
    pub kept_original: bool,
    pub pages: usize,
    pub streams_recompressed: usize,
    pub duplicates_merged: usize,
    pub removed: Vec<String>,
    pub images: Vec<ImageReport>,
}

impl CompressReport {
    pub fn finish(&mut self, input: usize, output: usize) {
        self.input_bytes = input;
        self.output_bytes = output;
        self.saved_bytes = input.saturating_sub(output);
        self.ratio = if input == 0 {
            1.0
        } else {
            output as f64 / input as f64
        };
    }
}

const STREAM_LIMIT: usize = 512 << 20;

pub fn compress(input: &[u8], opts: &CompressOptions) -> Result<(Vec<u8>, CompressReport)> {
    if let Some(q) = opts.quality
        && !(1..=100).contains(&q)
    {
        return Err(Error::option("quality must be between 1 and 100"));
    }
    if let Some(d) = opts.dpi
        && !(10.0..=1200.0).contains(&d)
    {
        return Err(Error::option("dpi must be between 10 and 1200"));
    }
    let mut doc = pdf::load(input, opts.password.as_deref())?;
    let was_encrypted = doc.encryption_state.is_some();
    let mut report = CompressReport {
        preset: Some(opts.preset),
        pages: doc.get_pages().len(),
        ..Default::default()
    };

    cleanup(&mut doc, opts, &mut report);
    report.duplicates_merged += dedupe(&mut doc);

    let settings = opts
        .preset
        .image_settings()
        .map(|(d, q)| (opts.dpi.unwrap_or(d), opts.quality.unwrap_or(q)));
    let settings = match (settings, opts.dpi, opts.quality) {
        (None, Some(d), q) => Some((d, q.unwrap_or(85))),
        (None, None, Some(q)) => Some((f32::INFINITY, q)),
        (s, _, _) => s,
    };
    if let Some((dpi, quality)) = settings {
        recompress_images(&mut doc, dpi, quality, opts.grayscale, &mut report);
        report.duplicates_merged += dedupe(&mut doc);
    }
    report.streams_recompressed = recompress_streams(&mut doc);

    let out = pdf::save(&mut doc)?;
    // A decrypted file must never come back as the protected original.
    if out.len() >= input.len() && !was_encrypted {
        report.kept_original = true;
        report.finish(input.len(), input.len());
        return Ok((input.to_vec(), report));
    }
    report.finish(input.len(), out.len());
    Ok((out, report))
}

fn cleanup(doc: &mut Document, opts: &CompressOptions, report: &mut CompressReport) {
    let mut removed: BTreeMap<&str, usize> = BTreeMap::new();
    let page_ids = pdf::page_ids(doc);
    for id in &page_ids {
        if let Ok(page) = doc.get_dictionary_mut(*id) {
            if page.remove(b"Thumb").is_some() {
                *removed.entry("thumbnails").or_default() += 1;
            }
            if page.remove(b"AA").is_some() {
                *removed.entry("javascript").or_default() += 1;
            }
            if opts.strip_metadata {
                for k in [&b"Metadata"[..], b"PieceInfo"] {
                    if page.remove(k).is_some() {
                        *removed.entry("metadata").or_default() += 1;
                    }
                }
            }
        }
    }
    let root = doc.trailer.get(b"Root").and_then(Object::as_reference).ok();
    let names_id = root.and_then(|r| {
        doc.get_dictionary(r)
            .ok()?
            .get(b"Names")
            .ok()
            .and_then(|n| n.as_reference().ok())
    });
    if let Some(r) = root
        && let Ok(cat) = doc.get_dictionary_mut(r)
    {
        if cat.remove(b"AA").is_some() {
            *removed.entry("javascript").or_default() += 1;
        }
        let js_open = matches!(cat.get(b"OpenAction"), Ok(Object::Dictionary(d)) if d.get(b"S").and_then(Object::as_name).ok() == Some(b"JavaScript"));
        if js_open {
            cat.remove(b"OpenAction");
            *removed.entry("javascript").or_default() += 1;
        }
        if opts.strip_metadata {
            for k in [&b"Metadata"[..], b"PieceInfo"] {
                if cat.remove(k).is_some() {
                    *removed.entry("metadata").or_default() += 1;
                }
            }
        }
        if let Ok(Object::Dictionary(names)) = cat.get_mut(b"Names") {
            strip_names(names, opts, &mut removed);
        }
    }
    if let Some(n) = names_id
        && let Ok(names) = doc.get_dictionary_mut(n)
    {
        strip_names(names, opts, &mut removed);
    }
    if opts.strip_metadata && doc.trailer.remove(b"Info").is_some() {
        *removed.entry("metadata").or_default() += 1;
    }
    if opts.remove_attachments {
        for id in &page_ids {
            let annots = doc
                .get_dictionary(*id)
                .ok()
                .and_then(|p| p.get(b"Annots").ok().cloned());
            let Some(annots) = annots else { continue };
            let list = match resolve(doc, &annots).as_array() {
                Ok(a) => a.clone(),
                Err(_) => continue,
            };
            let kept: Vec<Object> = list
                .into_iter()
                .filter(|a| {
                    let d = resolve(doc, a).as_dict().ok();
                    let attach = d
                        .and_then(|d| d.get(b"Subtype").ok())
                        .and_then(|s| s.as_name().ok())
                        == Some(b"FileAttachment");
                    if attach {
                        *removed.entry("attachments").or_default() += 1;
                    }
                    !attach
                })
                .collect();
            if let Ok(p) = doc.get_dictionary_mut(*id) {
                p.set("Annots", kept);
            }
        }
    }
    if opts.strip_metadata {
        for obj in doc.objects.values_mut() {
            if let Object::Stream(s) = obj
                && s.dict.remove(b"Metadata").is_some()
            {
                *removed.entry("metadata").or_default() += 1;
            }
        }
    }
    report.removed = removed
        .into_iter()
        .map(|(k, v)| format!("{k}:{v}"))
        .collect();
}

fn strip_names(
    names: &mut Dictionary,
    opts: &CompressOptions,
    removed: &mut BTreeMap<&str, usize>,
) {
    if names.remove(b"JavaScript").is_some() {
        *removed.entry("javascript").or_default() += 1;
    }
    if opts.remove_attachments && names.remove(b"EmbeddedFiles").is_some() {
        *removed.entry("attachments").or_default() += 1;
    }
}

// ---------------------------------------------------------------- dedupe

fn hash_obj(obj: &Object, h: &mut impl Hasher) {
    match obj {
        Object::Null => 0u8.hash(h),
        Object::Boolean(b) => b.hash(h),
        Object::Integer(i) => i.hash(h),
        Object::Real(r) => r.to_bits().hash(h),
        Object::Name(n) => n.hash(h),
        Object::String(s, _) => s.hash(h),
        Object::Array(a) => a.iter().for_each(|o| hash_obj(o, h)),
        Object::Dictionary(d) => hash_dict(d, h),
        Object::Stream(s) => {
            hash_dict(&s.dict, h);
            s.content.hash(h);
        }
        Object::Reference(r) => r.hash(h),
    }
}

fn hash_dict(d: &Dictionary, h: &mut impl Hasher) {
    for (k, v) in d.iter() {
        if k != b"Length" {
            k.hash(h);
            hash_obj(v, h);
        }
    }
}

fn same(a: &Object, b: &Object) -> bool {
    match (a, b) {
        (Object::Stream(x), Object::Stream(y)) => {
            let strip = |d: &Dictionary| {
                let mut d = d.clone();
                d.remove(b"Length");
                d
            };
            x.content == y.content && strip(&x.dict) == strip(&y.dict)
        }
        _ => a == b,
    }
}

fn dedupable(obj: &Object) -> bool {
    match obj {
        Object::Stream(s) => !matches!(s.dict.get_type().ok(), Some(b"ObjStm" | b"XRef")),
        Object::Dictionary(d) => matches!(
            d.get_type().ok(),
            Some(
                b"Font" | b"FontDescriptor" | b"ExtGState" | b"Encoding" | b"Pattern" | b"Shading"
            )
        ),
        _ => false,
    }
}

/// Merge byte-identical streams and resource dictionaries, repeating until
/// stable since merging fonts' file streams makes their dictionaries equal.
pub fn dedupe(doc: &mut Document) -> usize {
    let mut total = 0;
    for _ in 0..4 {
        let mut buckets: HashMap<u64, Vec<ObjectId>> = HashMap::new();
        for (id, obj) in &doc.objects {
            if dedupable(obj) {
                let mut h = std::collections::hash_map::DefaultHasher::new();
                hash_obj(obj, &mut h);
                buckets.entry(h.finish()).or_default().push(*id);
            }
        }
        let mut map: HashMap<ObjectId, ObjectId> = HashMap::new();
        for ids in buckets.values().filter(|v| v.len() > 1) {
            let mut canon: Vec<ObjectId> = vec![];
            for id in ids {
                let obj = &doc.objects[id];
                match canon.iter().find(|c| same(&doc.objects[*c], obj)) {
                    Some(c) => {
                        map.insert(*id, *c);
                    }
                    None => canon.push(*id),
                }
            }
        }
        if map.is_empty() {
            break;
        }
        total += map.len();
        for id in map.keys() {
            doc.objects.remove(id);
        }
        for obj in doc.objects.values_mut() {
            remap(obj, &map);
        }
        remap_dict(&mut doc.trailer, &map);
    }
    total
}

fn remap(obj: &mut Object, map: &HashMap<ObjectId, ObjectId>) {
    match obj {
        Object::Reference(r) => {
            if let Some(n) = map.get(r) {
                *r = *n;
            }
        }
        Object::Array(a) => a.iter_mut().for_each(|o| remap(o, map)),
        Object::Dictionary(d) => remap_dict(d, map),
        Object::Stream(s) => remap_dict(&mut s.dict, map),
        _ => {}
    }
}

fn remap_dict(d: &mut Dictionary, map: &HashMap<ObjectId, ObjectId>) {
    for (_, v) in d.iter_mut() {
        remap(v, map);
    }
}

// ---------------------------------------------------------------- streams

fn single_filter(dict: &Dictionary) -> Option<Option<Vec<u8>>> {
    match dict.get(b"Filter") {
        Err(_) => Some(None),
        Ok(Object::Name(n)) => Some(Some(n.clone())),
        Ok(Object::Array(a)) if a.is_empty() => Some(None),
        Ok(Object::Array(a)) if a.len() == 1 => a[0].as_name().ok().map(|n| Some(n.to_vec())),
        _ => None,
    }
}

/// Re-deflate Flate streams at the maximum level and deflate plain ones,
/// keeping whichever is smaller. Predictor parameters stay valid because the
/// predicted bytes are recompressed untouched.
pub fn recompress_streams(doc: &mut Document) -> usize {
    let mut count = 0;
    for obj in doc.objects.values_mut() {
        let Object::Stream(s) = obj else { continue };
        if matches!(
            s.dict.get_type().ok(),
            Some(b"ObjStm" | b"XRef" | b"Metadata")
        ) {
            continue;
        }
        if strip_ascii_layers(s) {
            count += 1;
        }
        match single_filter(&s.dict) {
            Some(Some(f)) if f == b"FlateDecode" => {
                let Ok(raw) = raster::inflate(&s.content, STREAM_LIMIT) else {
                    continue;
                };
                let packed = raster::deflate(&raw);
                if packed.len() < s.content.len() {
                    s.set_content(packed);
                    count += 1;
                }
            }
            Some(None) if s.content.len() > 64 && !s.dict.has(b"DecodeParms") => {
                let packed = raster::deflate(&s.content);
                if packed.len() + 16 < s.content.len() {
                    s.dict.set("Filter", "FlateDecode");
                    s.set_content(packed);
                    count += 1;
                }
            }
            _ => {}
        }
    }
    count
}

// ---------------------------------------------------------------- image usage

type Matrix = [f32; 6];
const IDENTITY: Matrix = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

fn mul(a: &Matrix, b: &Matrix) -> Matrix {
    [
        a[0] * b[0] + a[1] * b[2],
        a[0] * b[1] + a[1] * b[3],
        a[2] * b[0] + a[3] * b[2],
        a[2] * b[1] + a[3] * b[3],
        a[4] * b[0] + a[5] * b[2] + b[4],
        a[4] * b[1] + a[5] * b[3] + b[5],
    ]
}

fn matrix_of(obj: Option<&Object>) -> Matrix {
    let Some(Object::Array(a)) = obj else {
        return IDENTITY;
    };
    let v: Vec<f32> = a.iter().filter_map(num).collect();
    if v.len() == 6 {
        [v[0], v[1], v[2], v[3], v[4], v[5]]
    } else {
        IDENTITY
    }
}

/// Largest size, in points, at which each image XObject is drawn.
pub fn image_usage(doc: &Document) -> HashMap<ObjectId, (f32, f32)> {
    let mut usage = HashMap::new();
    for page in pdf::page_ids(doc) {
        let content = doc.get_page_content(page);
        let res = pdf::inherited(doc, page, b"Resources");
        let res = res
            .as_ref()
            .and_then(|r| r.as_dict().ok())
            .cloned()
            .unwrap_or_default();
        walk_content(
            doc,
            &content,
            &res,
            IDENTITY,
            &mut usage,
            &mut HashSet::new(),
            0,
        );
    }
    usage
}

fn walk_content(
    doc: &Document,
    bytes: &[u8],
    resources: &Dictionary,
    base: Matrix,
    usage: &mut HashMap<ObjectId, (f32, f32)>,
    stack: &mut HashSet<ObjectId>,
    depth: usize,
) {
    if depth > 12 {
        return;
    }
    let Ok(content) = Content::decode(bytes) else {
        return;
    };
    let xobjects = resources
        .get(b"XObject")
        .ok()
        .map(|o| resolve(doc, o))
        .and_then(|o| o.as_dict().ok());
    let mut ctm = base;
    let mut saved: Vec<Matrix> = vec![];
    for op in content.operations {
        match op.operator.as_str() {
            "q" => saved.push(ctm),
            "Q" => ctm = saved.pop().unwrap_or(base),
            "cm" => {
                let m = matrix_of(Some(&Object::Array(op.operands.clone())));
                ctm = mul(&m, &ctm);
            }
            "Do" => {
                let (Some(name), Some(xo)) =
                    (op.operands.first().and_then(|o| o.as_name().ok()), xobjects)
                else {
                    continue;
                };
                let Ok(Object::Reference(id)) = xo.get(name) else {
                    continue;
                };
                let Ok(stream) = doc.get_object(*id).and_then(Object::as_stream) else {
                    continue;
                };
                match stream.dict.get(b"Subtype").and_then(Object::as_name).ok() {
                    Some(b"Image") => {
                        let w = (ctm[0] * ctm[0] + ctm[1] * ctm[1]).sqrt();
                        let h = (ctm[2] * ctm[2] + ctm[3] * ctm[3]).sqrt();
                        let e = usage.entry(*id).or_insert((0.0, 0.0));
                        e.0 = e.0.max(w);
                        e.1 = e.1.max(h);
                    }
                    Some(b"Form") if stack.insert(*id) => {
                        let m = matrix_of(stream.dict.get(b"Matrix").ok().map(|o| resolve(doc, o)));
                        let res = stream
                            .dict
                            .get(b"Resources")
                            .ok()
                            .map(|o| resolve(doc, o))
                            .and_then(|o| o.as_dict().ok())
                            .unwrap_or(resources)
                            .clone();
                        if let Ok(inner) = stream.decompressed_content() {
                            walk_content(doc, &inner, &res, mul(&m, &ctm), usage, stack, depth + 1);
                        } else {
                            walk_content(
                                doc,
                                &stream.content,
                                &res,
                                mul(&m, &ctm),
                                usage,
                                stack,
                                depth + 1,
                            );
                        }
                        stack.remove(id);
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
}

// ---------------------------------------------------------------- images

enum Space {
    Gray,
    Rgb,
    Indexed { base_rgb: bool, palette: Vec<u8> },
}

fn color_space(doc: &Document, obj: &Object) -> Option<(Space, Option<Object>)> {
    let obj = resolve(doc, obj);
    match obj {
        Object::Name(n) => match n.as_slice() {
            b"DeviceGray" | b"CalGray" | b"G" => Some((Space::Gray, None)),
            b"DeviceRGB" | b"CalRGB" | b"RGB" => Some((Space::Rgb, None)),
            _ => None,
        },
        Object::Array(a) => {
            let kind = a.first()?.as_name().ok()?;
            match kind {
                b"ICCBased" => {
                    let s = resolve(doc, a.get(1)?).as_stream().ok()?;
                    let comps = s.dict.get(b"N").ok()?.as_i64().ok()?;
                    // Keep the profile when the component count is unchanged.
                    match comps {
                        1 => Some((Space::Gray, Some(obj.clone()))),
                        3 => Some((Space::Rgb, Some(obj.clone()))),
                        _ => None,
                    }
                }
                b"CalGray" => Some((Space::Gray, None)),
                b"CalRGB" => Some((Space::Rgb, None)),
                b"Indexed" | b"I" => {
                    let (base, _) = color_space(doc, a.get(1)?)?;
                    let base_rgb = match base {
                        Space::Rgb => true,
                        Space::Gray => false,
                        Space::Indexed { .. } => return None,
                    };
                    let lookup = match resolve(doc, a.get(3)?) {
                        Object::String(s, _) => s.clone(),
                        Object::Stream(s) => s.decompressed_content().ok()?,
                        _ => return None,
                    };
                    Some((
                        Space::Indexed {
                            base_rgb,
                            palette: lookup,
                        },
                        None,
                    ))
                }
                _ => None,
            }
        }
        _ => None,
    }
}

pub(crate) struct Decoded {
    pub(crate) px: Pixels,
    /// Color space to write back for this many components, when worth keeping.
    pub(crate) keep_space: Option<Object>,
}

pub(crate) fn decode_image(doc: &Document, s: &Stream) -> std::result::Result<Decoded, String> {
    let d = &s.dict;
    if d.get(b"ImageMask")
        .and_then(Object::as_bool)
        .unwrap_or(false)
    {
        return Err("stencil mask".into());
    }
    // /Decode [1 0] inverts a gray image; any other mapping is left alone.
    let invert = match d.get(b"Decode") {
        Err(_) => false,
        Ok(Object::Array(a))
            if a.len() == 2 && num(&a[0]) == Some(1.0) && num(&a[1]) == Some(0.0) =>
        {
            true
        }
        Ok(_) => return Err("custom /Decode".into()),
    };
    if matches!(d.get(b"Mask"), Ok(Object::Array(_))) {
        return Err("color-key mask".into());
    }
    let w = d
        .get(b"Width")
        .and_then(Object::as_i64)
        .map_err(|_| "no width")? as usize;
    let h = d
        .get(b"Height")
        .and_then(Object::as_i64)
        .map_err(|_| "no height")? as usize;
    if w == 0 || h == 0 || w * h > 100_000_000 {
        return Err("unreasonable dimensions".into());
    }
    let (content, filter) = unwrap_filters(s)?;
    let cs = d.get(b"ColorSpace").map_err(|_| "no color space")?;
    let (space, keep_space) = color_space(doc, cs).ok_or("unsupported color space")?;
    if invert && !matches!(space, Space::Gray) {
        return Err("custom /Decode".into());
    }
    let bits = d
        .get(b"BitsPerComponent")
        .and_then(Object::as_i64)
        .unwrap_or(8) as u8;

    if filter.as_deref() == Some(b"DCTDecode") {
        if matches!(space, Space::Indexed { .. }) {
            return Err("indexed JPEG".into());
        }
        let img = image::load_from_memory_with_format(&content, image::ImageFormat::Jpeg)
            .map_err(|e| e.to_string())?;
        let px = match space {
            Space::Gray => Pixels::Gray(img.to_luma8()),
            _ => Pixels::Rgb(img.to_rgb8()),
        };
        if px.width() as usize != w || px.height() as usize != h {
            return Err("JPEG size differs from dictionary".into());
        }
        return Ok(Decoded {
            px: if invert { negate(px) } else { px },
            keep_space,
        });
    }
    let raw = match filter.as_deref() {
        None => content,
        Some(b"FlateDecode") => {
            if content.len() == s.content.len() {
                s.decompressed_content().map_err(|e| e.to_string())?
            } else {
                // Predictors only ever apply to a lone Flate filter here.
                raster::inflate(&content, STREAM_LIMIT).map_err(|e| e.to_string())?
            }
        }
        Some(other) => return Err(format!("{} filter", String::from_utf8_lossy(other))),
    };
    let px = match space {
        Space::Gray => {
            let v = raster::unpack(&raw, w, h, 1, bits).ok_or("bad sample data")?;
            let scale = 255 / ((1u16 << bits) - 1) as u8;
            let v = if bits < 8 {
                v.into_iter().map(|x| x * scale).collect()
            } else {
                v
            };
            Pixels::Gray(GrayImage::from_raw(w as u32, h as u32, v).ok_or("bad gray data")?)
        }
        Space::Rgb => {
            if bits != 8 {
                return Err(format!("{bits}-bit RGB"));
            }
            let v = raster::unpack(&raw, w, h, 3, 8).ok_or("bad sample data")?;
            Pixels::Rgb(RgbImage::from_raw(w as u32, h as u32, v).ok_or("bad rgb data")?)
        }
        Space::Indexed { base_rgb, palette } => {
            let idx = raster::unpack(&raw, w, h, 1, bits).ok_or("bad index data")?;
            let comps = if base_rgb { 3 } else { 1 };
            let mut v = Vec::with_capacity(w * h * comps);
            for i in idx {
                let o = i as usize * comps;
                if o + comps > palette.len() {
                    return Err("index outside palette".into());
                }
                v.extend_from_slice(&palette[o..o + comps]);
            }
            if base_rgb {
                Pixels::Rgb(RgbImage::from_raw(w as u32, h as u32, v).ok_or("bad rgb data")?)
            } else {
                Pixels::Gray(GrayImage::from_raw(w as u32, h as u32, v).ok_or("bad gray data")?)
            }
        }
    };
    let px = if invert { negate(px) } else { px };
    Ok(Decoded {
        px,
        keep_space: None,
    })
}

fn negate(px: Pixels) -> Pixels {
    match px {
        Pixels::Gray(mut i) => {
            image::imageops::invert(&mut i);
            Pixels::Gray(i)
        }
        other => other,
    }
}

/// Strip leading Flate and ASCII layers off a filter chain such as
/// [/ASCII85Decode /DCTDecode]; return the remaining bytes and the one
/// filter left.
fn unwrap_filters(s: &Stream) -> std::result::Result<(Vec<u8>, Option<Vec<u8>>), String> {
    let names: Vec<Vec<u8>> = match s.dict.get(b"Filter") {
        Err(_) => vec![],
        Ok(Object::Name(n)) => vec![n.clone()],
        Ok(Object::Array(a)) => a
            .iter()
            .map(|o| o.as_name().map(<[u8]>::to_vec))
            .collect::<std::result::Result<_, _>>()
            .map_err(|_| "bad /Filter")?,
        Ok(_) => return Err("bad /Filter".into()),
    };
    let mut data = s.content.clone();
    let mut rest: &[Vec<u8>] = &names;
    while let [first, tail @ ..] = rest {
        let index = names.len() - rest.len();
        if let Some(decoded) = raster::decode_ascii_filter(first, &data) {
            data = decoded.map_err(|e| e.to_string())?;
        } else if !tail.is_empty() && first == b"FlateDecode" && !has_parms(&s.dict, index) {
            data = raster::inflate(&data, STREAM_LIMIT).map_err(|e| e.to_string())?;
        } else {
            break;
        }
        rest = tail;
    }
    match rest {
        [] => Ok((data, None)),
        [only] => Ok((data, Some(only.clone()))),
        _ => Err("filter chain".into()),
    }
}

/// Whether filter number `i` has decode parameters.
fn has_parms(dict: &Dictionary, i: usize) -> bool {
    match dict.get(b"DecodeParms") {
        Ok(Object::Array(p)) => p.get(i).is_some_and(|x| !matches!(x, Object::Null)),
        Ok(Object::Dictionary(_)) => i == 0,
        _ => false,
    }
}

/// Drop ASCII85/ASCIIHex layers from a stream: they only make binary data
/// 25-100% larger. Returns whether the stream changed.
fn strip_ascii_layers(s: &mut Stream) -> bool {
    let names: Vec<Object> = match s.dict.get(b"Filter") {
        Ok(Object::Name(n)) => vec![Object::Name(n.clone())],
        Ok(Object::Array(a)) => a.clone(),
        _ => return false,
    };
    let mut data = s.content.clone();
    let mut dropped = 0;
    for (i, n) in names.iter().enumerate() {
        let Ok(name) = n.as_name() else { break };
        if has_parms(&s.dict, i) {
            break;
        }
        match raster::decode_ascii_filter(name, &data) {
            Some(Ok(d)) => {
                data = d;
                dropped += 1;
            }
            _ => break,
        }
    }
    if dropped == 0 {
        return false;
    }
    let keep = |list: Vec<Object>, key: &str, s: &mut Stream| match list.len() {
        0 => {
            s.dict.remove(key.as_bytes());
        }
        1 => s.dict.set(key, list[0].clone()),
        _ => s.dict.set(key, list),
    };
    keep(names[dropped..].to_vec(), "Filter", s);
    if let Ok(Object::Array(p)) = s.dict.get(b"DecodeParms").cloned() {
        keep(p.into_iter().skip(dropped).collect(), "DecodeParms", s);
    }
    s.set_content(data);
    true
}

pub(crate) fn base_dict(old: &Dictionary, w: u32, h: u32) -> Dictionary {
    let mut d = dictionary! {
        "Type" => "XObject", "Subtype" => "Image", "Width" => w as i64, "Height" => h as i64,
    };
    for k in [
        &b"SMask"[..],
        b"Mask",
        b"Intent",
        b"Interpolate",
        b"SMaskInData",
        b"OC",
        b"StructParent",
        b"ID",
        b"Name",
    ] {
        if let Ok(v) = old.get(k) {
            d.set(k, v.clone());
        }
    }
    d
}

/// Encode pixels lossless: a packed palette when ≤256 colors, else 8-bit with PNG predictors.
pub(crate) fn encode_lossless(
    px: &Pixels,
    keep_space: Option<&Object>,
) -> (Vec<u8>, Dictionary, &'static str) {
    let mut d = Dictionary::new();
    if let Some(p) = raster::palette(px).filter(|p| p.bits < 8 || px.components() == 3) {
        let base: Object = if px.components() == 3 {
            "DeviceRGB".into()
        } else {
            "DeviceGray".into()
        };
        let hival = (p.colors.len() / px.components()) as i64 - 1;
        d.set(
            "ColorSpace",
            vec![
                "Indexed".into(),
                base,
                hival.into(),
                Object::string_literal(p.colors),
            ],
        );
        d.set("BitsPerComponent", p.bits as i64);
        let row = raster::row_bytes(px.width(), 1, p.bits);
        let data = raster::deflate(&raster::png_predict(&p.data, row, 1));
        d.set("Filter", "FlateDecode");
        d.set(
            "DecodeParms",
            dictionary! { "Predictor" => 15, "Colors" => 1, "BitsPerComponent" => p.bits as i64, "Columns" => px.width() as i64 },
        );
        return (data, d, "indexed");
    }
    let comps = px.components();
    d.set("ColorSpace", space_for(px, keep_space));
    d.set("BitsPerComponent", 8);
    let data = raster::deflate(&raster::png_predict(
        px.raw(),
        px.width() as usize * comps,
        comps,
    ));
    d.set("Filter", "FlateDecode");
    d.set(
        "DecodeParms",
        dictionary! { "Predictor" => 15, "Colors" => comps as i64, "BitsPerComponent" => 8, "Columns" => px.width() as i64 },
    );
    (data, d, "flate")
}

fn space_for(px: &Pixels, keep: Option<&Object>) -> Object {
    match (px, keep) {
        (_, Some(k)) => k.clone(),
        (Pixels::Gray(_), None) => "DeviceGray".into(),
        (Pixels::Rgb(_), None) => "DeviceRGB".into(),
    }
}

fn recompress_images(
    doc: &mut Document,
    target_dpi: f32,
    quality: u8,
    gray: bool,
    report: &mut CompressReport,
) {
    let usage = image_usage(doc);
    // Soft masks follow their parent's resolution.
    let mut smask_parent: HashMap<ObjectId, ObjectId> = HashMap::new();
    let ids: Vec<ObjectId> = doc
        .objects
        .iter()
        .filter_map(|(id, o)| {
            let s = o.as_stream().ok()?;
            (s.dict.get(b"Subtype").and_then(Object::as_name).ok() == Some(b"Image")).then_some(*id)
        })
        .collect();
    for id in &ids {
        if let Ok(Object::Reference(m)) = doc.objects[id]
            .as_stream()
            .map(|s| s.dict.get(b"SMask").cloned().unwrap_or(Object::Null))
        {
            smask_parent.insert(m, *id);
        }
    }
    let dpi_of = |doc: &Document, id: ObjectId| -> Option<f32> {
        let s = doc.objects.get(&id)?.as_stream().ok()?;
        let w = s.dict.get(b"Width").ok()?.as_i64().ok()? as f32;
        let h = s.dict.get(b"Height").ok()?.as_i64().ok()? as f32;
        let (pw, ph) = usage.get(&id)?;
        if *pw <= 0.01 || *ph <= 0.01 {
            return None;
        }
        Some((w / (pw / 72.0)).max(h / (ph / 72.0)))
    };

    for id in ids {
        let is_mask = smask_parent.contains_key(&id);
        let dpi = if is_mask {
            let parent = smask_parent[&id];
            dpi_of(doc, parent).and_then(|pd| {
                let ps = doc.objects.get(&parent)?.as_stream().ok()?;
                let pw = ps.dict.get(b"Width").ok()?.as_i64().ok()? as f32;
                let mw = doc
                    .objects
                    .get(&id)?
                    .as_stream()
                    .ok()?
                    .dict
                    .get(b"Width")
                    .ok()?
                    .as_i64()
                    .ok()? as f32;
                Some(pd * mw / pw)
            })
        } else {
            dpi_of(doc, id)
        };
        let stream = doc.objects[&id].as_stream().expect("image stream").clone();
        let before = stream.content.len();
        let mut entry = ImageReport {
            object: format!("{} {}", id.0, id.1),
            dpi,
            bytes_before: before,
            ..Default::default()
        };
        let decoded = match decode_image(doc, &stream) {
            Ok(d) => d,
            Err(reason) => {
                entry.width = stream
                    .dict
                    .get(b"Width")
                    .and_then(Object::as_i64)
                    .unwrap_or(0) as u32;
                entry.height = stream
                    .dict
                    .get(b"Height")
                    .and_then(Object::as_i64)
                    .unwrap_or(0) as u32;
                entry.new_width = entry.width;
                entry.new_height = entry.height;
                entry.bytes_after = before;
                entry.action = "skipped".into();
                entry.reason = Some(reason);
                report.images.push(entry);
                continue;
            }
        };
        let mut px = decoded.px;
        let mut keep_space = decoded.keep_space;
        entry.width = px.width();
        entry.height = px.height();
        if gray && !is_mask && px.components() == 3 {
            px = px.to_gray();
            keep_space = None;
        }
        if let Some(d) = dpi
            && d > target_dpi * 1.15
        {
            let scale = target_dpi / d;
            let nw = ((px.width() as f32 * scale).round() as u32).max(1);
            let nh = ((px.height() as f32 * scale).round() as u32).max(1);
            // Flat-color art keeps its palette: resampling would otherwise add
            // in-between shades and push it from a tiny palette to a full image.
            let flat = raster::palette(&px).map(|p| p.colors);
            px = px.resize(nw, nh);
            if let Some(colors) = flat {
                px = raster::snap_to_palette(&px, &colors);
            }
        }
        px = px.simplify();
        if keep_space.is_some() && px.components() == 1 {
            // An RGB profile no longer matches gray samples.
            keep_space = None;
        }
        entry.new_width = px.width();
        entry.new_height = px.height();

        let was_jpeg = single_filter(&stream.dict).flatten().as_deref() == Some(b"DCTDecode");
        let (lossless, mut ldict, lkind) = encode_lossless(&px, keep_space.as_ref());
        let mut best: (Vec<u8>, Dictionary, &str) = (lossless, std::mem::take(&mut ldict), lkind);
        let few_colors = lkind == "indexed";
        // A lossless original only turns lossy when that clearly pays off.
        if !is_mask
            && !few_colors
            && let Ok(jpg) = raster::encode_jpeg(&px, quality)
            && (was_jpeg || jpg.len() * 2 < best.0.len())
            && jpg.len() < best.0.len()
        {
            let mut d = Dictionary::new();
            d.set("ColorSpace", space_for(&px, keep_space.as_ref()));
            d.set("BitsPerComponent", 8);
            d.set("Filter", "DCTDecode");
            best = (jpg, d, "jpeg");
        }
        if best.0.len() < before {
            let mut dict = base_dict(&stream.dict, px.width(), px.height());
            for (k, v) in best.1.iter() {
                dict.set(k.clone(), v.clone());
            }
            entry.bytes_after = best.0.len();
            entry.action = best.2.into();
            doc.objects
                .insert(id, Object::Stream(Stream::new(dict, best.0)));
        } else {
            entry.new_width = entry.width;
            entry.new_height = entry.height;
            entry.bytes_after = before;
            entry.action = "kept".into();
        }
        report.images.push(entry);
    }
}

// ---------------------------------------------------------------- extreme

/// One rasterized page: a JPEG plus the size it was rendered at.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageImage {
    pub width: u32,
    pub height: u32,
    #[serde(default)]
    pub gray: bool,
}

/// Replace every page by a full-page JPEG (already rendered by the engine),
/// keeping each page's visible size. Used by the `extreme` preset.
pub fn rebuild_from_page_images(
    input: &[u8],
    password: Option<&str>,
    pages: &[(PageImage, Vec<u8>)],
) -> Result<Vec<u8>> {
    let src = pdf::load(input, password)?;
    let ids = pdf::page_ids(&src);
    if ids.len() != pages.len() {
        return Err(Error::option(format!(
            "{} page images for {} pages",
            pages.len(),
            ids.len()
        )));
    }
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let mut kids = vec![];
    for (i, (meta, jpeg)) in pages.iter().enumerate() {
        let b = pdf::page_box(&src, ids[i]);
        let rot = pdf::page_rotation(&src, ids[i]);
        let (mut w, mut h) = (b[2] - b[0], b[3] - b[1]);
        if rot == 90 || rot == 270 {
            std::mem::swap(&mut w, &mut h);
        }
        let img = doc.add_object(Stream::new(
            dictionary! {
                "Type" => "XObject", "Subtype" => "Image", "Width" => meta.width as i64, "Height" => meta.height as i64,
                "ColorSpace" => if meta.gray { "DeviceGray" } else { "DeviceRGB" }, "BitsPerComponent" => 8,
                "Filter" => "DCTDecode",
            },
            jpeg.clone(),
        ));
        let ops = format!("q {} 0 0 {} 0 0 cm /Im0 Do Q", pdf::n(w), pdf::n(h)).into_bytes();
        let content = doc.add_object(Stream::new(Dictionary::new(), ops));
        let page = doc.add_object(dictionary! {
            "Type" => "Page", "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), w.into(), h.into()],
            "Contents" => content,
            "Resources" => dictionary! { "XObject" => dictionary! { "Im0" => img } },
        });
        kids.push(Object::Reference(page));
    }
    let count = kids.len() as i64;
    doc.objects.insert(
        pages_id,
        dictionary! { "Type" => "Pages", "Kids" => kids, "Count" => count }.into(),
    );
    let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
    doc.trailer.set("Root", catalog);
    pdf::save(&mut doc)
}

/// Replace only the given pages (0-based) by full-page JPEGs, in place: the
/// rest of the document keeps its text, links and outline. Everything the
/// replaced pages held goes, annotations and form fields included, so the
/// output carries nothing of them but pixels. Used by redaction.
pub fn replace_pages_with_images(
    input: &[u8],
    password: Option<&str>,
    pages: &[(usize, PageImage, Vec<u8>)],
) -> Result<Vec<u8>> {
    let mut doc = pdf::load(input, password)?;
    let ids = pdf::page_ids(&doc);
    let mut dropped: HashSet<ObjectId> = Default::default();
    for (index, meta, jpeg) in pages {
        let id = *ids
            .get(*index)
            .ok_or_else(|| Error::option(format!("page {} does not exist", index + 1)))?;
        let b = pdf::page_box(&doc, id);
        let (mut w, mut h) = (b[2] - b[0], b[3] - b[1]);
        if pdf::page_rotation(&doc, id) % 180 == 90 {
            std::mem::swap(&mut w, &mut h);
        }
        let img = doc.add_object(Stream::new(
            dictionary! {
                "Type" => "XObject", "Subtype" => "Image", "Width" => meta.width as i64, "Height" => meta.height as i64,
                "ColorSpace" => if meta.gray { "DeviceGray" } else { "DeviceRGB" }, "BitsPerComponent" => 8,
                "Filter" => "DCTDecode",
            },
            jpeg.clone(),
        ));
        let ops = format!("q {} 0 0 {} 0 0 cm /Im0 Do Q", pdf::n(w), pdf::n(h)).into_bytes();
        let content = doc.add_object(Stream::new(Dictionary::new(), ops));
        let page = doc.get_dictionary_mut(id)?;
        if let Ok(Object::Array(annots)) = page.get(b"Annots") {
            dropped.extend(annots.iter().filter_map(|a| a.as_reference().ok()));
        }
        for key in [
            &b"Annots"[..],
            b"CropBox",
            b"TrimBox",
            b"BleedBox",
            b"ArtBox",
            b"Rotate",
            b"StructParents",
            b"Thumb",
            b"Group",
            b"PieceInfo",
            b"Metadata",
        ] {
            page.remove(key);
        }
        page.set("MediaBox", vec![0.into(), 0.into(), w.into(), h.into()]);
        page.set("Contents", content);
        page.set(
            "Resources",
            dictionary! { "XObject" => dictionary! { "Im0" => img } },
        );
    }
    if !dropped.is_empty() {
        prune_fields(&mut doc, &dropped);
    }
    pdf::save(&mut doc)
}

/// Drop form fields whose widgets were removed, so their values go too.
pub(crate) fn prune_fields(doc: &mut Document, dropped: &HashSet<ObjectId>) {
    fn keep(doc: &mut Document, id: ObjectId, dropped: &HashSet<ObjectId>, depth: usize) -> bool {
        if dropped.contains(&id) || depth > 32 {
            return false;
        }
        let kids: Option<Vec<Object>> = doc
            .get_dictionary(id)
            .ok()
            .and_then(|d| d.get(b"Kids").ok())
            .and_then(|k| k.as_array().ok())
            .cloned();
        let Some(kids) = kids else {
            return true;
        };
        let left: Vec<Object> = kids
            .into_iter()
            .filter(|k| {
                k.as_reference()
                    .map(|r| keep(doc, r, dropped, depth + 1))
                    .unwrap_or(true)
            })
            .collect();
        let empty = left.is_empty();
        if let Ok(d) = doc.get_dictionary_mut(id) {
            d.set("Kids", left);
        }
        !empty
    }
    let Some(acro) = doc
        .catalog()
        .ok()
        .and_then(|c| c.get(b"AcroForm").ok())
        .cloned()
    else {
        return;
    };
    let mut form = match &acro {
        Object::Reference(id) => match doc.get_dictionary(*id) {
            Ok(d) => d.clone(),
            Err(_) => return,
        },
        Object::Dictionary(d) => d.clone(),
        _ => return,
    };
    let fields = form
        .get(b"Fields")
        .ok()
        .and_then(|f| f.as_array().ok())
        .cloned()
        .unwrap_or_default();
    let left: Vec<Object> = fields
        .into_iter()
        .filter(|f| {
            f.as_reference()
                .map(|r| keep(doc, r, dropped, 0))
                .unwrap_or(true)
        })
        .collect();
    form.set("Fields", left);
    match acro {
        Object::Reference(id) => {
            doc.objects.insert(id, form.into());
        }
        _ => {
            if let Ok(c) = doc.catalog_mut() {
                c.set("AcroForm", form);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures;

    fn opts(preset: Preset) -> CompressOptions {
        CompressOptions {
            preset,
            ..Default::default()
        }
    }

    #[test]
    fn photo_shrinks_and_stays_valid() {
        let input = fixtures::photo_pdf();
        let (out, rep) = compress(&input, &opts(Preset::Balanced)).unwrap();
        assert!(
            out.len() < input.len() / 2,
            "{} -> {}",
            input.len(),
            out.len()
        );
        assert!(!rep.kept_original);
        let doc = pdf::load(&out, None).unwrap();
        assert_eq!(doc.get_pages().len(), 2);
        let jpeg = rep.images.iter().find(|i| i.width == 1500).unwrap();
        assert_eq!(jpeg.action, "jpeg");
        assert!(
            jpeg.new_width < 1500 && jpeg.new_width >= 700,
            "{}",
            jpeg.new_width
        );
        crate::testutil::qpdf_check(&out);
    }

    #[test]
    fn stronger_presets_are_smaller() {
        let input = fixtures::photo_pdf();
        let b = compress(&input, &opts(Preset::Balanced)).unwrap().0.len();
        let s = compress(&input, &opts(Preset::Strong)).unwrap().0.len();
        assert!(s < b, "strong {s} >= balanced {b}");
    }

    #[test]
    fn lossless_keeps_images_and_still_shrinks() {
        let input = fixtures::photo_pdf();
        let (out, rep) = compress(&input, &opts(Preset::Lossless)).unwrap();
        assert!(rep.images.is_empty());
        assert!(out.len() <= input.len());
        crate::testutil::qpdf_check(&out);
    }

    #[test]
    fn line_art_stays_lossless() {
        let input = fixtures::lineart_pdf();
        let (out, rep) = compress(&input, &opts(Preset::Strong)).unwrap();
        assert!(out.len() < input.len());
        assert!(
            rep.images.iter().all(|i| i.action == "indexed"),
            "{:?}",
            rep.images
        );
        crate::testutil::qpdf_check(&out);
    }

    #[test]
    fn never_grows() {
        let input = fixtures::text_pdf(1);
        let (out, rep) = compress(&input, &opts(Preset::Balanced)).unwrap();
        assert!(out.len() <= input.len());
        if rep.kept_original {
            assert_eq!(out, input);
        }
    }

    #[test]
    fn grayscale_option() {
        let input = fixtures::photo_pdf();
        let o = CompressOptions {
            grayscale: true,
            ..opts(Preset::Balanced)
        };
        let (out, _) = compress(&input, &o).unwrap();
        let doc = pdf::load(&out, None).unwrap();
        let grays = doc
            .objects
            .values()
            .filter_map(|o| o.as_stream().ok())
            .filter(|s| s.dict.get(b"Subtype").and_then(Object::as_name).ok() == Some(b"Image"))
            .filter(|s| s.dict.get(b"SMask").is_ok())
            .all(|s| {
                s.dict.get(b"ColorSpace").and_then(Object::as_name).ok() == Some(b"DeviceGray")
            });
        assert!(grays);
    }

    #[test]
    fn dedupe_merges_identical_images() {
        let mut b = fixtures::Builder::new();
        let img = fixtures::photo(400, 300, 3);
        let a = b.jpeg_image(&img, 90);
        let c = b.jpeg_image(&img, 90);
        b.image_page(a, 200.0, 150.0);
        b.image_page(c, 200.0, 150.0);
        let input = b.finish();
        let (_, rep) = compress(&input, &opts(Preset::Lossless)).unwrap();
        assert!(rep.duplicates_merged >= 1);
    }

    #[test]
    fn dpi_usage_is_measured() {
        let input = fixtures::photo_pdf();
        let doc = pdf::load(&input, None).unwrap();
        let usage = image_usage(&doc);
        let mut sizes: Vec<_> = usage.values().map(|(w, _)| w.round() as i32).collect();
        sizes.sort();
        assert_eq!(sizes, vec![216, 360]);
    }

    #[test]
    fn flate_wrapped_jpeg_and_inverted_gray() {
        let mut b = fixtures::Builder::new();
        let img = fixtures::photo(1500, 1000, 4);
        let jpeg = fixtures::jpeg(&img, 95);
        let wrapped = b.doc.add_object(Stream::new(
            dictionary! {
                "Type" => "XObject", "Subtype" => "Image", "Width" => 1500, "Height" => 1000,
                "ColorSpace" => "DeviceRGB", "BitsPerComponent" => 8,
                "Filter" => vec!["FlateDecode".into(), "DCTDecode".into()],
            },
            miniz_oxide::deflate::compress_to_vec_zlib(&jpeg, 6),
        ));
        b.image_page(wrapped, 360.0, 240.0);
        let gray = image::DynamicImage::ImageRgb8(fixtures::photo(1200, 800, 2)).to_luma8();
        let inverted = b.doc.add_object(Stream::new(
            dictionary! {
                "Type" => "XObject", "Subtype" => "Image", "Width" => 1200, "Height" => 800,
                "ColorSpace" => "DeviceGray", "BitsPerComponent" => 8, "Decode" => vec![1.into(), 0.into()],
                "Filter" => "FlateDecode",
            },
            miniz_oxide::deflate::compress_to_vec_zlib(gray.as_raw(), 1),
        ));
        b.image_page(inverted, 288.0, 192.0);
        let input = b.finish();
        let (out, rep) = compress(&input, &opts(Preset::Strong)).unwrap();
        assert!(
            rep.images.iter().all(|i| i.action == "jpeg"),
            "{:?}",
            rep.images
        );
        let doc = pdf::load(&out, None).unwrap();
        // Inverted samples were baked in, so /Decode must be gone.
        assert!(
            doc.objects
                .values()
                .filter_map(|o| o.as_stream().ok())
                .all(|s| !s.dict.has(b"Decode"))
        );
        crate::testutil::qpdf_check(&out);
    }

    #[test]
    fn ascii85_layers_are_removed_losslessly() {
        let mut b = fixtures::Builder::new();
        let jpeg = fixtures::jpeg(&fixtures::photo(600, 400, 8), 90);
        // Encode as ASCII85 the way ReportLab does.
        let mut a85 = Vec::new();
        for chunk in jpeg.chunks(4) {
            let mut buf = [0u8; 4];
            buf[..chunk.len()].copy_from_slice(chunk);
            let mut v = u32::from_be_bytes(buf);
            let mut digits = [0u8; 5];
            for d in digits.iter_mut().rev() {
                *d = (v % 85) as u8 + b'!';
                v /= 85;
            }
            a85.extend_from_slice(&digits[..chunk.len() + 1]);
        }
        a85.extend_from_slice(b"~>");
        let id = b.doc.add_object(Stream::new(
            dictionary! {
                "Type" => "XObject", "Subtype" => "Image", "Width" => 600, "Height" => 400,
                "ColorSpace" => "DeviceRGB", "BitsPerComponent" => 8,
                "Filter" => vec!["ASCII85Decode".into(), "DCTDecode".into()],
            },
            a85,
        ));
        b.image_page(id, 600.0 * 72.0 / 150.0, 400.0 * 72.0 / 150.0);
        let input = b.finish();
        let (out, _) = compress(&input, &opts(Preset::Lossless)).unwrap();
        let doc = pdf::load(&out, None).unwrap();
        let img = doc
            .objects
            .values()
            .filter_map(|o| o.as_stream().ok())
            .find(|s| s.dict.has(b"Width"))
            .unwrap();
        assert_eq!(img.content, jpeg);
        assert_eq!(
            img.dict.get(b"Filter").unwrap().as_name().unwrap(),
            b"DCTDecode"
        );
        crate::testutil::qpdf_check(&out);
    }

    #[test]
    fn rejects_bad_options() {
        let input = fixtures::text_pdf(1);
        let o = CompressOptions {
            quality: Some(0),
            ..Default::default()
        };
        assert_eq!(compress(&input, &o).unwrap_err().code(), "invalid_option");
    }

    #[test]
    fn extreme_rebuild() {
        let input = fixtures::text_pdf(2);
        let img = fixtures::photo(100, 130, 1);
        let jpg = fixtures::jpeg(&img, 60);
        let pages = vec![
            (
                PageImage {
                    width: 100,
                    height: 130,
                    gray: false,
                },
                jpg.clone(),
            ),
            (
                PageImage {
                    width: 100,
                    height: 130,
                    gray: false,
                },
                jpg,
            ),
        ];
        let out = rebuild_from_page_images(&input, None, &pages).unwrap();
        assert_eq!(pdf::load(&out, None).unwrap().get_pages().len(), 2);
        crate::testutil::qpdf_check(&out);
    }
}
