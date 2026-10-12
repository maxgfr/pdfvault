//! pdfvault-render: everything that needs to draw a page. Rasterizing
//! (PDF → images, the `extreme` preset, OCR input), positioned text for
//! Markdown, and page-by-page visual comparison. Built on hayro.

use hayro::hayro_interpret::font::{Glyph, GlyphRun};
use hayro::hayro_interpret::util::TransformExt;
use hayro::hayro_interpret::{
    BlendMode, ClipPath, Context, Device, DrawMode, DrawProps, Image, ImageDrawProps,
    InterpreterCache, InterpreterSettings, SoftMask, interpret_page,
};
use hayro::hayro_syntax::{LoadPdfError, Pdf};
use hayro::kurbo::{Affine, BezPath, Point, Rect};
use hayro::vello_cpu::color::palette::css::WHITE;
use hayro::{PixmapSettings, RenderCache, RenderSettings, render};
use image::{DynamicImage, RgbImage, RgbaImage};
use pdfvault_core::compress::{self, PageImage};
use pdfvault_core::dispatch::{OutFile, Output};
use pdfvault_core::images::{self, Format};
use pdfvault_core::markdown::{PageText, Span};
use pdfvault_core::raster::{self, Pixels};
use pdfvault_core::{Error, Result, pdf};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// Largest side of a rendered page, in pixels: keeps memory bounded.
const MAX_SIDE: f32 = 12_000.0;

pub fn open(bytes: &[u8], password: Option<&str>) -> Result<Pdf> {
    Pdf::new_with_password(bytes.to_vec(), password.unwrap_or("")).map_err(|e| match e {
        LoadPdfError::Decryption(_) if password.is_some_and(|p| !p.is_empty()) => {
            Error::WrongPassword
        }
        LoadPdfError::Decryption(_) => Error::PasswordRequired,
        LoadPdfError::Invalid => Error::InvalidPdf("the renderer could not read this file".into()),
    })
}

/// Render one page at `dpi` on white; returns straight RGBA.
pub fn render_page(pdf: &Pdf, index: usize, dpi: f32) -> Result<RgbaImage> {
    let pages = pdf.pages();
    let page = pages
        .get(index)
        .ok_or_else(|| Error::option(format!("page {} does not exist", index + 1)))?;
    let (w, h) = page.render_dimensions();
    let mut scale = dpi / 72.0;
    if w.max(h) * scale > MAX_SIDE {
        scale = MAX_SIDE / w.max(h);
    }
    let cache = RenderCache::new();
    let pix = render(
        page,
        &cache,
        &InterpreterSettings::default(),
        &RenderSettings::default(),
        &PixmapSettings {
            x_scale: scale,
            y_scale: scale,
            bg_color: WHITE,
        },
    );
    let (pw, ph) = (pix.width() as u32, pix.height() as u32);
    // On an opaque white background premultiplied and straight alpha agree.
    RgbaImage::from_raw(pw, ph, pix.data_as_u8_slice().to_vec())
        .ok_or_else(|| Error::Image("render buffer size".into()))
}

