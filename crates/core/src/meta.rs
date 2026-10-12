//! Document information: read, edit and strip.

use crate::error::Result;
use crate::pdf;
use lopdf::{Dictionary, Document, Object, StringFormat};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Decode a PDF text string: UTF-16BE with BOM, UTF-8 with BOM, else PDFDocEncoding
/// (treated as Latin-1, which matches it for printable characters).
pub fn decode_text(obj: &Object) -> Option<String> {
    let Object::String(b, _) = obj else {
        return None;
    };
    if b.starts_with(&[0xFE, 0xFF]) {
        let units: Vec<u16> = b[2..]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|c| u16::from_be_bytes(*c))
            .collect();
        return Some(String::from_utf16_lossy(&units));
    }
    if b.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return Some(String::from_utf8_lossy(&b[3..]).into_owned());
    }
    Some(b.iter().map(|&c| c as char).collect())
}

/// Encode a text string: plain ASCII stays literal, anything else is UTF-16BE.
pub fn encode_text(s: &str) -> Object {
    if s.is_ascii() {
        return Object::String(s.as_bytes().to_vec(), StringFormat::Literal);
    }
    let mut b = vec![0xFE, 0xFF];
    for u in s.encode_utf16() {
        b.extend(u.to_be_bytes());
    }
    Object::String(b, StringFormat::Hexadecimal)
}

pub const FIELDS: [&str; 8] = [
    "Title",
    "Author",
    "Subject",
    "Keywords",
    "Creator",
    "Producer",
    "CreationDate",
    "ModDate",
];

#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Metadata {
    pub info: BTreeMap<String, String>,
    pub has_xmp: bool,
}

pub fn info_dict(doc: &Document) -> Option<&Dictionary> {
    let o = doc.trailer.get(b"Info").ok()?;
    pdf::resolve(doc, o).as_dict().ok()
}

pub fn read(doc: &Document) -> Metadata {
    let mut info = BTreeMap::new();
    if let Some(d) = info_dict(doc) {
        for (k, v) in d.iter() {
            if let Some(s) = decode_text(pdf::resolve(doc, v)) {
                info.insert(String::from_utf8_lossy(k).into_owned(), s);
            }
        }
    }
    let has_xmp = doc.catalog().map(|c| c.has(b"Metadata")).unwrap_or(false);
    Metadata { info, has_xmp }
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct MetaOptions {
    /// Fields to set; an empty value deletes the field.
    pub set: BTreeMap<String, String>,
    /// Remove the Info dictionary and every XMP packet first.
    pub clear: bool,
    pub password: Option<String>,
}

pub fn edit(input: &[u8], o: &MetaOptions) -> Result<Vec<u8>> {
    let mut doc = pdf::load(input, o.password.as_deref())?;
    if o.clear {
        strip(&mut doc);
    }
    if !o.set.is_empty() {
        let mut info = info_dict(&doc).cloned().unwrap_or_default();
        for (k, v) in &o.set {
            if !FIELDS.contains(&k.as_str()) && !k.chars().all(|c| c.is_ascii_alphanumeric()) {
                return Err(crate::Error::option(format!("bad metadata key '{k}'")));
            }
            if v.is_empty() {
                info.remove(k.as_bytes());
            } else {
                info.set(k.as_bytes(), encode_text(v));
            }
        }
        let id = doc.add_object(info);
        doc.trailer.set("Info", id);
        // XMP would contradict the edited fields; readers prefer XMP.
        if let Ok(cat) = doc.catalog_mut() {
            cat.remove(b"Metadata");
        }
    }
    pdf::save(&mut doc)
}

/// Remove the Info dictionary and all XMP packets.
pub fn strip(doc: &mut Document) {
    doc.trailer.remove(b"Info");
    for obj in doc.objects.values_mut() {
        match obj {
            Object::Dictionary(d) => {
                d.remove(b"Metadata");
                d.remove(b"PieceInfo");
            }
            Object::Stream(s) => {
                s.dict.remove(b"Metadata");
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures;

    #[test]
    fn text_roundtrip() {
        for s in ["plain", "Éléphant €", "日本"] {
            assert_eq!(decode_text(&encode_text(s)).unwrap(), s);
        }
    }

    #[test]
    fn set_and_clear() {
        let mut set = BTreeMap::new();
        set.insert("Title".into(), "Rapport annuel é".into());
        set.insert("Author".into(), String::new());
        let out = edit(
            &fixtures::text_pdf(1),
            &MetaOptions {
                set,
                ..Default::default()
            },
        )
        .unwrap();
        let m = read(&pdf::load(&out, None).unwrap());
        assert_eq!(m.info.get("Title").unwrap(), "Rapport annuel é");
        assert!(!m.info.contains_key("Author"));
        crate::testutil::qpdf_check(&out);
        let out = edit(
            &out,
            &MetaOptions {
                clear: true,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(read(&pdf::load(&out, None).unwrap()).info.is_empty());
    }
}
