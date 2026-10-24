//! Deterministic test documents, generated in code so the repository ships no
//! third-party binaries. Used by the Rust tests and, through the `fixtures`
//! binary, by the TypeScript and Playwright suites.

use crate::pdf;
use image::{ImageBuffer, Rgb, RgbImage, Rgba, RgbaImage};
use lopdf::content::{Content, Operation};
use lopdf::{Dictionary, Document, Object, ObjectId, Stream, dictionary};

/// Small xorshift PRNG: identical output on every platform.
pub struct Rng(u64);
impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed.max(1))
    }
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    pub fn byte(&mut self) -> u8 {
        (self.next_u64() >> 32) as u8
    }
}

/// A photo-like image: smooth gradients, a few blobs and sensor noise, which
/// is what JPEG re-encoding and downsampling are meant for.
pub fn photo(w: u32, h: u32, seed: u64) -> RgbImage {
    let mut rng = Rng::new(seed);
    ImageBuffer::from_fn(w, h, |x, y| {
        let fx = x as f32 / w as f32;
        let fy = y as f32 / h as f32;
        let blob = ((fx * 9.0).sin() * (fy * 7.0).cos() * 0.5 + 0.5) * 255.0;
        let noise = (rng.byte() as f32 - 128.0) * 0.12;
        Rgb([
            (fx * 200.0 + noise + 30.0).clamp(0.0, 255.0) as u8,
            (blob * 0.8 + noise).clamp(0.0, 255.0) as u8,
            (fy * 220.0 - noise + 20.0).clamp(0.0, 255.0) as u8,
        ])
    })
}

/// Line art: flat colors and hard edges, where lossless coding wins.
pub fn line_art(w: u32, h: u32) -> RgbImage {
    ImageBuffer::from_fn(w, h, |x, y| {
        if (x / 40 + y / 40) % 2 == 0 {
            Rgb([255, 255, 255])
        } else if x % 97 < 6 || y % 89 < 6 {
            Rgb([10, 10, 10])
        } else {
            Rgb([30, 90, 200])
        }
    })
}

pub fn with_alpha(img: &RgbImage) -> RgbaImage {
    ImageBuffer::from_fn(img.width(), img.height(), |x, y| {
        let p = img.get_pixel(x, y);
        let a = if (x as i64 - img.width() as i64 / 2).pow(2)
            + (y as i64 - img.height() as i64 / 2).pow(2)
            < (img.width() as i64 / 3).pow(2)
        {
            255
        } else {
            60
        };
        Rgba([p[0], p[1], p[2], a])
    })
}

pub fn jpeg(img: &RgbImage, quality: u8) -> Vec<u8> {
    let mut out = Vec::new();
    let enc = jpeg_encoder::Encoder::new(&mut out, quality);
    enc.encode(
        img.as_raw(),
        img.width() as u16,
        img.height() as u16,
        jpeg_encoder::ColorType::Rgb,
    )
    .expect("encode fixture jpeg");
    out
}

pub fn png(img: &image::DynamicImage) -> Vec<u8> {
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png)
        .expect("encode fixture png");
    out.into_inner()
}

/// Builder for small, valid documents with standard-14 fonts only.
pub struct Builder {
    pub doc: Document,
    pages_id: ObjectId,
    pub font_id: ObjectId,
    kids: Vec<Object>,
    outlines: Option<ObjectId>,
    catalog_extra: Vec<(String, Object)>,
}

impl Default for Builder {
    fn default() -> Self {
        Self::new()
    }
}

impl Builder {
    pub fn new() -> Self {
        let mut doc = Document::with_version("1.7");
        let pages_id = doc.new_object_id();
        let font_id = doc.add_object(dictionary! {
            "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica", "Encoding" => "WinAnsiEncoding",
        });
        Builder {
            doc,
            pages_id,
            font_id,
            kids: vec![],
            outlines: None,
            catalog_extra: vec![],
        }
    }

