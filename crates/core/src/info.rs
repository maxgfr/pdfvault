//! Inspection (`info`) and repair.

use crate::error::{Error, Result};
use crate::pdf::{self, resolve};
use crate::security::{self, SecurityInfo};
use lopdf::Object;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PageInfo {
    pub width: f32,
    pub height: f32,
    pub rotation: i64,
}

#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FontInfo {
    pub name: String,
    pub subtype: String,
    pub embedded: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Info {
    pub bytes: usize,
    pub version: String,
    pub pages: usize,
    pub page_sizes: Vec<PageInfo>,
    pub security: SecurityInfo,
    pub metadata: BTreeMap<String, String>,
    pub has_xmp: bool,
    pub images: usize,
    pub image_bytes: usize,
    pub fonts: Vec<FontInfo>,
    pub form_fields: usize,
    pub javascript: bool,
    pub attachments: usize,
    pub bookmarks: usize,
    pub objects: usize,
}

pub fn info(input: &[u8], password: Option<&str>) -> Result<Info> {
    let security = security::inspect(input)?;
    let doc = match pdf::load(input, password) {
        Ok(d) => d,
        Err(Error::PasswordRequired) => {
            return Ok(Info {
                bytes: input.len(),
                version: String::new(),
                pages: 0,
                page_sizes: vec![],
                security,
                metadata: BTreeMap::new(),
                has_xmp: false,
                images: 0,
                image_bytes: 0,
                fonts: vec![],
                form_fields: 0,
                javascript: false,
                attachments: 0,
                bookmarks: 0,
                objects: 0,
            });
        }
        Err(e) => return Err(e),
    };
    let ids = pdf::page_ids(&doc);
    let page_sizes = ids
        .iter()
        .map(|id| {
            let b = pdf::page_box(&doc, *id);
            PageInfo {
                width: b[2] - b[0],
                height: b[3] - b[1],
                rotation: pdf::page_rotation(&doc, *id),
            }
        })
        .collect();
    let mut images = 0;
    let mut image_bytes = 0;
    let mut fonts: BTreeMap<String, FontInfo> = BTreeMap::new();
    let mut javascript = false;
    let mut attachments = 0;
    for obj in doc.objects.values() {
        match obj {
            Object::Stream(s)
                if s.dict.get(b"Subtype").and_then(Object::as_name).ok() == Some(b"Image") =>
            {
                images += 1;
                image_bytes += s.content.len();
            }
            Object::Dictionary(d) => {
                match d.get_type().ok() {
                    Some(b"Font") => {
                        let name = d
                            .get(b"BaseFont")
                            .and_then(Object::as_name)
                            .map(|n| String::from_utf8_lossy(n).into_owned())
                            .unwrap_or_default();
                        let subtype = d
                            .get(b"Subtype")
                            .and_then(Object::as_name)
                            .map(|n| String::from_utf8_lossy(n).into_owned())
                            .unwrap_or_default();
                        let descriptor = d
                            .get(b"FontDescriptor")
                            .ok()
                            .and_then(|f| resolve(&doc, f).as_dict().ok())
                            .or_else(|| {
                                // Type0 fonts carry the descriptor on their descendant.
                                let desc = resolve(&doc, d.get(b"DescendantFonts").ok()?)
                                    .as_array()
                                    .ok()?
                                    .first()?;
                                resolve(
                                    &doc,
                                    resolve(&doc, desc)
                                        .as_dict()
                                        .ok()?
                                        .get(b"FontDescriptor")
                                        .ok()?,
                                )
                                .as_dict()
                                .ok()
                            });
                        let embedded = descriptor
                            .map(|fd| {
                                fd.has(b"FontFile") || fd.has(b"FontFile2") || fd.has(b"FontFile3")
                            })
                            .unwrap_or(false);
                        if !name.is_empty()
                            && subtype != "CIDFontType0"
                            && subtype != "CIDFontType2"
                        {
                            fonts.entry(name.clone()).or_insert(FontInfo {
                                name,
                                subtype,
                                embedded,
                            });
                        }
                    }
                    Some(b"Filespec") => attachments += 1,
                    _ => {}
                }
                if d.get(b"S").and_then(Object::as_name).ok() == Some(b"JavaScript") {
                    javascript = true;
                }
            }
            _ => {}
        }
    }
    let catalog = doc.catalog().ok();
    let form_fields = catalog
        .and_then(|c| c.get(b"AcroForm").ok())
        .and_then(|a| resolve(&doc, a).as_dict().ok())
        .and_then(|a| a.get(b"Fields").ok())
        .and_then(|f| resolve(&doc, f).as_array().ok())
        .map(|f| f.len())
        .unwrap_or(0);
    let meta = crate::meta::read(&doc);
    let mut seen = BTreeSet::new();
    let bookmarks = count_outline(&doc, &mut seen);
    Ok(Info {
        bytes: input.len(),
        version: doc.version.clone(),
        pages: ids.len(),
        page_sizes,
        security,
        metadata: meta.info,
        has_xmp: meta.has_xmp,
        images,
        image_bytes,
        fonts: fonts.into_values().collect(),
        form_fields,
        javascript,
        attachments,
        bookmarks,
        objects: doc.objects.len(),
    })
}

