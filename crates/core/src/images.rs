//! Image files: embedding into PDFs, images → PDF, and the image tools
//! (compress, resize, convert, strip metadata, crop, rotate).

use crate::error::{Error, Result};
use crate::pdf::{self, n};
use crate::raster::{self, Pixels};
use image::{DynamicImage, GenericImageView, ImageFormat};
use lopdf::{Dictionary, Document, Object, ObjectId, Stream, dictionary};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    Jpeg,
    Png,
    Webp,
    Gif,
    Bmp,
}

impl Format {
    pub fn detect(b: &[u8]) -> Option<Format> {
        match b {
            [0xFF, 0xD8, 0xFF, ..] => Some(Format::Jpeg),
            [0x89, b'P', b'N', b'G', ..] => Some(Format::Png),
            [
                b'R',
                b'I',
                b'F',
                b'F',
                _,
                _,
                _,
                _,
                b'W',
                b'E',
                b'B',
                b'P',
                ..,
            ] => Some(Format::Webp),
            [b'G', b'I', b'F', b'8', ..] => Some(Format::Gif),
            [b'B', b'M', ..] => Some(Format::Bmp),
            _ => None,
        }
    }
    fn image_format(self) -> ImageFormat {
        match self {
            Format::Jpeg => ImageFormat::Jpeg,
            Format::Png => ImageFormat::Png,
            Format::Webp => ImageFormat::WebP,
            Format::Gif => ImageFormat::Gif,
            Format::Bmp => ImageFormat::Bmp,
        }
    }
    pub fn extension(self) -> &'static str {
        match self {
            Format::Jpeg => "jpg",
            Format::Png => "png",
            Format::Webp => "webp",
            Format::Gif => "gif",
            Format::Bmp => "bmp",
        }
    }
}

const MAX_PIXELS: u64 = 200_000_000;

fn sniff(bytes: &[u8]) -> Result<Format> {
    Format::detect(bytes).ok_or_else(|| {
        Error::Image("unrecognized image format (JPEG, PNG, WebP, GIF or BMP)".into())
    })
}

/// Decode with EXIF orientation applied, refusing pixel bombs.
pub fn decode(bytes: &[u8]) -> Result<DynamicImage> {
    let fmt = sniff(bytes)?;
    let mut reader =
        image::ImageReader::with_format(std::io::Cursor::new(bytes), fmt.image_format());
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(1 << 30);
    reader.limits(limits);
    let (w, h) = reader.into_dimensions()?;
    if w as u64 * h as u64 > MAX_PIXELS {
        return Err(Error::Limit(format!("{w}x{h} image is too large")));
    }
    let img = image::load_from_memory_with_format(bytes, fmt.image_format())?;
    Ok(match fmt {
        Format::Jpeg => orient(img, exif_orientation(bytes)),
        _ => img,
    })
}

fn orient(img: DynamicImage, o: u8) -> DynamicImage {
    match o {
        2 => img.fliph(),
        3 => img.rotate180(),
        4 => img.flipv(),
        5 => img.rotate90().fliph(),
        6 => img.rotate90(),
        7 => img.rotate270().fliph(),
        8 => img.rotate270(),
        _ => img,
    }
}

/// EXIF orientation (1-8) of a JPEG, 1 when absent.
pub fn exif_orientation(jpeg: &[u8]) -> u8 {
    for (marker, seg) in jpeg_segments(jpeg) {
        if marker == 0xE1 && seg.starts_with(b"Exif\0\0") {
            return tiff_orientation(&seg[6..]).unwrap_or(1);
        }
    }
    1
}

fn tiff_orientation(t: &[u8]) -> Option<u8> {
    let le = match t.get(0..2)? {
        b"II" => true,
        b"MM" => false,
        _ => return None,
    };
    let u16_at = |o: usize| -> Option<u16> {
        let b = t.get(o..o + 2)?;
        Some(if le {
            u16::from_le_bytes([b[0], b[1]])
        } else {
            u16::from_be_bytes([b[0], b[1]])
        })
    };
    let u32_at = |o: usize| -> Option<u32> {
        let b = t.get(o..o + 4)?;
        Some(if le {
            u32::from_le_bytes([b[0], b[1], b[2], b[3]])
        } else {
            u32::from_be_bytes([b[0], b[1], b[2], b[3]])
        })
    };
    let ifd = u32_at(4)? as usize;
    let count = u16_at(ifd)? as usize;
    for i in 0..count.min(256) {
        let e = ifd + 2 + i * 12;
        if u16_at(e)? == 0x0112 {
            let v = u16_at(e + 8)?;
            return (1..=8).contains(&v).then_some(v as u8);
        }
    }
    None
}

