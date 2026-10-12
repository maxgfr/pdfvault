//! Loading, saving and the small page-level helpers every tool shares.

use crate::error::{Error, Result};
use lopdf::{Dictionary, Document, Object, ObjectId, Stream};

/// Hard ceilings on untrusted input. They keep a hostile file from exhausting
/// the 4 GiB a wasm32 instance can address.
pub const MAX_INPUT_BYTES: usize = 1 << 30;
pub const MAX_PAGES: usize = 10_000;
const MAX_STREAM_BYTES: usize = 512 << 20;

/// Parse a PDF, decrypting it when it is protected. A file that opens with the
/// empty user password needs no `password`.
pub fn load(bytes: &[u8], password: Option<&str>) -> Result<Document> {
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(Error::Limit(format!(
            "{} bytes exceeds the {MAX_INPUT_BYTES}-byte limit",
            bytes.len()
        )));
    }
    if !bytes.windows(5).take(1024).any(|w| w == b"%PDF-") {
        return Err(Error::InvalidPdf("missing %PDF- header".into()));
    }
    let options = lopdf::LoadOptions {
        password: password.filter(|p| !p.is_empty()).map(str::to_owned),
        max_decompressed_size: Some(MAX_STREAM_BYTES),
        ..Default::default()
    };
    let mut doc = match Document::load_mem_with_options(bytes, options.clone()) {
        Ok(d) => d,
        // A wrong startxref is the most common damage; point it at the real table.
        Err(e) => match patch_startxref(bytes) {
            Some(fixed) => Document::load_mem_with_options(&fixed, options).map_err(|_| e)?,
            None => return Err(e.into()),
        },
    };
    if doc.is_encrypted() {
        if doc.encryption_state.is_none() {
            return Err(match password {
                Some(p) if !p.is_empty() => Error::WrongPassword,
                _ => Error::PasswordRequired,
            });
        }
        // Objects are already decrypted; drop the dictionary so a save writes clear text.
        if let Ok(id) = doc.trailer.get(b"Encrypt").and_then(Object::as_reference) {
            doc.objects.remove(&id);
        }
        doc.trailer.remove(b"Encrypt");
    }
    let pages = doc.get_pages().len();
    if pages == 0 {
        return Err(Error::InvalidPdf("the document has no pages".into()));
    }
    if pages > MAX_PAGES {
        return Err(Error::Limit(format!(
            "{pages} pages exceeds the {MAX_PAGES}-page limit"
        )));
    }
    Ok(doc)
}

/// Rewrite the trailing `startxref` to the offset of the last cross-reference
/// table or stream actually present in the file.
fn patch_startxref(bytes: &[u8]) -> Option<Vec<u8>> {
    let rfind = |needle: &[u8]| bytes.windows(needle.len()).rposition(|w| w == needle);
    let table = rfind(b"\nxref")
        .map(|p| p + 1)
        .filter(|p| bytes[*p..].starts_with(b"xref"));
    let stream = rfind(b"/Type/XRef")
        .or_else(|| rfind(b"/Type /XRef"))
        .and_then(|at| {
            // Walk back to the start of the "N G obj" line holding this dictionary.
            let head = &bytes[..at];
            let obj = head.windows(3).rposition(|w| w == b"obj")?;
            let line = head[..obj]
                .iter()
                .rposition(|&c| c == b'\n' || c == b'\r')
                .map(|p| p + 1)
                .unwrap_or(0);
            Some(line)
        });
    let offset = match (table, stream) {
        (Some(a), Some(b)) => a.max(b),
        (a, b) => a.or(b)?,
    };
    let cut = rfind(b"startxref").unwrap_or(bytes.len());
    let mut out = bytes[..cut].to_vec();
    out.extend(format!("startxref\n{offset}\n%%EOF\n").into_bytes());
    Some(out)
}

/// Serialize with object and cross-reference streams after dropping unreachable
/// objects. Deterministic: the same document always gives the same bytes.
pub fn save(doc: &mut Document) -> Result<Vec<u8>> {
    doc.prune_objects();
    doc.renumber_objects();
    fix_lengths(doc);
    if doc.version.as_str() < "1.5" {
        doc.version = "1.5".into();
    }
    let mut out = Vec::new();
    doc.save_modern(&mut out)
        .map_err(|e| Error::InvalidPdf(e.to_string()))?;
    Ok(out)
}

/// Serialize without object streams: required once a document is encrypted.
pub fn save_plain(doc: &mut Document) -> Result<Vec<u8>> {
    fix_lengths(doc);
    let mut out = Vec::new();
    doc.save_to(&mut out)
        .map_err(|e| Error::InvalidPdf(e.to_string()))?;
    Ok(out)
}