    /// Add a page of `size` drawing `ops`; `xobjects` are named Im0, Im1...
    pub fn page(
        &mut self,
        size: (f32, f32),
        ops: Vec<Operation>,
        xobjects: Vec<ObjectId>,
    ) -> ObjectId {
        let content = Content { operations: ops }
            .encode()
            .expect("encode content");
        let mut stream = Stream::new(Dictionary::new(), content);
        let _ = stream.compress();
        let content_id = self.doc.add_object(stream);
        let mut xo = Dictionary::new();
        for (i, id) in xobjects.iter().enumerate() {
            xo.set(format!("Im{i}"), *id);
        }
        let page = self.doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => self.pages_id,
            "MediaBox" => vec![0.into(), 0.into(), size.0.into(), size.1.into()],
            "Contents" => content_id,
            "Resources" => dictionary! { "Font" => dictionary! { "F1" => self.font_id }, "XObject" => xo },
        });
        self.kids.push(page.into());
        page
    }

    pub fn text_page(&mut self, lines: &[&str]) -> ObjectId {
        let mut ops = vec![
            Operation::new("BT", vec![]),
            Operation::new("Tf", vec!["F1".into(), 24.into()]),
        ];
        ops.push(Operation::new("Td", vec![72.into(), 720.into()]));
        for (i, line) in lines.iter().enumerate() {
            if i == 0 {
                ops.push(Operation::new("Tj", vec![Object::string_literal(*line)]));
                ops.push(Operation::new("Tf", vec!["F1".into(), 12.into()]));
            } else {
                ops.push(Operation::new("Td", vec![0.into(), (-18).into()]));
                ops.push(Operation::new("Tj", vec![Object::string_literal(*line)]));
            }
        }
        ops.push(Operation::new("ET", vec![]));
        self.page((612.0, 792.0), ops, vec![])
    }

    /// A page showing one image across `width_pt` points.
    pub fn image_page(&mut self, image: ObjectId, width_pt: f32, height_pt: f32) -> ObjectId {
        let ops = vec![
            Operation::new("q", vec![]),
            Operation::new(
                "cm",
                vec![
                    width_pt.into(),
                    0.into(),
                    0.into(),
                    height_pt.into(),
                    36.into(),
                    36.into(),
                ],
            ),
            Operation::new("Do", vec!["Im0".into()]),
            Operation::new("Q", vec![]),
        ];
        self.page((width_pt + 72.0, height_pt + 72.0), ops, vec![image])
    }

    pub fn jpeg_image(&mut self, img: &RgbImage, quality: u8) -> ObjectId {
        let data = jpeg(img, quality);
        self.doc.add_object(Stream::new(
            dictionary! {
                "Type" => "XObject", "Subtype" => "Image",
                "Width" => img.width() as i64, "Height" => img.height() as i64,
                "ColorSpace" => "DeviceRGB", "BitsPerComponent" => 8, "Filter" => "DCTDecode",
            },
            data,
        ))
    }

    pub fn flate_image(&mut self, img: &RgbImage, smask: Option<&[u8]>) -> ObjectId {
        let mut dict = dictionary! {
            "Type" => "XObject", "Subtype" => "Image",
            "Width" => img.width() as i64, "Height" => img.height() as i64,
            "ColorSpace" => "DeviceRGB", "BitsPerComponent" => 8,
        };
        if let Some(alpha) = smask {
            let mut s = Stream::new(
                dictionary! {
                    "Type" => "XObject", "Subtype" => "Image",
                    "Width" => img.width() as i64, "Height" => img.height() as i64,
                    "ColorSpace" => "DeviceGray", "BitsPerComponent" => 8,
                },
                alpha.to_vec(),
            );
            let _ = s.compress();
            let id = self.doc.add_object(s);
            dict.set("SMask", id);
        }
        // Deliberately weak compression, as many producers write.
        let data = miniz_oxide::deflate::compress_to_vec_zlib(img.as_raw(), 1);
        dict.set("Filter", "FlateDecode");
        self.doc.add_object(Stream::new(dict, data))
    }

    pub fn outline(&mut self, entries: &[(&str, usize)]) {
        let outlines_id = self.doc.new_object_id();
        let ids: Vec<ObjectId> = entries.iter().map(|_| self.doc.new_object_id()).collect();
        for (i, (title, page)) in entries.iter().enumerate() {
            let mut d = dictionary! {
                "Title" => Object::string_literal(*title),
                "Parent" => outlines_id,
                "Dest" => vec![self.kids[*page].clone(), "Fit".into()],
            };
            if i > 0 {
                d.set("Prev", ids[i - 1]);
            }
            if i + 1 < ids.len() {
                d.set("Next", ids[i + 1]);
            }
            self.doc.objects.insert(ids[i], d.into());
        }
        self.doc.objects.insert(
            outlines_id,
            dictionary! {
                "Type" => "Outlines", "First" => ids[0], "Last" => *ids.last().unwrap(), "Count" => ids.len() as i64,
            }
            .into(),
        );
        self.outlines = Some(outlines_id);
    }

    pub fn finish(mut self) -> Vec<u8> {
        self.finish_doc();
        pdf::save(&mut self.doc).expect("save fixture")
    }

    pub fn finish_doc(&mut self) {
        let count = self.kids.len() as i64;
        self.doc.objects.insert(
            self.pages_id,
            dictionary! { "Type" => "Pages", "Kids" => self.kids.clone(), "Count" => count }.into(),
        );
        let mut catalog = dictionary! { "Type" => "Catalog", "Pages" => self.pages_id };
        if let Some(o) = self.outlines {
            catalog.set("Outlines", o);
        }
        for (k, v) in std::mem::take(&mut self.catalog_extra) {
            catalog.set(k, v);
        }
        let catalog_id = self.doc.add_object(catalog);
        self.doc.trailer.set("Root", catalog_id);
        let info = self.doc.add_object(dictionary! {
            "Title" => Object::string_literal("Fixture"),
            "Author" => Object::string_literal("pdfvault tests"),
            "Producer" => Object::string_literal("fixtures"),
        });
        self.doc.trailer.set("Info", info);
    }

    pub fn catalog_set(&mut self, key: &str, value: Object) {
        self.catalog_extra.push((key.to_string(), value));
    }
}

