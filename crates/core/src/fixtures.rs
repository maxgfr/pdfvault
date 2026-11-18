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

/// A short Markdown note with the blocks `md2pdf` lays out: headings, a list,
/// a table, code and accented text.
pub fn markdown() -> Vec<u8> {
    "# Notes de réunion\n\nLe devis de **1 250,00 €** est *accepté*.\n\n\
     - Premier point\n- Second point\n\n\
     | Date | Montant |\n|---|---:|\n| 2026-01-05 | 534,95 |\n| 2026-02-05 | 534,95 |\n\n\
     ```\ntotal = 1069,90\n```\n"
        .as_bytes()
        .to_vec()
}

// ------------------------------------------------------------ Office (OOXML)

const XML_DECL: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#;
const NS_REL: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const NS_PKG_REL: &str = "http://schemas.openxmlformats.org/package/2006/relationships";

/// A zip of `(name, contents)` parts, in order.
fn package(parts: Vec<(&str, Vec<u8>)>) -> Vec<u8> {
    let files: Vec<(String, Vec<u8>)> =
        parts.into_iter().map(|(n, c)| (n.to_string(), c)).collect();
    crate::epub::zip(&files)
}

/// `[Content_Types].xml` with these defaults `(extension, type)` and overrides `(part, type)`.
fn content_types(defaults: &[(&str, &str)], overrides: &[(&str, String)]) -> Vec<u8> {
    let mut s = format!(
        r#"{XML_DECL}<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/>"#
    );
    for (ext, ty) in defaults {
        s += &format!(r#"<Default Extension="{ext}" ContentType="{ty}"/>"#);
    }
    for (part, ty) in overrides {
        s += &format!(r#"<Override PartName="/{part}" ContentType="{ty}"/>"#);
    }
    s += "</Types>";
    s.into_bytes()
}

/// A relationships part of `(id, type, target)`, the type relative to the
/// officeDocument relationships namespace.
fn relationships(rels: &[(&str, &str, &str)]) -> Vec<u8> {
    let mut s = format!(r#"{XML_DECL}<Relationships xmlns="{NS_PKG_REL}">"#);
    for (id, ty, target) in rels {
        s += &format!(r#"<Relationship Id="{id}" Type="{NS_REL}/{ty}" Target="{target}"/>"#);
    }
    s += "</Relationships>";
    s.into_bytes()
}

/// A Word document: a Heading 1 title, a paragraph with bold and italic runs,
/// a bulleted list, a 3×3 table with borders and a PNG picture. Body text is
/// Calibri and the heading Cambria, Word's defaults.
pub fn docx() -> Vec<u8> {
    const W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
    let run = |text: &str, props: &str| {
        format!(r#"<w:r><w:rPr>{props}</w:rPr><w:t xml:space="preserve">{text}</w:t></w:r>"#)
    };
    let bullet = |text: &str| {
        format!(
            r#"<w:p><w:pPr><w:pStyle w:val="ListParagraph"/><w:numPr><w:ilvl w:val="0"/><w:numId w:val="1"/></w:numPr></w:pPr>{}</w:p>"#,
            run(text, "")
        )
    };
    let row = |cells: [&str; 3], header: bool| {
        let props = if header { "<w:b/>" } else { "" };
        let tcs: String = cells
            .iter()
            .map(|c| {
                format!(
                    r#"<w:tc><w:tcPr><w:tcW w:w="3000" w:type="dxa"/></w:tcPr><w:p>{}</w:p></w:tc>"#,
                    run(c, props)
                )
            })
            .collect();
        format!("<w:tr>{tcs}</w:tr>")
    };
    let borders: String = ["top", "left", "bottom", "right", "insideH", "insideV"]
        .iter()
        .map(|side| format!(r#"<w:{side} w:val="single" w:sz="4" w:space="0" w:color="000000"/>"#))
        .collect();
    // 300×120 px shown at 2 × 0.8 inches (914400 EMU per inch).
    let (cx, cy) = (1_828_800, 731_520);
    let picture = format!(
        r#"<w:p><w:r><w:drawing><wp:inline distT="0" distB="0" distL="0" distR="0"><wp:extent cx="{cx}" cy="{cy}"/><wp:docPr id="1" name="Logo"/><a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture"><pic:pic><pic:nvPicPr><pic:cNvPr id="1" name="logo.png"/><pic:cNvPicPr/></pic:nvPicPr><pic:blipFill><a:blip r:embed="rId3"/><a:stretch><a:fillRect/></a:stretch></pic:blipFill><pic:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></pic:spPr></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing></w:r></w:p>"#
    );
    let document = format!(
        r#"{XML_DECL}<w:document xmlns:w="{W}" xmlns:r="{NS_REL}" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture"><w:body><w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr>{title}</w:p><w:p>{p1}{p2}{p3}{p4}{p5}</w:p>{b1}{b2}{b3}<w:tbl><w:tblPr><w:tblW w:w="9000" w:type="dxa"/><w:tblBorders>{borders}</w:tblBorders></w:tblPr><w:tblGrid><w:gridCol w:w="3000"/><w:gridCol w:w="3000"/><w:gridCol w:w="3000"/></w:tblGrid>{r1}{r2}{r3}</w:tbl><w:p/>{picture}<w:sectPr><w:pgSz w:w="11906" w:h="16838"/><w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440" w:header="708" w:footer="708" w:gutter="0"/></w:sectPr></w:body></w:document>"#,
        title = run("Quarterly report", ""),
        p1 = run("Revenue grew by ", ""),
        p2 = run("twelve percent", "<w:b/>"),
        p3 = run(" over the quarter, ", ""),
        p4 = run("café", "<w:i/>"),
        p5 = run(" included.", ""),
        b1 = bullet("Paper"),
        b2 = bullet("Ink"),
        b3 = bullet("Postage"),
        r1 = row(["Item", "Q1", "Q2"], true),
        r2 = row(["Paper", "120", "135"], false),
        r3 = row(["Ink", "80", "95"], false),
    );
    let styles = format!(
        r#"{XML_DECL}<w:styles xmlns:w="{W}"><w:docDefaults><w:rPrDefault><w:rPr><w:rFonts w:ascii="Calibri" w:hAnsi="Calibri" w:cs="Calibri"/><w:sz w:val="22"/><w:lang w:val="en-US"/></w:rPr></w:rPrDefault><w:pPrDefault><w:pPr><w:spacing w:after="160" w:line="259" w:lineRule="auto"/></w:pPr></w:pPrDefault></w:docDefaults><w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/></w:style><w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:pPr><w:keepNext/><w:spacing w:before="240" w:after="120"/><w:outlineLvl w:val="0"/></w:pPr><w:rPr><w:rFonts w:ascii="Cambria" w:hAnsi="Cambria"/><w:b/><w:sz w:val="32"/></w:rPr></w:style><w:style w:type="paragraph" w:styleId="ListParagraph"><w:name w:val="List Paragraph"/><w:basedOn w:val="Normal"/><w:pPr><w:ind w:left="720"/></w:pPr></w:style></w:styles>"#
    );
    let numbering = format!(
        r#"{XML_DECL}<w:numbering xmlns:w="{W}"><w:abstractNum w:abstractNumId="0"><w:multiLevelType w:val="hybridMultilevel"/><w:lvl w:ilvl="0"><w:start w:val="1"/><w:numFmt w:val="bullet"/><w:lvlText w:val="•"/><w:lvlJc w:val="left"/><w:pPr><w:ind w:left="720" w:hanging="360"/></w:pPr></w:lvl></w:abstractNum><w:num w:numId="1"><w:abstractNumId w:val="0"/></w:num></w:numbering>"#
    );
    let wml = "application/vnd.openxmlformats-officedocument.wordprocessingml";
    package(vec![
        (
            "[Content_Types].xml",
            content_types(
                &[("png", "image/png")],
                &[
                    ("word/document.xml", format!("{wml}.document.main+xml")),
                    ("word/styles.xml", format!("{wml}.styles+xml")),
                    ("word/numbering.xml", format!("{wml}.numbering+xml")),
                ],
            ),
        ),
        (
            "_rels/.rels",
            relationships(&[("rId1", "officeDocument", "word/document.xml")]),
        ),
        ("word/document.xml", document.into_bytes()),
        (
            "word/_rels/document.xml.rels",
            relationships(&[
                ("rId1", "styles", "styles.xml"),
                ("rId2", "numbering", "numbering.xml"),
                ("rId3", "image", "media/image1.png"),
            ]),
        ),
        ("word/styles.xml", styles.into_bytes()),
        ("word/numbering.xml", numbering.into_bytes()),
        (
            "word/media/image1.png",
            png(&image::DynamicImage::ImageRgb8(line_art(300, 120))),
        ),
    ])
}

/// An Excel workbook of two sheets: "Summary" (a bold title and a total whose
/// formula, =SUM(Data!B2:B4), carries its cached value 335) and "Data" (a
/// header row, three items and their amounts). Strings are shared.
pub fn xlsx() -> Vec<u8> {
    const S: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
    let strings = [
        "Quarterly summary",
        "Total",
        "Item",
        "Amount",
        "Paper",
        "Ink",
        "Postage",
    ];
    let items: String = strings
        .iter()
        .map(|s| format!("<si><t>{s}</t></si>"))
        .collect();
    let shared = format!(
        r#"{XML_DECL}<sst xmlns="{S}" count="{n}" uniqueCount="{n}">{items}</sst>"#,
        n = strings.len()
    );
    let sheet = |rows: &str| {
        format!(
            r#"{XML_DECL}<worksheet xmlns="{S}" xmlns:r="{NS_REL}"><cols><col min="1" max="1" width="24" customWidth="1"/><col min="2" max="2" width="12" customWidth="1"/></cols><sheetData>{rows}</sheetData><pageMargins left="0.7" right="0.7" top="0.75" bottom="0.75" header="0.3" footer="0.3"/></worksheet>"#
        )
    };
    let summary = sheet(
        r#"<row r="1"><c r="A1" s="1" t="s"><v>0</v></c></row><row r="3"><c r="A3" t="s"><v>1</v></c><c r="B3"><f>SUM(Data!B2:B4)</f><v>335</v></c></row>"#,
    );
    let data = sheet(
        r#"<row r="1"><c r="A1" s="1" t="s"><v>2</v></c><c r="B1" s="1" t="s"><v>3</v></c></row><row r="2"><c r="A2" t="s"><v>4</v></c><c r="B2"><v>120</v></c></row><row r="3"><c r="A3" t="s"><v>5</v></c><c r="B3"><v>80</v></c></row><row r="4"><c r="A4" t="s"><v>6</v></c><c r="B4"><v>135</v></c></row>"#,
    );
    let workbook = format!(
        r#"{XML_DECL}<workbook xmlns="{S}" xmlns:r="{NS_REL}"><sheets><sheet name="Summary" sheetId="1" r:id="rId1"/><sheet name="Data" sheetId="2" r:id="rId2"/></sheets><calcPr calcId="191029"/></workbook>"#
    );
    let styles = format!(
        r#"{XML_DECL}<styleSheet xmlns="{S}"><fonts count="2"><font><sz val="11"/><name val="Calibri"/><family val="2"/></font><font><b/><sz val="11"/><name val="Calibri"/><family val="2"/></font></fonts><fills count="2"><fill><patternFill patternType="none"/></fill><fill><patternFill patternType="gray125"/></fill></fills><borders count="1"><border><left/><right/><top/><bottom/><diagonal/></border></borders><cellStyleXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0"/></cellStyleXfs><cellXfs count="2"><xf numFmtId="0" fontId="0" fillId="0" borderId="0" xfId="0"/><xf numFmtId="0" fontId="1" fillId="0" borderId="0" xfId="0" applyFont="1"/></cellXfs><cellStyles count="1"><cellStyle name="Normal" xfId="0" builtinId="0"/></cellStyles></styleSheet>"#
    );
    let sml = "application/vnd.openxmlformats-officedocument.spreadsheetml";
    package(vec![
        (
            "[Content_Types].xml",
            content_types(
                &[],
                &[
                    ("xl/workbook.xml", format!("{sml}.sheet.main+xml")),
                    ("xl/worksheets/sheet1.xml", format!("{sml}.worksheet+xml")),
                    ("xl/worksheets/sheet2.xml", format!("{sml}.worksheet+xml")),
                    ("xl/sharedStrings.xml", format!("{sml}.sharedStrings+xml")),
                    ("xl/styles.xml", format!("{sml}.styles+xml")),
                ],
            ),
        ),
        (
            "_rels/.rels",
            relationships(&[("rId1", "officeDocument", "xl/workbook.xml")]),
        ),
        ("xl/workbook.xml", workbook.into_bytes()),
        (
            "xl/_rels/workbook.xml.rels",
            relationships(&[
                ("rId1", "worksheet", "worksheets/sheet1.xml"),
                ("rId2", "worksheet", "worksheets/sheet2.xml"),
                ("rId3", "sharedStrings", "sharedStrings.xml"),
                ("rId4", "styles", "styles.xml"),
            ]),
        ),
        ("xl/worksheets/sheet1.xml", summary.into_bytes()),
        ("xl/worksheets/sheet2.xml", data.into_bytes()),
        ("xl/sharedStrings.xml", shared.into_bytes()),
        ("xl/styles.xml", styles.into_bytes()),
    ])
}

/// A 16:9 PowerPoint deck of two slides, each a title and a text box:
/// "Roadmap", then "Next steps".
pub fn pptx() -> Vec<u8> {
    pptx_with(false)
}

/// [`pptx`] with its second slide marked hidden.
pub fn pptx_hidden_slide() -> Vec<u8> {
    pptx_with(true)
}

fn pptx_with(hide_second: bool) -> Vec<u8> {
    const NS: &str = r#"xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main""#;
    let text_box = |id: u32, name: &str, y: u32, cy: u32, size: u32, text: &str| {
        format!(
            r#"<p:sp><p:nvSpPr><p:cNvPr id="{id}" name="{name}"/><p:cNvSpPr txBox="1"/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm><a:off x="838200" y="{y}"/><a:ext cx="10515600" cy="{cy}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr><p:txBody><a:bodyPr wrap="square"/><a:lstStyle/><a:p><a:r><a:rPr lang="en-US" sz="{size}"><a:latin typeface="Calibri"/></a:rPr><a:t>{text}</a:t></a:r></a:p></p:txBody></p:sp>"#
        )
    };
    let slide = |title: &str, body: &str, hidden: bool| {
        format!(
            r#"{XML_DECL}<p:sld {NS}{show}><p:cSld><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/>{t}{b}</p:spTree></p:cSld></p:sld>"#,
            show = if hidden { r#" show="0""# } else { "" },
            t = text_box(2, "Title", 457_200, 1_325_563, 4000, title),
            b = text_box(3, "Text", 2_057_400, 3_000_000, 2400, body),
        )
    };
    let presentation = format!(
        r#"{XML_DECL}<p:presentation {NS}><p:sldIdLst><p:sldId id="256" r:id="rId1"/><p:sldId id="257" r:id="rId2"/></p:sldIdLst><p:sldSz cx="12192000" cy="6858000"/><p:notesSz cx="6858000" cy="9144000"/></p:presentation>"#
    );
    let pml = "application/vnd.openxmlformats-officedocument.presentationml";
    package(vec![
        (
            "[Content_Types].xml",
            content_types(
                &[],
                &[
                    (
                        "ppt/presentation.xml",
                        format!("{pml}.presentation.main+xml"),
                    ),
                    ("ppt/slides/slide1.xml", format!("{pml}.slide+xml")),
                    ("ppt/slides/slide2.xml", format!("{pml}.slide+xml")),
                ],
            ),
        ),
        (
            "_rels/.rels",
            relationships(&[("rId1", "officeDocument", "ppt/presentation.xml")]),
        ),
        ("ppt/presentation.xml", presentation.into_bytes()),
        (
            "ppt/_rels/presentation.xml.rels",
            relationships(&[
                ("rId1", "slide", "slides/slide1.xml"),
                ("rId2", "slide", "slides/slide2.xml"),
            ]),
        ),
        (
            "ppt/slides/slide1.xml",
            slide("Roadmap", "Ship the Office converter", false).into_bytes(),
        ),
        (
            "ppt/slides/slide2.xml",
            slide("Next steps", "OpenDocument formats", hide_second).into_bytes(),
        ),
    ])
}

/// What a legacy .doc (or .xls, .ppt) starts with: an OLE2 compound file
/// header, then nothing usable.
pub fn legacy_doc() -> Vec<u8> {
    let mut v = vec![0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];
    v.resize(1024, 0);
    v
}