/// Point every stream's /Length at its real size. Files often store /Length as
/// an indirect object, which goes stale once a stream is rewritten.
pub fn fix_lengths(doc: &mut Document) {
    for obj in doc.objects.values_mut() {
        if let Object::Stream(s) = obj {
            let len = s.content.len() as i64;
            s.dict.set("Length", len);
        }
    }
}

/// Page object ids in document order.
pub fn page_ids(doc: &Document) -> Vec<ObjectId> {
    doc.get_pages().into_values().collect()
}

/// Resolve a page attribute, walking up /Parent for inheritable keys.
pub fn inherited(doc: &Document, page: ObjectId, key: &[u8]) -> Option<Object> {
    let mut id = page;
    for _ in 0..64 {
        let dict = doc.get_dictionary(id).ok()?;
        if let Ok(v) = dict.get(key) {
            return Some(resolve(doc, v).clone());
        }
        id = dict.get(b"Parent").and_then(Object::as_reference).ok()?;
    }
    None
}

pub fn resolve<'a>(doc: &'a Document, obj: &'a Object) -> &'a Object {
    match obj {
        Object::Reference(id) => doc.get_object(*id).unwrap_or(obj),
        _ => obj,
    }
}

pub fn num(obj: &Object) -> Option<f32> {
    match obj {
        Object::Integer(i) => Some(*i as f32),
        Object::Real(r) => Some(*r),
        _ => None,
    }
}

fn rect(doc: &Document, obj: &Object) -> Option<[f32; 4]> {
    let arr = resolve(doc, obj).as_array().ok()?;
    if arr.len() != 4 {
        return None;
    }
    let v: Vec<f32> = arr.iter().filter_map(|o| num(resolve(doc, o))).collect();
    if v.len() != 4 {
        return None;
    }
    Some([
        v[0].min(v[2]),
        v[1].min(v[3]),
        v[0].max(v[2]),
        v[1].max(v[3]),
    ])
}

/// The visible page area: CropBox clipped to MediaBox, US Letter as a last resort.
pub fn page_box(doc: &Document, page: ObjectId) -> [f32; 4] {
    let media = inherited(doc, page, b"MediaBox")
        .and_then(|o| rect(doc, &o))
        .unwrap_or([0.0, 0.0, 612.0, 792.0]);
    match inherited(doc, page, b"CropBox").and_then(|o| rect(doc, &o)) {
        Some(c) => {
            let b = [
                c[0].max(media[0]),
                c[1].max(media[1]),
                c[2].min(media[2]),
                c[3].min(media[3]),
            ];
            if b[2] > b[0] && b[3] > b[1] { b } else { media }
        }
        None => media,
    }
}

/// /Rotate normalized to 0, 90, 180 or 270.
pub fn page_rotation(doc: &Document, page: ObjectId) -> i64 {
    let r = inherited(doc, page, b"Rotate")
        .and_then(|o| o.as_i64().ok())
        .unwrap_or(0);
    ((r % 360) + 360) % 360 / 90 * 90
}

/// Copy inheritable attributes onto the page itself so it can be moved to a
/// new parent without changing how it looks.
pub fn materialize(doc: &mut Document, page: ObjectId) {
    for key in [&b"Resources"[..], b"MediaBox", b"CropBox", b"Rotate"] {
        let own = doc.get_dictionary(page).map(|d| d.has(key)).unwrap_or(true);
        if !own
            && let Some(v) = inherited(doc, page, key)
            && let Ok(d) = doc.get_dictionary_mut(page)
        {
            d.set(key, v);
        }
    }
    let has_res = doc
        .get_dictionary(page)
        .map(|d| d.has(b"Resources"))
        .unwrap_or(true);
    if !has_res && let Ok(d) = doc.get_dictionary_mut(page) {
        d.set("Resources", Dictionary::new());
    }
}

/// Return the page's /Resources as an owned dictionary held directly by the
/// page, so adding entries never leaks into pages that shared the original.
pub fn own_resources(doc: &mut Document, page: ObjectId) -> Result<()> {
    materialize(doc, page);
    let res = doc.get_dictionary(page)?.get(b"Resources")?.clone();
    let owned = match res {
        Object::Reference(id) => doc.get_dictionary(id)?.clone(),
        Object::Dictionary(d) => d,
        _ => Dictionary::new(),
    };
    // Sub-dictionaries such as /Font may also be shared references: inline them.
    let mut owned = owned;
    for key in [&b"Font"[..], b"XObject", b"ExtGState"] {
        if let Ok(Object::Reference(id)) = owned.get(key) {
            let inner = doc.get_dictionary(*id)?.clone();
            owned.set(key, inner);
        }
    }
    doc.get_dictionary_mut(page)?.set("Resources", owned);
    Ok(())
}