/// (marker, payload) for each JPEG segment before the scan data.
fn jpeg_segments(b: &[u8]) -> Vec<(u8, &[u8])> {
    let mut out = vec![];
    let mut i = 2;
    while i + 4 <= b.len() && b[i] == 0xFF {
        let marker = b[i + 1];
        if marker == 0xD8 || (0xD0..=0xD7).contains(&marker) || marker == 0x01 || marker == 0xFF {
            i += if marker == 0xFF { 1 } else { 2 };
            continue;
        }
        let len = u16::from_be_bytes([b[i + 2], b[i + 3]]) as usize;
        if len < 2 || i + 2 + len > b.len() {
            break;
        }
        out.push((marker, &b[i + 4..i + 2 + len]));
        if marker == 0xDA {
            break;
        }
        i += 2 + len;
    }
    out
}

/// Matrix drawing an image XObject (unit square) into a `w`×`h` box at
/// (`x`, `y`), turned the way its EXIF orientation says it should be seen.
pub fn placement(orientation: u8, x: f32, y: f32, w: f32, h: f32) -> [f32; 6] {
    let m = match orientation {
        2 => [-w, 0.0, 0.0, h, w, 0.0],
        3 => [-w, 0.0, 0.0, -h, w, h],
        4 => [w, 0.0, 0.0, -h, 0.0, h],
        5 => [0.0, -h, -w, 0.0, w, h],
        6 => [0.0, -h, w, 0.0, 0.0, h],
        7 => [0.0, h, w, 0.0, 0.0, 0.0],
        8 => [0.0, h, -w, 0.0, w, 0.0],
        _ => [w, 0.0, 0.0, h, 0.0, 0.0],
    };
    [m[0], m[1], m[2], m[3], m[4] + x, m[5] + y]
}

pub struct Embedded {
    pub id: ObjectId,
    /// Size as displayed, after orientation.
    pub width: u32,
    pub height: u32,
    pub orientation: u8,
}

/// Add an image file as an XObject. Baseline gray/RGB JPEGs go in untouched;
/// everything else is decoded and stored losslessly, alpha as a soft mask.
pub fn embed(doc: &mut Document, bytes: &[u8]) -> Result<Embedded> {
    let fmt = sniff(bytes)?;
    if fmt == Format::Jpeg {
        let img = image::load_from_memory_with_format(bytes, ImageFormat::Jpeg)?;
        let (w, h) = img.dimensions();
        let space = match img.color() {
            image::ColorType::L8 | image::ColorType::L16 => Some("DeviceGray"),
            image::ColorType::Rgb8 | image::ColorType::Rgb16 => Some("DeviceRGB"),
            _ => None,
        };
        // Sniff CMYK from the frame header: image converts it to RGB on decode.
        let comps = jpeg_segments(bytes)
            .iter()
            .find(|(m, _)| matches!(m, 0xC0..=0xC2))
            .and_then(|(_, s)| s.get(5).copied());
        if let (Some(space), false) = (space, comps == Some(4)) {
            let id = doc.add_object(Stream::new(
                dictionary! {
                    "Type" => "XObject", "Subtype" => "Image", "Width" => w as i64, "Height" => h as i64,
                    "ColorSpace" => space, "BitsPerComponent" => 8, "Filter" => "DCTDecode",
                },
                bytes.to_vec(),
            ));
            let o = exif_orientation(bytes);
            let (dw, dh) = if o >= 5 { (h, w) } else { (w, h) };
            return Ok(Embedded {
                id,
                width: dw,
                height: dh,
                orientation: o,
            });
        }
    }
    let img = decode(bytes)?;
    let (w, h) = img.dimensions();
    let has_alpha = img.color().has_alpha() && img.to_rgba8().pixels().any(|p| p[3] != 255);
    let px = match img.color() {
        image::ColorType::L8
        | image::ColorType::L16
        | image::ColorType::La8
        | image::ColorType::La16 => Pixels::Gray(img.to_luma8()),
        _ => Pixels::Rgb(img.to_rgb8()).simplify(),
    };
    let mut dict = dictionary! { "Type" => "XObject", "Subtype" => "Image", "Width" => w as i64, "Height" => h as i64 };
    let data = flate_pixels(&px, &mut dict);
    if has_alpha {
        let alpha: Vec<u8> = img.to_rgba8().pixels().map(|p| p[3]).collect();
        let mut mdict = dictionary! { "Type" => "XObject", "Subtype" => "Image", "Width" => w as i64, "Height" => h as i64 };
        let mdata = flate_pixels(
            &Pixels::Gray(image::GrayImage::from_raw(w, h, alpha).expect("alpha size")),
            &mut mdict,
        );
        let mid = doc.add_object(Stream::new(mdict, mdata));
        dict.set("SMask", mid);
    }
    let id = doc.add_object(Stream::new(dict, data));
    Ok(Embedded {
        id,
        width: w,
        height: h,
        orientation: 1,
    })
}