/// N pages of text, each titled "Page i".
pub fn text_pdf(pages: usize) -> Vec<u8> {
    let mut b = Builder::new();
    for i in 1..=pages {
        let title = format!("Page {i}");
        b.text_page(&[
            &title,
            "The quick brown fox jumps over the lazy dog.",
            "Second line of body text.",
        ]);
    }
    b.finish()
}

/// Two pages of 300-DPI photos: one JPEG at quality 95, one weakly flated with an alpha mask.
pub fn photo_pdf() -> Vec<u8> {
    let mut b = Builder::new();
    let img = photo(1500, 1000, 7);
    let jpg = b.jpeg_image(&img, 95);
    b.image_page(jpg, 360.0, 240.0);
    let img2 = photo(900, 600, 11);
    let alpha: Vec<u8> = with_alpha(&img2).pixels().map(|p| p[3]).collect();
    let fl = b.flate_image(&img2, Some(&alpha));
    b.image_page(fl, 216.0, 144.0);
    b.finish()
}

/// Line-art scan at 300 DPI.
pub fn lineart_pdf() -> Vec<u8> {
    let mut b = Builder::new();
    let img = line_art(1200, 1200);
    let id = b.flate_image(&img, None);
    b.image_page(id, 288.0, 288.0);
    b.finish()
}

/// Three chapters with one bookmark each.
pub fn outlined_pdf() -> Vec<u8> {
    let mut b = Builder::new();
    for (i, t) in ["Intro", "Intro 2", "Methods", "Results", "Results 2"]
        .iter()
        .enumerate()
    {
        let title = format!("{t} ({})", i + 1);
        b.text_page(&[&title]);
    }
    b.outline(&[("Intro", 0), ("Methods", 2), ("Results", 3)]);
    b.finish()
}