fn count_outline(doc: &lopdf::Document, seen: &mut BTreeSet<(u32, u16)>) -> usize {
    let Some(root) = doc
        .catalog()
        .ok()
        .and_then(|c| c.get(b"Outlines").ok())
        .and_then(|o| resolve(doc, o).as_dict().ok())
    else {
        return 0;
    };
    let mut stack: Vec<(u32, u16)> = root
        .get(b"First")
        .and_then(Object::as_reference)
        .into_iter()
        .collect();
    let mut count = 0;
    while let Some(id) = stack.pop() {
        if !seen.insert(id) || seen.len() > 100_000 {
            continue;
        }
        count += 1;
        if let Ok(d) = doc.get_dictionary(id) {
            for k in [&b"First"[..], b"Next"] {
                if let Ok(r) = d.get(k).and_then(Object::as_reference) {
                    stack.push(r);
                }
            }
        }
    }
    count
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepairReport {
    pub pages: usize,
    pub objects: usize,
}

/// Rebuild the file from whatever objects can be recovered: a fresh
/// cross-reference table, unreachable and broken objects dropped.
pub fn repair(input: &[u8], password: Option<&str>) -> Result<(Vec<u8>, RepairReport)> {
    let mut doc = pdf::load(input, password)?;
    let pages = doc.get_pages().len();
    let out = pdf::save(&mut doc)?;
    Ok((
        out,
        RepairReport {
            pages,
            objects: doc.objects.len(),
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures;

    #[test]
    fn describes_a_file() {
        let i = info(&fixtures::photo_pdf(), None).unwrap();
        assert_eq!(i.pages, 2);
        assert_eq!(i.images, 3);
        assert_eq!(i.metadata.get("Title").map(String::as_str), Some("Fixture"));
        assert!(!i.security.encrypted);
        let i = info(&fixtures::outlined_pdf(), None).unwrap();
        assert_eq!(i.bookmarks, 3);
        assert_eq!(i.fonts.len(), 1);
        assert!(!i.fonts[0].embedded);
        let i = info(&fixtures::form_pdf(), None).unwrap();
        assert_eq!(i.form_fields, 2);
    }

    #[test]
    fn locked_file_reports_without_password() {
        let enc = crate::security::encrypt(
            &fixtures::text_pdf(1),
            &crate::security::EncryptOptions {
                user_password: "x".into(),
                ..Default::default()
            },
        )
        .unwrap();
        let i = info(&enc, None).unwrap();
        assert!(i.security.needs_password);
        assert_eq!(i.pages, 0);
        assert_eq!(info(&enc, Some("x")).unwrap().pages, 1);
    }

    #[test]
    fn repairs_broken_classic_xref() {
        let mut doc = pdf::load(&fixtures::text_pdf(2), None).unwrap();
        let mut v = pdf::save_plain(&mut doc).unwrap();
        let at = v.windows(9).rposition(|w| w == b"startxref").unwrap();
        v.truncate(at);
        v.extend(b"startxref\n12\n%%EOF");
        let (out, rep) = repair(&v, None).unwrap();
        assert_eq!(rep.pages, 2);
        crate::testutil::qpdf_check(&out);
    }

    #[test]
    fn repairs_broken_xref() {
        let mut v = fixtures::text_pdf(2);
        // Break the startxref offset; lopdf must rebuild the table by scanning.
        let at = v.windows(9).rposition(|w| w == b"startxref").unwrap();
        v.truncate(at);
        v.extend(b"startxref\n999999\n%%EOF");
        let (out, rep) = repair(&v, None).unwrap();
        assert_eq!(rep.pages, 2);
        crate::testutil::qpdf_check(&out);
        assert!(repair(b"%PDF-1.4\nnothing here", None).is_err());
    }
}