/// Back-compat helper for stamps: the XObject and its displayed size.
pub fn embed_image(doc: &mut Document, bytes: &[u8]) -> Result<(ObjectId, u32, u32)> {
    let e = embed(doc, bytes)?;
    if e.orientation != 1 {
        // Stamps draw the unit square directly; bake orientation into pixels.
        let img = decode(bytes)?;
        let mut out = std::io::Cursor::new(Vec::new());
        img.write_to(&mut out, ImageFormat::Png)?;
        return embed_image(doc, &out.into_inner());
    }
    Ok((e.id, e.width, e.height))
}

fn flate_pixels(px: &Pixels, dict: &mut Dictionary) -> Vec<u8> {
    let comps = px.components();
    dict.set(
        "ColorSpace",
        if comps == 1 {
            "DeviceGray"
        } else {
            "DeviceRGB"
        },
    );
    dict.set("BitsPerComponent", 8);
    dict.set("Filter", "FlateDecode");
    dict.set(
        "DecodeParms",
        dictionary! { "Predictor" => 15, "Colors" => comps as i64, "BitsPerComponent" => 8, "Columns" => px.width() as i64 },
    );
    raster::deflate(&raster::png_predict(
        px.raw(),
        px.width() as usize * comps,
        comps,
    ))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Img2PdfOptions {
    /// "fit" (page = image size), "a4" or "letter".
    pub page_size: String,
    /// "auto" follows each image, or "portrait" / "landscape".
    pub orientation: String,
    /// Margin in points around the image on fixed-size pages.
    pub margin: f32,
    /// Pixels per inch used to size "fit" pages.
    pub dpi: f32,
}

impl Default for Img2PdfOptions {
    fn default() -> Self {
        Img2PdfOptions {
            page_size: "a4".into(),
            orientation: "auto".into(),
            margin: 24.0,
            dpi: 150.0,
        }
    }
}

pub fn img2pdf(images: &[Vec<u8>], o: &Img2PdfOptions) -> Result<Vec<u8>> {
    if images.is_empty() {
        return Err(Error::option("no images given"));
    }
    if !(10.0..=2400.0).contains(&o.dpi) {
        return Err(Error::option("dpi must be between 10 and 2400"));
    }
    let mut doc = Document::with_version("1.7");
    let pages_id = doc.new_object_id();
    let mut kids = vec![];
    for bytes in images {
        let e = embed(&mut doc, bytes)?;
        let (iw, ih) = (
            e.width as f32 * 72.0 / o.dpi,
            e.height as f32 * 72.0 / o.dpi,
        );
        let (pw, ph, x, y, w, h) = match o.page_size.as_str() {
            "fit" => (iw, ih, 0.0, 0.0, iw, ih),
            size => {
                let (mut pw, mut ph) = match size {
                    "a4" => (595.28, 841.89),
                    "letter" => (612.0, 792.0),
                    other => return Err(Error::option(format!("unknown page size '{other}'"))),
                };
                let landscape = match o.orientation.as_str() {
                    "auto" => e.width > e.height,
                    "landscape" => true,
                    "portrait" => false,
                    other => return Err(Error::option(format!("unknown orientation '{other}'"))),
                };
                if landscape {
                    std::mem::swap(&mut pw, &mut ph);
                }
                let (bw, bh) = (pw - 2.0 * o.margin, ph - 2.0 * o.margin);
                if bw <= 1.0 || bh <= 1.0 {
                    return Err(Error::option("margin leaves no room for the image"));
                }
                let s = (bw / e.width as f32).min(bh / e.height as f32);
                let (w, h) = (e.width as f32 * s, e.height as f32 * s);
                (pw, ph, (pw - w) / 2.0, (ph - h) / 2.0, w, h)
            }
        };
        let m = placement(e.orientation, x, y, w, h);
        let ops = format!(
            "q {} {} {} {} {} {} cm /Im0 Do Q",
            n(m[0]),
            n(m[1]),
            n(m[2]),
            n(m[3]),
            n(m[4]),
            n(m[5])
        );
        let content = doc.add_object(Stream::new(Dictionary::new(), ops.into_bytes()));
        let page = doc.add_object(dictionary! {
            "Type" => "Page", "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), pw.into(), ph.into()],
            "Contents" => content, "Resources" => dictionary! { "XObject" => dictionary! { "Im0" => e.id } },
        });
        kids.push(Object::Reference(page));
    }
    let count = kids.len() as i64;
    doc.objects.insert(
        pages_id,
        dictionary! { "Type" => "Pages", "Kids" => kids, "Count" => count }.into(),
    );
    let root = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
    doc.trailer.set("Root", root);
    pdf::save(&mut doc)
}

// ---------------------------------------------------------------- image tools

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ImageOptions {
    /// Output format; the input's format when absent (GIF and BMP become PNG).
    pub format: Option<Format>,
    /// JPEG quality, 1-100.
    pub quality: Option<u8>,
    /// Reduce a PNG to at most this many colors (lossy, 2-256).
    pub colors: Option<u16>,
    /// Resize: fit inside these bounds, keeping the aspect ratio.
    pub max_width: Option<u32>,
    pub max_height: Option<u32>,
    /// Resize: exact size; one side alone keeps the aspect ratio.
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub percent: Option<f32>,
    /// Crop rectangle [x, y, width, height] in pixels, after orientation.
    pub crop: Option<[u32; 4]>,
    /// Clockwise: 90, 180 or 270.
    pub rotate: Option<u32>,
    /// "horizontal" or "vertical".
    pub flip: Option<String>,
}

#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ImageReport {
    pub format: Option<Format>,
    pub width: u32,
    pub height: u32,
    pub bytes_before: usize,
    pub bytes_after: usize,
    pub kept_original: bool,
}