/// One page with a filled text field and a checkbox, both with appearance streams.
pub fn form_pdf() -> Vec<u8> {
    let mut b = Builder::new();
    let page = b.text_page(&["Form", "Name:"]);
    let font = b.font_id;
    let ap = {
        let mut s = Stream::new(
            dictionary! {
                "Type" => "XObject", "Subtype" => "Form", "BBox" => vec![0.into(), 0.into(), 200.into(), 20.into()],
                "Resources" => dictionary! { "Font" => dictionary! { "F1" => font } },
            },
            b"/Tx BMC q BT /F1 12 Tf 2 5 Td (Ada Lovelace) Tj ET Q EMC".to_vec(),
        );
        let _ = s.compress();
        b.doc.add_object(s)
    };
    let check_on = b.doc.add_object(Stream::new(
        dictionary! { "Type" => "XObject", "Subtype" => "Form", "BBox" => vec![0.into(), 0.into(), 14.into(), 14.into()] },
        b"0 0 0 rg 2 2 10 10 re f".to_vec(),
    ));
    let check_off = b.doc.add_object(Stream::new(
        dictionary! { "Type" => "XObject", "Subtype" => "Form", "BBox" => vec![0.into(), 0.into(), 14.into(), 14.into()] },
        b"".to_vec(),
    ));
    let field = b.doc.add_object(dictionary! {
        "Type" => "Annot", "Subtype" => "Widget", "FT" => "Tx", "T" => Object::string_literal("name"),
        "V" => Object::string_literal("Ada Lovelace"), "Rect" => vec![120.into(), 680.into(), 320.into(), 700.into()],
        "P" => page, "F" => 4, "AP" => dictionary! { "N" => ap },
    });
    let check = b.doc.add_object(dictionary! {
        "Type" => "Annot", "Subtype" => "Widget", "FT" => "Btn", "T" => Object::string_literal("agree"),
        "V" => "Yes", "AS" => "Yes", "Rect" => vec![120.into(), 650.into(), 134.into(), 664.into()],
        "P" => page, "F" => 4, "AP" => dictionary! { "N" => dictionary! { "Yes" => check_on, "Off" => check_off } },
    });
    b.doc
        .get_dictionary_mut(page)
        .unwrap()
        .set("Annots", vec![field.into(), check.into()]);
    let acro = b
        .doc
        .add_object(dictionary! { "Fields" => vec![field.into(), check.into()] });
    b.catalog_set("AcroForm", acro.into());
    b.finish()
}

/// Whether any stream of the PDF, once decoded, contains `needle`. Text drawn
/// inside form XObjects (field appearances) is invisible to `extract_text`.
pub fn any_stream_contains(pdf_bytes: &[u8], needle: &[u8]) -> bool {
    let doc = crate::pdf::load(pdf_bytes, None).unwrap();
    doc.objects
        .values()
        .filter_map(|o| o.as_stream().ok())
        .any(|s| {
            let data = s
                .decompressed_content()
                .unwrap_or_else(|_| s.content.clone());
            data.windows(needle.len()).any(|w| w == needle)
        })
}

/// A font the engine ships (models/fonts, from `bun run fetch-models`), such
/// as "LiberationSans-Regular.ttf".
pub fn bundled_font(name: &str) -> Vec<u8> {
    let dir = std::env::var("PDFVAULT_MODELS")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| {
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../models")
        });
    std::fs::read(dir.join("fonts").join(name))
        .unwrap_or_else(|_| panic!("{name} missing: run `bun run fetch-models` first"))
}

/// Make `doc` a one-page, US Letter document drawing `content` with `resources`.
pub fn single_page(doc: &mut Document, content: ObjectId, resources: Dictionary) -> ObjectId {
    let pages = doc.new_object_id();
    let page = doc.add_object(dictionary! {
        "Type" => "Page", "Parent" => pages,
        "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
        "Contents" => content, "Resources" => resources,
    });
    doc.objects.insert(
        pages,
        dictionary! { "Type" => "Pages", "Kids" => vec![page.into()], "Count" => 1 }.into(),
    );
    let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages });
    doc.trailer.set("Root", catalog);
    page
}

/// Bytes that start like a PDF and then fall apart.
pub fn corrupt_pdf() -> Vec<u8> {
    let mut v = text_pdf(2);
    let cut = v.len() * 2 / 3;
    v.truncate(cut);
    v.extend(b"\ngarbage that is not a pdf object\n");
    v
}