/// Add an entry under /Resources/<category> of a page and return its name.
pub fn add_resource(
    doc: &mut Document,
    page: ObjectId,
    category: &[u8],
    prefix: &str,
    value: Object,
) -> Result<Vec<u8>> {
    own_resources(doc, page)?;
    let page_dict = doc.get_dictionary_mut(page)?;
    let res = page_dict.get_mut(b"Resources")?.as_dict_mut()?;
    if !res.has(category) {
        res.set(category, Dictionary::new());
    }
    let cat = res.get_mut(category)?.as_dict_mut()?;
    let mut n = 0;
    let name = loop {
        let candidate = format!("{prefix}{n}").into_bytes();
        match cat.get(&candidate) {
            Ok(existing) if *existing == value => break candidate,
            Ok(_) => n += 1,
            Err(_) => {
                cat.set(candidate.clone(), value);
                break candidate;
            }
        }
    };
    Ok(name)
}

/// Wrap the existing content in q/Q and append `ops` after it, so whatever
/// graphics state the page leaves behind cannot distort what we draw.
pub fn append_content(doc: &mut Document, page: ObjectId, ops: Vec<u8>) -> Result<()> {
    let before = doc.add_object(Stream::new(Dictionary::new(), b"q\n".to_vec()));
    let mut tail = b"\nQ\n".to_vec();
    tail.extend(ops);
    let mut tail_stream = Stream::new(Dictionary::new(), tail);
    let _ = tail_stream.compress();
    let after = doc.add_object(tail_stream);
    let dict = doc.get_dictionary_mut(page)?;
    let mut contents: Vec<Object> = match dict.get(b"Contents") {
        Ok(Object::Array(a)) => a.clone(),
        Ok(o @ Object::Reference(_)) => vec![o.clone()],
        _ => vec![],
    };
    contents.insert(0, Object::Reference(before));
    contents.push(Object::Reference(after));
    dict.set("Contents", contents);
    Ok(())
}

/// Parse a 1-based page selection such as `1-3,7,10-` against `count` pages.
/// Order and duplicates are kept: `3,1,1` means exactly that.
pub fn parse_ranges(spec: &str, count: usize) -> Result<Vec<usize>> {
    let mut out = Vec::new();
    let spec = spec.trim();
    if spec.is_empty() || spec == "all" {
        return Ok((0..count).collect());
    }
    for part in spec.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        let page = |s: &str| -> Result<usize> {
            let s = s.trim();
            let n = if s == "last" || s == "z" {
                count
            } else {
                s.parse::<usize>()
                    .map_err(|_| Error::option(format!("bad page number '{s}'")))?
            };
            if n == 0 || n > count {
                return Err(Error::option(format!("page {n} is out of range 1-{count}")));
            }
            Ok(n - 1)
        };
        match part.split_once('-') {
            Some((a, b)) => {
                let a = if a.trim().is_empty() { 0 } else { page(a)? };
                let b = if b.trim().is_empty() {
                    count - 1
                } else {
                    page(b)?
                };
                if a <= b {
                    out.extend(a..=b);
                } else {
                    out.extend((b..=a).rev());
                }
            }
            None => out.push(page(part)?),
        }
    }
    if out.is_empty() {
        return Err(Error::option("the page selection is empty"));
    }
    Ok(out)
}

/// Escape a byte string for a PDF literal string `( ... )`.
pub fn pdf_string(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len() + 2);
    out.push(b'(');
    for &b in bytes {
        match b {
            b'(' | b')' | b'\\' => {
                out.push(b'\\');
                out.push(b);
            }
            b'\n' => out.extend(b"\\n"),
            b'\r' => out.extend(b"\\r"),
            _ => out.push(b),
        }
    }
    out.push(b')');
    out
}

/// Format a number for a content stream without exponent notation.
pub fn n(v: f32) -> String {
    if (v - v.round()).abs() < 1e-4 {
        format!("{}", v.round() as i64)
    } else {
        let s = format!("{v:.4}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranges() {
        assert_eq!(parse_ranges("1-3,5", 6).unwrap(), vec![0, 1, 2, 4]);
        assert_eq!(parse_ranges("4-", 6).unwrap(), vec![3, 4, 5]);
        assert_eq!(parse_ranges("-2", 6).unwrap(), vec![0, 1]);
        assert_eq!(parse_ranges("3-1", 6).unwrap(), vec![2, 1, 0]);
        assert_eq!(parse_ranges("last,1", 6).unwrap(), vec![5, 0]);
        assert_eq!(parse_ranges("", 2).unwrap(), vec![0, 1]);
        assert!(parse_ranges("0", 6).is_err());
        assert!(parse_ranges("7", 6).is_err());
        assert!(parse_ranges("x", 6).is_err());
    }

    #[test]
    fn numbers() {
        assert_eq!(n(12.0), "12");
        assert_eq!(n(0.5), "0.5");
        assert_eq!(n(-3.25), "-3.25");
    }

    #[test]
    fn strings() {
        assert_eq!(pdf_string(b"a(b)\\"), b"(a\\(b\\)\\\\)".to_vec());
    }
}