/// Re-encode pixels in a format. JPEG drops alpha onto white.
pub fn encode(
    img: &DynamicImage,
    fmt: Format,
    quality: u8,
    colors: Option<u16>,
) -> Result<Vec<u8>> {
    match fmt {
        Format::Jpeg => {
            let px = if img.color().has_color() {
                Pixels::Rgb(flatten_alpha(img))
            } else {
                Pixels::Gray(DynamicImage::ImageRgb8(flatten_alpha(img)).to_luma8())
            };
            raster::encode_jpeg(&px, quality)
        }
        Format::Png => Ok(png_encode(img, colors)),
        Format::Webp => {
            let mut out = std::io::Cursor::new(Vec::new());
            let rgba = DynamicImage::ImageRgba8(img.to_rgba8());
            let enc = image::codecs::webp::WebPEncoder::new_lossless(&mut out);
            rgba.write_with_encoder(enc)?;
            Ok(out.into_inner())
        }
        other => Err(Error::option(format!(
            "cannot write {other:?}; use jpeg, png or webp"
        ))),
    }
}

fn flatten_alpha(img: &DynamicImage) -> image::RgbImage {
    if !img.color().has_alpha() {
        return img.to_rgb8();
    }
    let rgba = img.to_rgba8();
    image::RgbImage::from_fn(rgba.width(), rgba.height(), |x, y| {
        let p = rgba.get_pixel(x, y);
        let a = p[3] as u32;
        let mix = |c: u8| ((c as u32 * a + 255 * (255 - a)) / 255) as u8;
        image::Rgb([mix(p[0]), mix(p[1]), mix(p[2])])
    })
}