fn to_rgb(img: &RgbaImage) -> RgbImage {
    DynamicImage::ImageRgba8(img.clone()).to_rgb8()
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RenderOptions {
    pub dpi: f32,
    /// png, jpeg or webp; "rgba" returns raw pixels (for OCR).
    pub format: String,
    pub quality: u8,
    pub pages: Option<String>,
    pub password: Option<String>,
    pub grayscale: bool,
}

impl Default for RenderOptions {
    fn default() -> Self {
        RenderOptions {
            dpi: 150.0,
            format: "png".into(),
            quality: 85,
            pages: None,
            password: None,
            grayscale: false,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct RenderedPage {
    pub page: usize,
    pub width: u32,
    pub height: u32,
}

/// Rasterize the selected pages. Files come back in page order.
pub fn render_pages(bytes: &[u8], o: &RenderOptions) -> Result<(Vec<OutFile>, Vec<RenderedPage>)> {
    if !(10.0..=1200.0).contains(&o.dpi) {
        return Err(Error::option("dpi must be between 10 and 1200"));
    }
    let doc = open(bytes, o.password.as_deref())?;
    let count = doc.pages().len();
    let selection = pdf::parse_ranges(o.pages.as_deref().unwrap_or(""), count)?;
    let mut files = vec![];
    let mut meta = vec![];
    for p in selection {
        let mut img = render_page(&doc, p, o.dpi)?;
        if o.grayscale {
            img = DynamicImage::ImageLuma8(DynamicImage::ImageRgba8(img).to_luma8()).to_rgba8();
        }
        let (w, h) = img.dimensions();
        let (bytes, ext) = match o.format.as_str() {
            "rgba" => (img.into_raw(), "rgba"),
            "png" => (
                images::png_encode(&DynamicImage::ImageRgb8(to_rgb(&img)), None),
                "png",
            ),
            "jpeg" | "jpg" => {
                let px = if o.grayscale {
                    Pixels::Gray(DynamicImage::ImageRgba8(img).to_luma8())
                } else {
                    Pixels::Rgb(to_rgb(&img))
                };
                (raster::encode_jpeg(&px, o.quality)?, "jpg")
            }
            "webp" => (
                images::encode(
                    &DynamicImage::ImageRgb8(to_rgb(&img)),
                    Format::Webp,
                    o.quality,
                    None,
                )?,
                "webp",
            ),
            other => {
                return Err(Error::option(format!(
                    "unknown image format '{other}' (png, jpeg, webp)"
                )));
            }
        };
        files.push(OutFile {
            suffix: format!("page-{}", p + 1),
            ext: ext.into(),
            bytes,
        });
        meta.push(RenderedPage {
            page: p,
            width: w,
            height: h,
        });
    }
    Ok((files, meta))
}

/// The `extreme` preset: every page becomes one JPEG. Text turns into pixels.
pub fn extreme(
    bytes: &[u8],
    o: &compress::CompressOptions,
) -> Result<(Vec<u8>, compress::CompressReport)> {
    let (dpi_default, q_default) = compress::Preset::Extreme.raster_settings();
    let dpi = o.dpi.unwrap_or(dpi_default);
    let quality = o.quality.unwrap_or(q_default);
    let doc = open(bytes, o.password.as_deref())?;
    let mut pages = vec![];
    for p in 0..doc.pages().len() {
        let img = render_page(&doc, p, dpi)?;
        let px = if o.grayscale {
            Pixels::Gray(DynamicImage::ImageRgba8(img).to_luma8())
        } else {
            Pixels::Rgb(to_rgb(&img)).simplify()
        };
        let gray = px.components() == 1;
        let meta = PageImage {
            width: px.width(),
            height: px.height(),
            gray,
        };
        pages.push((meta, raster::encode_jpeg(&px, quality)?));
    }
    let out = compress::rebuild_from_page_images(bytes, o.password.as_deref(), &pages)?;
    let mut report = compress::CompressReport {
        preset: Some(compress::Preset::Extreme),
        pages: pages.len(),
        ..Default::default()
    };
    report.removed = vec!["text and vector content rasterized".into()];
    if out.len() >= bytes.len() && o.password.is_none() {
        report.kept_original = true;
        report.finish(bytes.len(), bytes.len());
        return Ok((bytes.to_vec(), report));
    }
    report.finish(bytes.len(), out.len());
    Ok((out, report))
}

// ---------------------------------------------------------------- text

/// Collects glyphs with their position and size in page points (top-left origin).
struct TextCollector {
    spans: Vec<Span>,
}

impl TextCollector {
    fn push_glyph(&mut self, text: String, origin: Point, end_x: f64, size: f64) {
        let (x, y, size) = (origin.x as f32, origin.y as f32, size as f32);
        if text.chars().all(char::is_control) || size <= 0.01 {
            return;
        }
        if let Some(last) = self.spans.last_mut() {
            let same_line =
                (last.y - y).abs() < size * 0.4 && (last.size - size).abs() < size * 0.2;
            let expected = last.x + last.w;
            let gap = x - expected;
            if same_line && gap > -size * 0.5 && gap < size * 0.8 {
                if gap > size * 0.15 && !last.text.ends_with(' ') && !text.starts_with(' ') {
                    last.text.push(' ');
                }
                last.text.push_str(&text);
                last.w = (end_x as f32 - last.x).max(last.w);
                return;
            }
        }
        self.spans.push(Span {
            text,
            x,
            y,
            w: (end_x as f32 - x).max(0.0),
            size,
            bold: false,
        });
    }
}

impl<'a> Device<'a> for TextCollector {
    fn draw_path(&mut self, _: &BezPath, _: DrawProps<'a>, _: &DrawMode) {}
    fn push_clip_path(&mut self, _: &ClipPath) {}
    fn push_transparency_group(&mut self, _: f32, _: Option<SoftMask<'a>>, _: BlendMode) {}
    fn draw_glyph_run(&mut self, run: &GlyphRun<'_, 'a>, props: DrawProps<'a>, _: &DrawMode) {
        for g in run.glyphs() {
            let Some(u) = g.as_unicode() else { continue };
            let text = match u {
                hayro::hayro_interpret::hayro_cmap::BfString::Char(c) => c.to_string(),
                hayro::hayro_interpret::hayro_cmap::BfString::String(s) => s,
            };
            let t: Affine = props.transform * g.transform();
            let origin = t * Point::new(0.0, 0.0);
            let up = t * Point::new(0.0, 1000.0);
            let size = (up - origin).hypot();
            let advance = match &**g {
                Glyph::Outline(o) => o.advance_width().unwrap_or(500.0) as f64,
                Glyph::Type3(_) => 500.0,
            };
            let end = t * Point::new(advance, 0.0);
            // The baseline sits below the glyph's visual top; report the top.
            let top = Point::new(origin.x, origin.y - size * 0.8);
            self.push_glyph(text, top, end.x, size);
        }
    }
    fn draw_image(&mut self, _: Image<'a, '_>, _: ImageDrawProps<'a>) {}
    fn pop_clip(&mut self) {}
    fn pop_transparency_group(&mut self) {}
}

/// Positioned text of each selected page, in points from the top-left of
/// the page as displayed.
pub fn extract_text(
    bytes: &[u8],
    pages: Option<&str>,
    password: Option<&str>,
) -> Result<Vec<(usize, PageText)>> {
    let doc = open(bytes, password)?;
    let count = doc.pages().len();
    let cache = InterpreterCache::new();
    let mut out = vec![];
    for p in pdf::parse_ranges(pages.unwrap_or(""), count)? {
        let page = &doc.pages()[p];
        let (w, h) = page.render_dimensions();
        let mut ctx = Context::new(
            page.initial_transform(true).to_kurbo(),
            Rect::new(0.0, 0.0, w as f64, h as f64),
            &cache,
            page.xref(),
            InterpreterSettings {
                render_annotations: false,
                ..Default::default()
            },
        );
        let mut dev = TextCollector { spans: vec![] };
        interpret_page(page, &mut ctx, &mut dev);
        for s in &mut dev.spans {
            s.text = s.text.trim_end().to_string();
        }
        dev.spans.retain(|s| !s.text.trim().is_empty());
        out.push((p, PageText { spans: dev.spans }));
    }
    Ok(out)
}

// ---------------------------------------------------------------- compare

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PageDiff {
    pub page: usize,
    /// Share of pixels that differ noticeably, 0..1.
    pub changed: f64,
    pub missing_in: Option<String>,
}

/// Render both documents and mark differing pixels in red over a faded copy
/// of the second one.
pub fn compare(
    a: &[u8],
    b: &[u8],
    dpi: f32,
    password: Option<&str>,
) -> Result<(Vec<OutFile>, Vec<PageDiff>)> {
    let (da, db) = (open(a, password)?, open(b, password)?);
    let (na, nb) = (da.pages().len(), db.pages().len());
    let mut files = vec![];
    let mut diffs = vec![];
    for p in 0..na.max(nb) {
        if p >= na || p >= nb {
            diffs.push(PageDiff {
                page: p,
                changed: 1.0,
                missing_in: Some(if p >= na { "first" } else { "second" }.into()),
            });
            continue;
        }
        let ia = render_page(&da, p, dpi)?;
        let mut ib = render_page(&db, p, dpi)?;
        if ib.dimensions() != ia.dimensions() {
            ib = image::imageops::resize(
                &ib,
                ia.width(),
                ia.height(),
                image::imageops::FilterType::Triangle,
            );
        }
        let mut changed = 0u64;
        let out = RgbImage::from_fn(ia.width(), ia.height(), |x, y| {
            let (pa, pb) = (ia.get_pixel(x, y), ib.get_pixel(x, y));
            let delta = (0..3)
                .map(|c| (pa[c] as i32 - pb[c] as i32).abs())
                .max()
                .unwrap_or(0);
            if delta > 40 {
                changed += 1;
                image::Rgb([220, 30, 40])
            } else {
                let l = ((pb[0] as u32 * 3 + pb[1] as u32 * 6 + pb[2] as u32) / 10) as u8;
                let faded = 255 - (255 - l) / 4;
                image::Rgb([faded, faded, faded])
            }
        });
        let total = (ia.width() as u64 * ia.height() as u64).max(1);
        diffs.push(PageDiff {
            page: p,
            changed: changed as f64 / total as f64,
            missing_in: None,
        });
        if changed > 0 {
            files.push(OutFile {
                suffix: format!("diff-page-{}", p + 1),
                ext: "png".into(),
                bytes: images::png_encode(&DynamicImage::ImageRgb8(out), None),
            });
        }
    }
    Ok((files, diffs))
}

// ---------------------------------------------------------------- dispatch

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct Common {
    password: Option<String>,
    pages: Option<String>,
    dpi: Option<f32>,
}

pub const OPS: &[&str] = &["render", "text", "compare", "extreme", "page-count"];

/// Same contract as [`pdfvault_core::dispatch::run`].
pub fn run(op: &str, inputs: Vec<Vec<u8>>, options: &str) -> Result<Output> {
    let v: Value = if options.trim().is_empty() {
        Value::Null
    } else {
        serde_json::from_str(options).map_err(|e| Error::option(e.to_string()))?
    };
    let parse = |v: &Value| -> Result<Common> {
        if v.is_null() {
            Ok(Common::default())
        } else {
            serde_json::from_value(v.clone()).map_err(|e| Error::option(e.to_string()))
        }
    };
    let one = || -> Result<&Vec<u8>> {
        match inputs.as_slice() {
            [only] => Ok(only),
            _ => Err(Error::option(format!("{op} takes exactly one input"))),
        }
    };
    match op {
        "render" => {
            let o: RenderOptions = if v.is_null() {
                RenderOptions::default()
            } else {
                serde_json::from_value(v).map_err(|e| Error::option(e.to_string()))?
            };
            let (files, pages) = render_pages(one()?, &o)?;
            Ok(Output {
                files,
                report: json!({ "pages": pages }),
            })
        }
        "text" => {
            let c = parse(&v)?;
            let pages = extract_text(one()?, c.pages.as_deref(), c.password.as_deref())?;
            let list: Vec<Value> = pages
                .into_iter()
                .map(|(p, t)| json!({ "page": p, "spans": t.spans }))
                .collect();
            Ok(Output {
                files: vec![],
                report: json!({ "pages": list }),
            })
        }
        "compare" => {
            let c = parse(&v)?;
            let [a, b] = inputs.as_slice() else {
                return Err(Error::option("compare takes two PDFs"));
            };
            let (files, pages) = compare(a, b, c.dpi.unwrap_or(72.0), c.password.as_deref())?;
            let identical = pages.iter().all(|p| p.changed == 0.0);
            Ok(Output {
                files,
                report: json!({ "identical": identical, "pages": pages }),
            })
        }
        "extreme" => {
            let o: compress::CompressOptions = if v.is_null() {
                Default::default()
            } else {
                serde_json::from_value(v).map_err(|e| Error::option(e.to_string()))?
            };
            let (bytes, report) = extreme(one()?, &o)?;
            Ok(Output {
                files: vec![OutFile {
                    suffix: "compressed".into(),
                    ext: "pdf".into(),
                    bytes,
                }],
                report: serde_json::to_value(report).map_err(|e| Error::option(e.to_string()))?,
            })
        }
        "page-count" => {
            let c = parse(&v)?;
            let doc = open(one()?, c.password.as_deref())?;
            let sizes: Vec<Value> = doc
                .pages()
                .iter()
                .map(|p| {
                    let (w, h) = p.render_dimensions();
                    json!({ "width": w, "height": h })
                })
                .collect();
            Ok(Output {
                files: vec![],
                report: json!({ "pages": doc.pages().len(), "sizes": sizes }),
            })
        }
        other => Err(Error::option(format!("unknown operation '{other}'"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pdfvault_core::fixtures;
    use pdfvault_core::lopdf::dictionary;

    #[test]
    fn renders_pages_in_formats() {
        let input = fixtures::text_pdf(2);
        for (fmt, magic) in [
            ("png", &b"\x89PNG"[..]),
            ("jpeg", &[0xFF, 0xD8][..]),
            ("webp", &b"RIFF"[..]),
        ] {
            let o = RenderOptions {
                format: fmt.into(),
                dpi: 72.0,
                ..Default::default()
            };
            let (files, meta) = render_pages(&input, &o).unwrap();
            assert_eq!(files.len(), 2);
            assert!(files[0].bytes.starts_with(magic), "{fmt}");
            assert_eq!((meta[0].width, meta[0].height), (612, 792));
        }
        let o = RenderOptions {
            format: "rgba".into(),
            dpi: 36.0,
            pages: Some("2".into()),
            ..Default::default()
        };
        let (files, meta) = render_pages(&input, &o).unwrap();
        assert_eq!(
            files[0].bytes.len(),
            (meta[0].width * meta[0].height * 4) as usize
        );
        // Text is drawn: some pixels are dark.
        assert!(files[0].bytes.chunks(4).any(|p| p[0] < 100));
    }

    #[test]
    fn text_with_positions() {
        let pages = extract_text(&fixtures::text_pdf(1), None, None).unwrap();
        let spans = &pages[0].1.spans;
        assert_eq!(spans[0].text, "Page 1");
        assert!((spans[0].size - 24.0).abs() < 0.5, "{}", spans[0].size);
        assert!(spans[0].y < 80.0 && spans[0].x > 70.0);
        assert!(
            spans
                .iter()
                .any(|s| s.text.contains("quick brown fox") && (s.size - 12.0).abs() < 0.5)
        );
        let md = pdfvault_core::markdown::to_markdown(&[pages[0].1.clone()], &Default::default());
        assert!(md.starts_with("# Page 1\n\nThe quick brown fox"), "{md}");
    }

    #[test]
    fn compare_finds_changes() {
        let a = fixtures::text_pdf(2);
        let (_, same) = compare(&a, &a, 36.0, None).unwrap();
        assert!(same.iter().all(|p| p.changed == 0.0));
        let b = pdfvault_core::stamp::watermark(
            &a,
            &pdfvault_core::stamp::WatermarkOptions {
                text: "CHANGED".into(),
                opacity: 1.0,
                pages: Some("2".into()),
                ..Default::default()
            },
        )
        .unwrap();
        let (files, diffs) = compare(&a, &b, 36.0, None).unwrap();
        assert_eq!(diffs[0].changed, 0.0);
        assert!(diffs[1].changed > 0.001);
        assert_eq!(files.len(), 1);
        let (_, d) = compare(&a, &fixtures::text_pdf(3), 36.0, None).unwrap();
        assert_eq!(d[2].missing_in.as_deref(), Some("first"));
    }

    #[test]
    fn extreme_rasterizes() {
        let input = fixtures::photo_pdf();
        let (out, rep) = extreme(&input, &Default::default()).unwrap();
        assert!(out.len() < input.len());
        assert!(!rep.kept_original);
        let doc = pdf::load(&out, None).unwrap();
        assert_eq!(doc.get_pages().len(), 2);
    }

    /// Mean absolute difference per channel between a page render and the
    /// image drawn on it at 1:1.
    fn render_error(jpeg: Vec<u8>, img: &image::RgbImage) -> f64 {
        let (w, h) = img.dimensions();
        let mut b = fixtures::Builder::new();
        let id = b.doc.add_object(pdfvault_core::lopdf::Stream::new(
            dictionary! {
                "Type" => "XObject", "Subtype" => "Image", "Width" => w as i64, "Height" => h as i64,
                "ColorSpace" => "DeviceRGB", "BitsPerComponent" => 8, "Filter" => "DCTDecode",
            },
            jpeg,
        ));
        b.image_page(id, w as f32, h as f32);
        let doc = open(&b.finish(), None).unwrap();
        let r = render_page(&doc, 0, 72.0).unwrap();
        let mut sum = 0f64;
        for (x, y, s) in img.enumerate_pixels() {
            let a = r.get_pixel(x + 36, y + 36);
            sum += (0..3)
                .map(|c| (a[c] as f64 - s[c] as f64).abs())
                .sum::<f64>();
        }
        sum / (w as f64 * h as f64 * 3.0)
    }

    #[test]
    fn our_jpegs_render_correctly() {
        // Regression: optimized Huffman tables from jpeg-encoder decoded as noise in hayro.
        let img = fixtures::photo(400, 300, 3);
        let ours = raster::encode_jpeg(&Pixels::Rgb(img.clone()), 80).unwrap();
        assert!(render_error(ours, &img) < 12.0);
        // And end to end: a compressed photo PDF renders like the original.
        let input = fixtures::photo_pdf();
        let (out, _) = compress::compress(
            &input,
            &compress::CompressOptions {
                preset: compress::Preset::Strong,
                ..Default::default()
            },
        )
        .unwrap();
        let a = render_page(&open(&input, None).unwrap(), 0, 36.0).unwrap();
        let b = render_page(&open(&out, None).unwrap(), 0, 36.0).unwrap();
        let diff: f64 = a
            .pixels()
            .zip(b.pixels())
            .map(|(p, q)| {
                (0..3)
                    .map(|c| (p[c] as f64 - q[c] as f64).abs())
                    .sum::<f64>()
            })
            .sum::<f64>()
            / (a.width() as f64 * a.height() as f64 * 3.0);
        assert!(
            diff < 12.0,
            "compressed page differs by {diff:.1} per channel"
        );
    }

    #[test]
    fn encrypted_input() {
        let enc = pdfvault_core::security::encrypt(
            &fixtures::text_pdf(1),
            &pdfvault_core::security::EncryptOptions {
                user_password: "pw".into(),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(open(&enc, None).err().unwrap().code(), "password_required");
        assert_eq!(
            open(&enc, Some("bad")).err().unwrap().code(),
            "wrong_password"
        );
        let o = RenderOptions {
            dpi: 36.0,
            password: Some("pw".into()),
            ..Default::default()
        };
        assert_eq!(render_pages(&enc, &o).unwrap().0.len(), 1);
    }
}