/// PNG writer with per-row filter selection and maximum deflate; indexed
/// when the image has few colors or when `colors` asks for quantization.
pub fn png_encode(img: &DynamicImage, colors: Option<u16>) -> Vec<u8> {
    let (w, h) = img.dimensions();
    let rgba = img.to_rgba8();
    let opaque = rgba.pixels().all(|p| p[3] == 255);
    let gray = !img.color().has_color();

    // Exact palette first, then optional NeuQuant quantization.
    let mut palette: Vec<[u8; 4]> = vec![];
    let mut map = std::collections::HashMap::new();
    let mut indices: Option<Vec<u8>> = Some(Vec::with_capacity((w * h) as usize));
    for p in rgba.pixels() {
        let key = p.0;
        let idx = match map.get(&key) {
            Some(i) => *i,
            None if palette.len() < 256 => {
                let i = palette.len() as u8;
                palette.push(key);
                map.insert(key, i);
                i
            }
            None => {
                indices = None;
                break;
            }
        };
        if let Some(v) = indices.as_mut() {
            v.push(idx);
        }
    }
    if let Some(target) = colors.map(|c| c.clamp(2, 256) as usize)
        && (indices.is_none() || palette.len() > target)
    {
        let nq = color_quant::NeuQuant::new(10, target, rgba.as_raw());
        palette = nq.color_map_rgba().as_chunks::<4>().0.to_vec();
        indices = Some(rgba.pixels().map(|p| nq.index_of(&p.0) as u8).collect());
    }
    if let Some(idx) = indices.filter(|_| !(gray && palette.len() > 16)) {
        let bits: u8 = match palette.len() {
            0..=2 => 1,
            3..=4 => 2,
            5..=16 => 4,
            _ => 8,
        };
        let row = (w as usize * bits as usize).div_ceil(8);
        let mut data = vec![0u8; row * h as usize];
        for (y, r) in idx.chunks_exact(w as usize).enumerate() {
            for (x, &i) in r.iter().enumerate() {
                let bit = x * bits as usize;
                data[y * row + bit / 8] |= i << (8 - bits as usize - bit % 8);
            }
        }
        let plte: Vec<u8> = palette.iter().flat_map(|c| [c[0], c[1], c[2]]).collect();
        let trns: Vec<u8> = if opaque {
            vec![]
        } else {
            palette.iter().map(|c| c[3]).collect()
        };
        return png_chunks(
            w,
            h,
            3,
            bits,
            &raster::png_predict(&data, row, 1),
            Some(&plte),
            Some(&trns),
        );
    }
    let (ctype, comps, raw): (u8, usize, Vec<u8>) = match (gray, opaque) {
        (true, true) => (0, 1, img.to_luma8().into_raw()),
        (true, false) => (4, 2, img.to_luma_alpha8().into_raw()),
        (false, true) => (2, 3, img.to_rgb8().into_raw()),
        (false, false) => (6, 4, rgba.into_raw()),
    };
    png_chunks(
        w,
        h,
        ctype,
        8,
        &raster::png_predict(&raw, w as usize * comps, comps),
        None,
        None,
    )
}

fn png_chunks(
    w: u32,
    h: u32,
    ctype: u8,
    bits: u8,
    filtered: &[u8],
    plte: Option<&[u8]>,
    trns: Option<&[u8]>,
) -> Vec<u8> {
    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut chunk = |kind: &[u8; 4], data: &[u8]| {
        out.extend((data.len() as u32).to_be_bytes());
        let mut crc = crc32fast::Hasher::new();
        crc.update(kind);
        crc.update(data);
        out.extend(kind);
        out.extend(data);
        out.extend(crc.finalize().to_be_bytes());
    };
    let mut ihdr = vec![];
    ihdr.extend(w.to_be_bytes());
    ihdr.extend(h.to_be_bytes());
    ihdr.extend([bits, ctype, 0, 0, 0]);
    chunk(b"IHDR", &ihdr);
    if let Some(p) = plte {
        chunk(b"PLTE", p);
    }
    if let Some(t) = trns.filter(|t| !t.is_empty()) {
        chunk(b"tRNS", t);
    }
    chunk(b"IDAT", &raster::deflate(filtered));
    chunk(b"IEND", &[]);
    out
}

/// Apply geometry, then encode. `compress` only replaces the input when smaller.
pub fn transform(
    input: &[u8],
    o: &ImageOptions,
    compress_only: bool,
) -> Result<(Vec<u8>, ImageReport)> {
    let src_fmt = sniff(input)?;
    let mut img = decode(input)?;
    if let Some([x, y, w, h]) = o.crop {
        let (iw, ih) = img.dimensions();
        if w == 0 || h == 0 || x.saturating_add(w) > iw || y.saturating_add(h) > ih {
            return Err(Error::option(format!(
                "crop {x},{y} {w}x{h} falls outside the {iw}x{ih} image"
            )));
        }
        img = img.crop_imm(x, y, w, h);
    }
    match o.rotate {
        None | Some(0) => {}
        Some(90) => img = img.rotate90(),
        Some(180) => img = img.rotate180(),
        Some(270) => img = img.rotate270(),
        Some(r) => {
            return Err(Error::option(format!(
                "rotation {r} must be 90, 180 or 270"
            )));
        }
    }
    match o.flip.as_deref() {
        None => {}
        Some("horizontal") => img = img.fliph(),
        Some("vertical") => img = img.flipv(),
        Some(f) => {
            return Err(Error::option(format!(
                "flip '{f}' must be horizontal or vertical"
            )));
        }
    }
    let (iw, ih) = img.dimensions();
    let target = if let Some(p) = o.percent {
        if !(1.0..=1000.0).contains(&p) {
            return Err(Error::option("percent must be between 1 and 1000"));
        }
        Some((
            (iw as f32 * p / 100.0).round() as u32,
            (ih as f32 * p / 100.0).round() as u32,
        ))
    } else if o.width.is_some() || o.height.is_some() {
        Some(match (o.width, o.height) {
            (Some(w), Some(h)) => (w, h),
            (Some(w), None) => (w, (ih as f32 * w as f32 / iw as f32).round() as u32),
            (None, Some(h)) => ((iw as f32 * h as f32 / ih as f32).round() as u32, h),
            _ => unreachable!(),
        })
    } else if o.max_width.is_some() || o.max_height.is_some() {
        let s = (o.max_width.unwrap_or(u32::MAX) as f32 / iw as f32)
            .min(o.max_height.unwrap_or(u32::MAX) as f32 / ih as f32);
        (s < 1.0).then(|| {
            (
                ((iw as f32 * s).round() as u32),
                ((ih as f32 * s).round() as u32),
            )
        })
    } else {
        None
    };
    if let Some((w, h)) = target {
        if w == 0 || h == 0 || w as u64 * h as u64 > MAX_PIXELS {
            return Err(Error::option(format!("target size {w}x{h} is invalid")));
        }
        img = img.resize_exact(w, h, image::imageops::FilterType::Lanczos3);
    }
    let fmt = o.format.unwrap_or(match src_fmt {
        Format::Gif | Format::Bmp => Format::Png,
        f => f,
    });
    let quality = o.quality.unwrap_or(if compress_only { 75 } else { 90 });
    if !(1..=100).contains(&quality) {
        return Err(Error::option("quality must be between 1 and 100"));
    }
    let out = encode(&img, fmt, quality, o.colors)?;
    let mut rep = ImageReport {
        format: Some(fmt),
        width: img.width(),
        height: img.height(),
        bytes_before: input.len(),
        bytes_after: out.len(),
        kept_original: false,
    };
    if compress_only && fmt == src_fmt && out.len() >= input.len() {
        rep.bytes_after = input.len();
        rep.kept_original = true;
        return Ok((input.to_vec(), rep));
    }
    Ok((out, rep))
}

/// Remove EXIF, XMP, IPTC, comments and text chunks without touching pixels.
/// A JPEG whose EXIF rotates it is re-encoded upright, since dropping the tag
/// would otherwise turn it sideways.
pub fn strip(input: &[u8]) -> Result<(Vec<u8>, ImageReport)> {
    let fmt = sniff(input)?;
    let out = match fmt {
        Format::Jpeg if exif_orientation(input) != 1 => {
            encode(&decode(input)?, Format::Jpeg, 92, None)?
        }
        Format::Jpeg => strip_jpeg(input),
        Format::Png => strip_png(input)?,
        Format::Webp => strip_webp(input)?,
        Format::Gif | Format::Bmp => input.to_vec(),
    };
    let (w, h) = image::ImageReader::with_format(std::io::Cursor::new(&out), fmt.image_format())
        .into_dimensions()?;
    Ok((
        out.clone(),
        ImageReport {
            format: Some(fmt),
            width: w,
            height: h,
            bytes_before: input.len(),
            bytes_after: out.len(),
            kept_original: false,
        },
    ))
}

fn strip_jpeg(b: &[u8]) -> Vec<u8> {
    let mut out = vec![0xFF, 0xD8];
    let mut i = 2;
    while i + 4 <= b.len() && b[i] == 0xFF {
        let marker = b[i + 1];
        if marker == 0xFF {
            i += 1;
            continue;
        }
        let len = u16::from_be_bytes([b[i + 2], b[i + 3]]) as usize;
        if marker == 0xDA || i + 2 + len > b.len() {
            out.extend_from_slice(&b[i..]);
            return out;
        }
        let seg = &b[i..i + 2 + len];
        let payload = &seg[4..];
        // Keep JFIF (APP0), ICC profiles (APP2 ICC_PROFILE) and Adobe color info (APP14).
        let keep = match marker {
            0xE0 => payload.starts_with(b"JFIF"),
            0xE2 => payload.starts_with(b"ICC_PROFILE"),
            0xEE => payload.starts_with(b"Adobe"),
            0xE1 | 0xE3..=0xED | 0xEF | 0xFE => false,
            _ => true,
        };
        if keep {
            out.extend_from_slice(seg);
        }
        i += 2 + len;
    }
    out.extend_from_slice(&b[i.min(b.len())..]);
    out
}

fn strip_png(b: &[u8]) -> Result<Vec<u8>> {
    let mut out = b[..8].to_vec();
    let mut i = 8;
    while i + 12 <= b.len() {
        let len = u32::from_be_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]) as usize;
        let end = i + 12 + len;
        if end > b.len() {
            return Err(Error::Image("truncated PNG chunk".into()));
        }
        let kind = &b[i + 4..i + 8];
        if !matches!(kind, b"tEXt" | b"zTXt" | b"iTXt" | b"eXIf" | b"tIME") {
            out.extend_from_slice(&b[i..end]);
        }
        i = end;
    }
    Ok(out)
}

fn strip_webp(b: &[u8]) -> Result<Vec<u8>> {
    let mut chunks = vec![];
    let mut i = 12;
    while i + 8 <= b.len() {
        let len = u32::from_le_bytes([b[i + 4], b[i + 5], b[i + 6], b[i + 7]]) as usize;
        let end = i + 8 + len + (len & 1);
        if end > b.len() {
            return Err(Error::Image("truncated WebP chunk".into()));
        }
        let kind = &b[i..i + 4];
        if kind != b"EXIF" && kind != b"XMP " {
            let mut c = b[i..end].to_vec();
            if kind == b"VP8X" && c.len() > 8 {
                // Clear the EXIF (bit 3) and XMP (bit 2) flags.
                c[8] &= !0b0000_1100;
            }
            chunks.push(c);
        }
        i = end;
    }
    let body: Vec<u8> = chunks.concat();
    let mut out = b"RIFF".to_vec();
    out.extend(((body.len() + 4) as u32).to_le_bytes());
    out.extend(b"WEBP");
    out.extend(body);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures;

    fn jpeg_with_exif(orientation: u16) -> Vec<u8> {
        let base = fixtures::jpeg(&fixtures::photo(40, 20, 1), 90);
        // TIFF, big endian, one IFD entry: orientation, plus a fake GPS-ish tag.
        let mut tiff = b"MM\0\x2a\0\0\0\x08\0\x02".to_vec();
        tiff.extend([0x01, 0x12, 0, 3, 0, 0, 0, 1]);
        tiff.extend(orientation.to_be_bytes());
        tiff.extend([0, 0]);
        tiff.extend([0x88, 0x25, 0, 4, 0, 0, 0, 1, 0, 0, 0, 0]);
        tiff.extend([0, 0, 0, 0]);
        let mut app1 = b"Exif\0\0".to_vec();
        app1.extend(tiff);
        let mut out = vec![0xFF, 0xD8, 0xFF, 0xE1];
        out.extend(((app1.len() + 2) as u16).to_be_bytes());
        out.extend(app1);
        out.extend(&base[2..]);
        out
    }

    #[test]
    fn orientation_is_read_and_applied() {
        let j = jpeg_with_exif(6);
        assert_eq!(exif_orientation(&j), 6);
        assert_eq!(decode(&j).unwrap().dimensions(), (20, 40));
    }

    #[test]
    fn strip_removes_exif() {
        let j = jpeg_with_exif(1);
        let (out, rep) = strip(&j).unwrap();
        assert!(out.len() < j.len());
        assert!(!out.windows(4).any(|w| w == b"Exif"));
        assert_eq!((rep.width, rep.height), (40, 20));
        // Rotated JPEGs come out upright.
        let (out, rep) = strip(&jpeg_with_exif(8)).unwrap();
        assert_eq!(exif_orientation(&out), 1);
        assert_eq!((rep.width, rep.height), (20, 40));
    }

    #[test]
    fn strip_png_text() {
        let mut p = fixtures::png(&DynamicImage::ImageRgb8(fixtures::line_art(30, 30)));
        // Insert a tEXt chunk after IHDR.
        let mut chunk = vec![0, 0, 0, 7];
        chunk.extend(b"tEXtAuthor\0");
        chunk.truncate(4 + 4 + 7);
        let mut crc = crc32fast::Hasher::new();
        crc.update(&chunk[4..]);
        chunk.extend(crc.finalize().to_be_bytes());
        let at = 8 + 25;
        p.splice(at..at, chunk);
        assert!(image::load_from_memory(&p).is_ok());
        let (out, _) = strip(&p).unwrap();
        assert!(!out.windows(4).any(|w| w == b"tEXt"));
        assert!(image::load_from_memory(&out).is_ok());
    }

    #[test]
    fn convert_and_resize() {
        let p = fixtures::png(&DynamicImage::ImageRgb8(fixtures::photo(200, 100, 2)));
        let o = ImageOptions {
            format: Some(Format::Webp),
            max_width: Some(50),
            ..Default::default()
        };
        let (out, rep) = transform(&p, &o, false).unwrap();
        assert_eq!(Format::detect(&out), Some(Format::Webp));
        assert_eq!((rep.width, rep.height), (50, 25));
        let o = ImageOptions {
            format: Some(Format::Jpeg),
            width: Some(80),
            ..Default::default()
        };
        let (out, rep) = transform(&p, &o, false).unwrap();
        assert_eq!(Format::detect(&out), Some(Format::Jpeg));
        assert_eq!((rep.width, rep.height), (80, 40));
    }

    #[test]
    fn crop_rotate_flip() {
        let p = fixtures::png(&DynamicImage::ImageRgb8(fixtures::photo(100, 60, 2)));
        let o = ImageOptions {
            crop: Some([10, 10, 50, 30]),
            rotate: Some(90),
            flip: Some("vertical".into()),
            ..Default::default()
        };
        let (_, rep) = transform(&p, &o, false).unwrap();
        assert_eq!((rep.width, rep.height), (30, 50));
        let bad = ImageOptions {
            crop: Some([90, 0, 50, 30]),
            ..Default::default()
        };
        assert!(transform(&p, &bad, false).is_err());
    }

    #[test]
    fn compress_jpeg_and_png() {
        let j = fixtures::jpeg(&fixtures::photo(300, 200, 4), 98);
        let (out, rep) = transform(
            &j,
            &ImageOptions {
                quality: Some(60),
                ..Default::default()
            },
            true,
        )
        .unwrap();
        assert!(out.len() < j.len() && !rep.kept_original);
        let p = fixtures::png(&DynamicImage::ImageRgb8(fixtures::photo(120, 80, 4)));
        let (out, _) = transform(
            &p,
            &ImageOptions {
                colors: Some(64),
                ..Default::default()
            },
            true,
        )
        .unwrap();
        assert!(out.len() < p.len());
        let decoded = image::load_from_memory(&out).unwrap();
        assert_eq!(decoded.dimensions(), (120, 80));
    }

    #[test]
    fn png_writer_is_lossless() {
        let src = DynamicImage::ImageRgba8(fixtures::with_alpha(&fixtures::photo(64, 48, 9)));
        let back = image::load_from_memory(&png_encode(&src, None)).unwrap();
        assert_eq!(back.to_rgba8(), src.to_rgba8());
        let flat = DynamicImage::ImageRgb8(fixtures::line_art(64, 48));
        let back = image::load_from_memory(&png_encode(&flat, None)).unwrap();
        assert_eq!(back.to_rgb8(), flat.to_rgb8());
    }

    #[test]
    fn images_to_pdf() {
        let j = jpeg_with_exif(6);
        let p = fixtures::png(&DynamicImage::ImageRgba8(fixtures::with_alpha(
            &fixtures::photo(50, 50, 1),
        )));
        let out = img2pdf(&[j.clone(), p], &Img2PdfOptions::default()).unwrap();
        let doc = pdf::load(&out, None).unwrap();
        assert_eq!(doc.get_pages().len(), 2);
        // The JPEG is embedded byte for byte.
        assert!(
            doc.objects
                .values()
                .filter_map(|o| o.as_stream().ok())
                .any(|s| s.content == j)
        );
        crate::testutil::qpdf_check(&out);
        let fit = img2pdf(
            &[fixtures::jpeg(&fixtures::photo(300, 150, 1), 80)],
            &Img2PdfOptions {
                page_size: "fit".into(),
                dpi: 150.0,
                ..Default::default()
            },
        )
        .unwrap();
        let doc = pdf::load(&fit, None).unwrap();
        assert_eq!(
            pdf::page_box(&doc, pdf::page_ids(&doc)[0]),
            [0.0, 0.0, 144.0, 72.0]
        );
        assert!(img2pdf(&[b"nope".to_vec()], &Img2PdfOptions::default()).is_err());
    }
}
