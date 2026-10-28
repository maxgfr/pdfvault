//! PDF/A-2b (ISO 19005-2, level B), the archival profile: everything a
//! reader needs is inside the file and nothing in it runs or points outside.
//!
//! The conversion takes out what the profile forbids (actions and
//! JavaScript, attachments, multimedia annotations, XFA, transfer functions,
//! LZW, external streams), embeds the fonts it can, adds an sRGB output
//! intent (device CMYK colour gets a CMYK profile of its own) and writes XMP
//! metadata matching the Info dictionary. What it cannot fix (a font that is
//! not embedded and has no stand-in, an annotation without an appearance) is
//! listed in [`PdfaReport::blockers`]; the file then does not claim PDF/A in
//! its XMP.
//!
//! Standard 14 fonts that are not embedded (Helvetica, Times, Courier, and
//! Arial, Times New Roman, Courier New) are embedded as the Liberation font
//! of the same metrics, given as attachments the way the editor gets them.

use crate::error::{Error, Result};
use crate::meta;
use crate::pdf::{self, resolve};
use crate::pdffont::{Family, Font, Kind};
use lopdf::{Dictionary, Document, Object, ObjectId, Stream, StringFormat, dictionary};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// sRGB IEC 61966-2.1, version 2, 456 bytes: `sRGB-v2-micro.icc` from
/// github.com/saucecontrol/Compact-ICC-Profiles (CC0 1.0), SHA-256
/// 0a8a33aea66a6f154a5642ebe168ef287e73265d9f7b51c42a45e6eedbacda7a.
const SRGB: &[u8] = include_bytes!("../assets/sRGB-v2-micro.icc");

/// CMYK for device CMYK colour, a press profile compatible with CGATS
/// TR 001 (SWOP), version 2, 8464 bytes: `CGATS001Compat-v2-micro.icc` from
/// the same collection (CC0 1.0), SHA-256
/// 73e1ba37d2bad5bab2a964f40a9eed96209666efc067c3322626214bbef234a0.
const CMYK: &[u8] = include_bytes!("../assets/CGATS001Compat-v2-micro.icc");

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct PdfaOptions {
    pub password: Option<String>,
    /// Bundled fonts ("sans-regular"…) by attachment index; the engine fills
    /// this in from [`plan`].
    pub bundled: BTreeMap<String, usize>,
}

/// A font that was not embedded and now is.
#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FontFix {
    pub font: String,
    pub embedded: String,
}

#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PdfaReport {
    pub pages: usize,
    /// Nothing in [`Self::blockers`]: the file declares itself PDF/A-2b.
    pub conformant: bool,
    /// What was taken out, as "kind:count".
    pub removed: Vec<String>,
    pub fonts: Vec<FontFix>,
    /// What still keeps the file from being PDF/A-2b.
    pub blockers: Vec<String>,
}

/// The bundled font a non-embedded standard font is replaced with.
fn stand_in(font: &Font) -> Option<String> {
    if font.embedded || font.kind != Kind::Simple {
        return None;
    }
    // Only the metrics Liberation matches: "TimesNewRomanPS-BoldItalicMT"
    // comes down to "times", but "Arial Narrow" or "HelveticaNeue" do not.
    let mut n: String = font
        .base
        .to_ascii_lowercase()
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .collect();
    for word in [
        "bold", "italic", "oblique", "regular", "roman", "psmt", "ps", "mt", "new",
    ] {
        n = n.replace(word, "");
    }
    let family = match n.as_str() {
        "helvetica" | "arial" => Family::Sans,
        "times" => Family::Serif,
        "courier" => Family::Mono,
        _ => return None,
    };
    Some(crate::textedit::bundled_key(family, font.bold, font.italic))
}

/// Font dictionaries stored as objects (CID descendants are read through
/// their Type 0 parent).
fn font_ids(doc: &Document) -> Vec<ObjectId> {
    doc.objects
        .iter()
        .filter_map(|(id, o)| {
            let d = o.as_dict().ok()?;
            let sub = d.get(b"Subtype").and_then(Object::as_name).ok();
            (d.get_type().ok() == Some(b"Font")
                && !matches!(sub, Some(b"CIDFontType0" | b"CIDFontType2")))
            .then_some(*id)
        })
        .collect()
}

/// The bundled fonts [`convert`] will need, so only those are loaded.
pub fn plan(input: &[u8], password: Option<&str>) -> Result<Vec<String>> {
    let mut doc = pdf::load(input, password)?;
    lift_fonts(&mut doc);
    let keys: BTreeSet<String> = font_ids(&doc)
        .into_iter()
        .filter_map(|id| stand_in(&Font::load(&doc, doc.get_dictionary(id).ok()?)))
        .collect();
    Ok(keys.into_iter().collect())
}

pub fn convert(inputs: &[Vec<u8>], o: &PdfaOptions) -> Result<(Vec<u8>, PdfaReport)> {
    let (input, files) = inputs
        .split_first()
        .ok_or_else(|| Error::option("pdfa needs the PDF"))?;
    let mut doc = pdf::load(input, o.password.as_deref())?;
    let mut removed: BTreeMap<&str, usize> = BTreeMap::new();
    let mut blockers = vec![];
    let cat = doc.trailer.get(b"Root").and_then(Object::as_reference)?;

    lift_fonts(&mut doc);
    clean_catalog(&mut doc, cat, &mut removed);
    clean_annotations(&mut doc, &mut removed, &mut blockers);
    clean_dicts(&mut doc, &mut removed);
    doc.prune_objects();
    clean_streams(&mut doc, &mut blockers);
    let fonts = embed_fonts(&mut doc, files, &o.bundled, &mut blockers);
    let components = output_intent(&mut doc, cat)?;
    fix_colours(&mut doc, components, &mut blockers);

    let conformant = blockers.is_empty();
    write_metadata(&mut doc, cat, conformant)?;
    if !doc.trailer.get(b"ID").is_ok_and(|v| {
        v.as_array()
            .is_ok_and(|a| a.len() == 2 && a.iter().all(|s| s.as_str().is_ok()))
    }) {
        let id = Object::String(file_id(input), StringFormat::Hexadecimal);
        doc.trailer.set("ID", vec![id.clone(), id]);
    }
    // PDF/A-2 is built on PDF 1.7; the header comment needs bytes above 127.
    doc.version = "1.7".into();
    doc.binary_mark = vec![0xE2, 0xE3, 0xCF, 0xD3];
    let pages = doc.get_pages().len();
    let out = pdf::save(&mut doc)?;
    let report = PdfaReport {
        pages,
        conformant,
        removed: removed
            .into_iter()
            .map(|(k, v)| format!("{k}:{v}"))
            .collect(),
        fonts,
        blockers,
    };
    Ok((out, report))
}

/// Sixteen bytes derived from the input: the same file always gets the same ID.
fn file_id(input: &[u8]) -> Vec<u8> {
    let hash = |seed: u64| {
        input
            .iter()
            .fold(seed, |h, &x| (h ^ x as u64).wrapping_mul(0x100000001b3))
    };
    [hash(0xcbf29ce484222325), hash(0x84222325cbf29ce4)]
        .iter()
        .flat_map(|h| h.to_be_bytes())
        .collect()
}

/// Call `f` on every dictionary of the document, nested and stream ones
/// included; the flag says whether it is a stream's.
fn each_dict(doc: &mut Document, mut f: impl FnMut(&mut Dictionary, bool)) {
    fn walk(o: &mut Object, f: &mut impl FnMut(&mut Dictionary, bool), depth: usize) {
        if depth > 64 {
            return;
        }
        let (d, stream) = match o {
            Object::Dictionary(d) => (d, false),
            Object::Stream(s) => (&mut s.dict, true),
            Object::Array(a) => {
                a.iter_mut().for_each(|v| walk(v, f, depth + 1));
                return;
            }
            _ => return,
        };
        f(d, stream);
        d.iter_mut().for_each(|(_, v)| walk(v, f, depth + 1));
    }
    for o in doc.objects.values_mut() {
        walk(o, &mut f, 0);
    }
}

/// Give font dictionaries written inline in a /Font resource their own
/// object, so the font pass sees every font.
fn lift_fonts(doc: &mut Document) {
    let mut next = doc.max_id;
    let mut lifted = vec![];
    each_dict(doc, |d, _| {
        let Ok(Object::Dictionary(fonts)) = d.get_mut(b"Font") else {
            return;
        };
        for (_, v) in fonts.iter_mut() {
            if matches!(v, Object::Dictionary(f) if f.get_type().ok() == Some(b"Font")) {
                next += 1;
                lifted.push((
                    (next, 0),
                    std::mem::replace(v, Object::Reference((next, 0))),
                ));
            }
        }
    });
    doc.max_id = next;
    doc.objects.extend(lifted);
}

/// An object's dictionary, a stream's included.
fn dict_mut(doc: &mut Document, id: ObjectId) -> Option<&mut Dictionary> {
    match doc.objects.get_mut(&id)? {
        Object::Dictionary(d) => Some(d),
        Object::Stream(s) => Some(&mut s.dict),
        _ => None,
    }
}

/// The dictionary under `key` of object `owner`, inline or its own object.
fn sub_dict<'a>(doc: &'a mut Document, owner: ObjectId, key: &[u8]) -> Option<&'a mut Dictionary> {
    match dict_mut(doc, owner)?.get(key).ok()?.as_reference() {
        Ok(id) => dict_mut(doc, id),
        Err(_) => dict_mut(doc, owner)?.get_mut(key).ok()?.as_dict_mut().ok(),
    }
}

fn clean_catalog(doc: &mut Document, cat: ObjectId, removed: &mut BTreeMap<&str, usize>) {
    if let Ok(c) = doc.get_dictionary_mut(cat) {
        // A Version above 1.7 would contradict the header.
        for k in [&b"NeedsRendering"[..], b"Requirements", b"Version"] {
            c.remove(k);
        }
    }
    if let Some(names) = sub_dict(doc, cat, b"Names") {
        for (k, kind) in [
            (&b"JavaScript"[..], "javascript"),
            (b"EmbeddedFiles", "attachments"),
            (b"AlternatePresentations", "presentations"),
        ] {
            if names.remove(k).is_some() {
                *removed.entry(kind).or_default() += 1;
            }
        }
    }
    if let Some(form) = sub_dict(doc, cat, b"AcroForm") {
        if form.remove(b"XFA").is_some() {
            *removed.entry("xfa").or_default() += 1;
        }
        form.remove(b"NeedAppearances");
    }
    // Every optional content configuration needs a name and no /AS.
    let mut n = 0;
    let mut name = |d: &mut Dictionary| {
        d.remove(b"AS");
        if !d.has(b"Name") {
            n += 1;
            let label = if n == 1 {
                "Default".into()
            } else {
                format!("Config {n}")
            };
            d.set("Name", Object::string_literal(label));
        }
    };
    let configs: Vec<Object> = match sub_dict(doc, cat, b"OCProperties") {
        Some(oc) => {
            if let Ok(Object::Dictionary(d)) = oc.get_mut(b"D") {
                name(d);
            }
            if let Ok(Object::Array(a)) = oc.get_mut(b"Configs") {
                a.iter_mut().for_each(|c| {
                    if let Object::Dictionary(d) = c {
                        name(d);
                    }
                });
            }
            let mut refs: Vec<Object> = oc.get(b"D").into_iter().cloned().collect();
            if let Ok(Object::Array(a)) = oc.get(b"Configs") {
                refs.extend(a.iter().cloned());
            }
            refs
        }
        None => vec![],
    };
    for c in configs {
        if let Ok(id) = c.as_reference()
            && let Ok(d) = doc.get_dictionary_mut(id)
        {
            name(d);
        }
    }
}

/// Annotation types ISO 32000-1 defines and PDF/A-2 keeps.
const ANNOTATIONS: &[&[u8]] = &[
    b"Text",
    b"Link",
    b"FreeText",
    b"Line",
    b"Square",
    b"Circle",
    b"Polygon",
    b"PolyLine",
    b"Highlight",
    b"Underline",
    b"Squiggly",
    b"StrikeOut",
    b"Stamp",
    b"Caret",
    b"Ink",
    b"Popup",
    b"Widget",
    b"PrinterMark",
    b"TrapNet",
    b"Watermark",
];

fn clean_annotations(
    doc: &mut Document,
    removed: &mut BTreeMap<&str, usize>,
    blockers: &mut Vec<String>,
) {
    let mut missing = 0;
    for page in pdf::page_ids(doc) {
        let Some(list) = doc
            .get_dictionary(page)
            .ok()
            .and_then(|p| p.get(b"Annots").ok())
            .and_then(|a| resolve(doc, a).as_array().ok())
            .cloned()
        else {
            continue;
        };
        let mut kept = vec![];
        for item in list {
            let Ok(a) = resolve(doc, &item).as_dict() else {
                continue;
            };
            let sub = a.get(b"Subtype").and_then(Object::as_name).unwrap_or(b"");
            if !ANNOTATIONS.contains(&sub) {
                let kind = match sub {
                    b"FileAttachment" => "attachments",
                    b"Sound" | b"Movie" | b"Screen" | b"3D" | b"RichMedia" => "multimedia",
                    _ => "annotations",
                };
                *removed.entry(kind).or_default() += 1;
                continue;
            }
            let popup = sub == b"Popup";
            let mut a = a.clone();
            // Printed, never hidden.
            if !popup {
                let f = a.get(b"F").and_then(Object::as_i64).unwrap_or(0);
                a.set("F", (f | 4) & !(1 | 2 | 32 | 256));
            }
            // Only the normal appearance.
            let ap = a
                .get(b"AP")
                .ok()
                .map(|v| resolve(doc, v))
                .and_then(|v| v.as_dict().ok());
            match ap.map(|ap| (ap.len(), ap.get(b"N").ok().cloned())) {
                Some((1, Some(_))) => {}
                Some((_, Some(n))) => a.set("AP", dictionary! { "N" => n }),
                _ => {
                    let flat = a
                        .get(b"Rect")
                        .ok()
                        .and_then(|r| resolve(doc, r).as_array().ok())
                        .map(|r| {
                            r.iter()
                                .filter_map(|v| pdf::num(resolve(doc, v)))
                                .collect::<Vec<_>>()
                        })
                        .is_some_and(|r| r.len() == 4 && (r[0] == r[2] || r[1] == r[3]));
                    if !popup && sub != b"Link" && !flat {
                        missing += 1;
                    }
                }
            }
            match item {
                Object::Reference(id) => {
                    doc.objects.insert(id, a.into());
                    kept.push(item);
                }
                _ => kept.push(a.into()),
            }
        }
        if let Ok(p) = doc.get_dictionary_mut(page) {
            p.set("Annots", kept);
        }
    }
    if missing > 0 {
        blockers.push(format!("{missing} annotation(s) without an appearance"));
    }
}

/// Whether an action is one PDF/A-2 forbids (JavaScript, launching
/// programs, sounds, movies, form resets and imports…), and its kind.
fn forbidden(d: &Dictionary) -> Option<&'static str> {
    if d.get_type().is_ok_and(|t| t != b"Action") {
        return None;
    }
    match d.get(b"S").and_then(Object::as_name).ok()? {
        b"GoTo" | b"GoToR" | b"GoToE" | b"Thread" | b"URI" | b"SubmitForm" => None,
        b"Named" => match d.get(b"N").and_then(Object::as_name).ok() {
            Some(b"NextPage" | b"PrevPage" | b"FirstPage" | b"LastPage") => None,
            _ => Some("actions"),
        },
        b"JavaScript" => Some("javascript"),
        _ => Some("actions"),
    }
}

/// Keys PDF/A-2 forbids wherever they appear, and the kind each is reported as.
fn clean_dicts(doc: &mut Document, removed: &mut BTreeMap<&str, usize>) {
    let bad: HashMap<ObjectId, &str> = doc
        .objects
        .iter()
        .filter_map(|(id, o)| Some((*id, forbidden(o.as_dict().ok()?)?)))
        .collect();
    let kind_of = |o: &Object| match o {
        Object::Dictionary(d) => forbidden(d),
        Object::Reference(id) => bad.get(id).copied(),
        _ => None,
    };
    each_dict(doc, |d, stream| {
        let mut count = |kind: &'static str| *removed.entry(kind).or_default() += 1;
        if d.remove(b"AA").is_some() {
            count("actions");
        }
        for key in [&b"A"[..], b"OpenAction", b"Next"] {
            match d.get_mut(key) {
                Ok(Object::Array(list)) => list.retain(|a| match kind_of(a) {
                    Some(kind) => {
                        count(kind);
                        false
                    }
                    None => true,
                }),
                Ok(v) => {
                    if let Some(kind) = kind_of(v) {
                        count(kind);
                        d.remove(key);
                    }
                }
                Err(_) => {}
            }
        }
        let mut drop = |d: &mut Dictionary, key: &[u8], kind: &'static str| {
            if d.remove(key).is_some() {
                count(kind);
            }
        };
        for k in [&b"TR"[..], b"HTP", b"HTO"] {
            drop(d, k, "transfer functions");
        }
        if d.get(b"TR2").and_then(Object::as_name).ok() != Some(b"Default") {
            drop(d, b"TR2", "transfer functions");
        }
        for k in [&b"Alternates"[..], b"OPI"] {
            drop(d, k, "prepress data");
        }
        if d.get(b"Subtype").and_then(Object::as_name).ok() == Some(b"Form") {
            for k in [&b"PS"[..], b"Subtype2", b"Ref"] {
                drop(d, k, "prepress data");
            }
        }
        // Associated files and presentation steps belong to PDF/A-3 and slides.
        drop(d, b"AF", "attachments");
        drop(d, b"PresSteps", "presentations");
        if d.get(b"Interpolate")
            .and_then(Object::as_bool)
            .unwrap_or(false)
        {
            d.set("Interpolate", false);
        }
        // Overprint mode 1 is not allowed with ICC CMYK, which device CMYK
        // becomes (see `default_cmyk`).
        if d.get(b"OPM").and_then(Object::as_i64).ok() == Some(1) {
            d.set("OPM", 0);
        }
        if stream {
            for k in [&b"F"[..], b"FFilter", b"FDecodeParms"] {
                drop(d, k, "external streams");
            }
        }
    });
}

/// LZW streams become Flate; PostScript XObjects cannot stay.
fn clean_streams(doc: &mut Document, blockers: &mut Vec<String>) {
    let (mut lzw, mut ps) = (0, 0);
    for s in doc
        .objects
        .values_mut()
        .filter_map(|o| o.as_stream_mut().ok())
    {
        if s.filters().is_ok_and(|f| f.contains(&&b"LZWDecode"[..])) {
            if s.decompress().is_ok() {
                let _ = s.compress();
            } else {
                lzw += 1;
            }
        }
        if s.dict.get(b"Subtype").and_then(Object::as_name).ok() == Some(b"PS") {
            ps += 1;
        }
    }
    if lzw > 0 {
        blockers.push(format!("{lzw} LZW stream(s) could not be re-encoded"));
    }
    if ps > 0 {
        blockers.push(format!("{ps} PostScript XObject(s)"));
    }
}

fn embed_fonts(
    doc: &mut Document,
    files: &[Vec<u8>],
    bundled: &BTreeMap<String, usize>,
    blockers: &mut Vec<String>,
) -> Vec<FontFix> {
    let mut fixed = vec![];
    let mut programs: HashMap<String, (ObjectId, String)> = HashMap::new();
    for id in font_ids(doc) {
        let Ok(dict) = doc.get_dictionary(id).cloned() else {
            continue;
        };
        let font = Font::load(doc, &dict);
        if font.embedded || font.kind == Kind::Type3 {
            continue;
        }
        let file = stand_in(&font).and_then(|k| Some((files.get(*bundled.get(&k)?)?, k)));
        let problem = match file.map(|(data, key)| embed(doc, id, &font, data, &key, &mut programs))
        {
            Some(Ok(name)) => {
                let fix = FontFix {
                    font: font.base.clone(),
                    embedded: name,
                };
                if !fixed.contains(&fix) {
                    fixed.push(fix);
                }
                continue;
            }
            Some(Err(e)) => format!("the font {} could not be embedded: {e}", font.base),
            None => format!("the font {} is not embedded", font.base),
        };
        // One line per font, however many copies of it the file holds.
        if !blockers.contains(&problem) {
            blockers.push(problem);
        }
    }
    fixed
}

/// Replace a non-embedded simple font with a TrueType font embedding `data`
/// whole: Widths from its glyphs, and the font's own encoding spelled out
/// over WinAnsi (or MacRoman) with Adobe glyph names, as PDF/A asks of a
/// non-symbolic TrueType font. Returns the embedded font's name.
#[cfg(feature = "typeset")]
fn embed(
    doc: &mut Document,
    id: ObjectId,
    font: &Font,
    data: &[u8],
    key: &str,
    programs: &mut HashMap<String, (ObjectId, String)>,
) -> Result<String> {
    use crate::glyphlist;
    use crate::typeset::{metrics_of, postscript_name};
    use skrifa::instance::{LocationRef, Size};
    use skrifa::{FontRef, GlyphId, MetadataProvider};

    let face = FontRef::new(data).map_err(|_| Error::option("not a TrueType font"))?;
    let m = metrics_of(&face);
    let k = 1000.0 / m.units_per_em.max(1) as f32;
    if !programs.contains_key(key) {
        let mut file = Stream::new(
            dictionary! { "Length1" => data.len() as i64 },
            data.to_vec(),
        );
        let _ = file.compress();
        let file = doc.add_object(file);
        let mut flags = 32; // non-symbolic
        if m.is_monospace {
            flags |= 1;
        }
        if key.starts_with("serif") {
            flags |= 2;
        }
        if m.italic_angle != 0.0 {
            flags |= 64;
        }
        let bb = m
            .bounds
            .map(|b| [b.x_min, b.y_min, b.x_max, b.y_max])
            .unwrap_or([0.0, m.descent, 1.0 / k, m.ascent]);
        let name = postscript_name(&face);
        let descriptor = doc.add_object(dictionary! {
            "Type" => "FontDescriptor",
            "FontName" => Object::Name(name.clone().into_bytes()),
            "Flags" => flags,
            "FontBBox" => bb.iter().map(|v| Object::Real(v * k)).collect::<Vec<_>>(),
            "ItalicAngle" => m.italic_angle,
            "Ascent" => m.ascent * k,
            "Descent" => m.descent * k,
            "CapHeight" => m.cap_height.unwrap_or(m.ascent) * k,
            "StemV" => 80,
            "FontFile2" => file,
        });
        programs.insert(key.to_string(), (descriptor, name));
    }
    let (descriptor, name) = programs[key].clone();

    let dict = doc.get_dictionary(id)?;
    let encoding = dict.get(b"Encoding").map(|e| resolve(doc, e));
    let mac = match encoding {
        Ok(Object::Name(n)) => n == b"MacRomanEncoding",
        Ok(Object::Dictionary(d)) => {
            d.get(b"BaseEncoding").and_then(Object::as_name).ok() == Some(b"MacRomanEncoding")
        }
        _ => false,
    };
    let (base, table): (&str, &[u16; 256]) = if mac {
        ("MacRomanEncoding", &glyphlist::MAC_ROMAN)
    } else {
        ("WinAnsiEncoding", &glyphlist::WIN_ANSI)
    };
    // The glyph names the font's own /Differences gives, when the Adobe
    // Glyph List knows them.
    let mut named: HashMap<u8, (&str, u16)> = HashMap::new();
    if let Ok(Object::Dictionary(d)) = encoding
        && let Ok(list) = d.get(b"Differences").and_then(Object::as_array)
    {
        let mut code = 0i64;
        for o in list {
            match resolve(doc, o) {
                Object::Integer(i) => code = *i,
                Object::Name(n) => {
                    let name = std::str::from_utf8(n).unwrap_or("");
                    if let (Ok(c), Some(u)) = (u8::try_from(code), glyphlist::unicode_of(name)) {
                        named.insert(c, (name, u));
                    }
                    code += 1;
                }
                _ => {}
            }
        }
    }
    let mut chars: Vec<u16> = table.to_vec();
    let mut differences = vec![];
    for code in 0..=255u8 {
        let glyph = named.get(&code).copied().or_else(|| {
            let mut it = font.encoding_text(code)?.chars();
            let (Some(c), None) = (it.next(), it.next()) else {
                return None;
            };
            let u = u16::try_from(c as u32).ok()?;
            Some((glyphlist::name_of(u)?, u))
        });
        if let Some((name, u)) = glyph
            && chars[code as usize] != u
        {
            differences.push(Object::Integer(code as i64));
            differences.push(Object::Name(name.as_bytes().to_vec()));
            chars[code as usize] = u;
        }
    }
    let encoding = if differences.is_empty() {
        Object::Name(base.as_bytes().to_vec())
    } else {
        dictionary! {
            "Type" => "Encoding",
            "BaseEncoding" => base,
            "Differences" => differences,
        }
        .into()
    };
    let charmap = face.charmap();
    let advances = face.glyph_metrics(Size::unscaled(), LocationRef::default());
    let widths: Vec<Object> = chars
        .iter()
        .map(|c| {
            let gid = char::from_u32(*c as u32)
                .filter(|_| *c != 0)
                .and_then(|c| charmap.map(c))
                .unwrap_or(GlyphId::NOTDEF);
            Object::Real((advances.advance_width(gid).unwrap_or(0.0) * k).round())
        })
        .collect();
    let mut new = dictionary! {
        "Type" => "Font",
        "Subtype" => "TrueType",
        "BaseFont" => Object::Name(name.clone().into_bytes()),
        "FirstChar" => 0,
        "LastChar" => 255,
        "Widths" => widths,
        "FontDescriptor" => descriptor,
        "Encoding" => encoding,
    };
    if let Ok(tu) = dict.get(b"ToUnicode") {
        new.set("ToUnicode", tu.clone());
    }
    doc.objects.insert(id, new.into());
    Ok(name)
}

#[cfg(not(feature = "typeset"))]
fn embed(
    _: &mut Document,
    _: ObjectId,
    _: &Font,
    _: &[u8],
    _: &str,
    _: &mut HashMap<String, (ObjectId, String)>,
) -> Result<String> {
    Err(Error::Unsupported("embedding fonts".into()))
}

/// Keep a PDF/A output intent the file already has, else add sRGB. Returns
/// the number of colour components of its profile.
fn output_intent(doc: &mut Document, cat: ObjectId) -> Result<i64> {
    let existing = doc
        .get_dictionary(cat)?
        .get(b"OutputIntents")
        .ok()
        .and_then(|v| resolve(doc, v).as_array().ok())
        .into_iter()
        .flatten()
        .find_map(|i| {
            let d = resolve(doc, i).as_dict().ok()?;
            if d.get(b"S").and_then(Object::as_name).ok() != Some(b"GTS_PDFA1") {
                return None;
            }
            let profile = resolve(doc, d.get(b"DestOutputProfile").ok()?)
                .as_stream()
                .ok()?;
            Some((
                i.clone(),
                profile.dict.get(b"N").and_then(Object::as_i64).ok()?,
            ))
        });
    let (intent, n) = match existing {
        Some(found) => found,
        None => {
            let mut profile = Stream::new(dictionary! { "N" => 3 }, SRGB.to_vec());
            let _ = profile.compress();
            let profile = doc.add_object(profile);
            let label = Object::string_literal("sRGB IEC61966-2.1");
            let intent = doc.add_object(dictionary! {
                "Type" => "OutputIntent",
                "S" => "GTS_PDFA1",
                "OutputConditionIdentifier" => label.clone(),
                "Info" => label,
                "RegistryName" => Object::string_literal("http://www.color.org"),
                "DestOutputProfile" => profile,
            });
            (Object::Reference(intent), 3)
        }
    };
    // Several intents must share one profile: keep only this one.
    doc.get_dictionary_mut(cat)?
        .set("OutputIntents", vec![intent]);
    Ok(n)
}

/// Device colour the output intent does not describe (gray goes with
/// either): CMYK under an RGB intent is given a CMYK profile as its
/// DefaultCMYK; RGB under a CMYK intent the file came with is a blocker.
fn fix_colours(doc: &mut Document, components: i64, blockers: &mut Vec<String>) {
    match components {
        3 if device_colour(doc, true).0 => default_cmyk(doc),
        4 if device_colour(doc, false) == (true, false) => {
            blockers.push("device RGB colour under a CMYK output intent".into())
        }
        _ => {}
    }
}

/// Every resource dictionary content is drawn with (pages, forms, Type 3
/// fonts) gets the CMYK profile as DefaultCMYK, unless it has one.
fn default_cmyk(doc: &mut Document) {
    let mut profile = Stream::new(dictionary! { "N" => 4 }, CMYK.to_vec());
    let _ = profile.compress();
    let space = Object::Array(vec!["ICCBased".into(), doc.add_object(profile).into()]);
    let mut holders = pdf::page_ids(doc);
    for &p in &holders {
        pdf::materialize(doc, p);
    }
    holders.extend(doc.objects.iter().filter_map(|(id, o)| {
        let d = match o {
            Object::Stream(s) => &s.dict,
            Object::Dictionary(d) => d,
            _ => return None,
        };
        let kind = d.get(b"Subtype").and_then(Object::as_name).ok();
        (d.has(b"Resources") && matches!(kind, Some(b"Form" | b"Type3"))).then_some(*id)
    }));
    for h in holders {
        let Some(res) = sub_dict(doc, h, b"Resources") else {
            continue;
        };
        let shared = res.get(b"ColorSpace").and_then(Object::as_reference).ok();
        let spaces = match shared {
            Some(id) => dict_mut(doc, id),
            None => {
                if !res.has(b"ColorSpace") {
                    res.set("ColorSpace", Dictionary::new());
                }
                res.get_mut(b"ColorSpace")
                    .ok()
                    .and_then(|c| c.as_dict_mut().ok())
            }
        };
        if let Some(s) = spaces
            && !s.has(b"DefaultCMYK")
        {
            s.set("DefaultCMYK", space.clone());
        }
    }
}

/// Whether device CMYK (or RGB) colour is used, and whether a DefaultCMYK
/// (or DefaultRGB) space is anywhere to stand in for it.
fn device_colour(doc: &Document, cmyk: bool) -> (bool, bool) {
    let (short, fill) = if cmyk { ("CMYK", "k") } else { ("RGB", "rg") };
    let space = format!("Device{short}").into_bytes();
    let default = format!("Default{short}").into_bytes();
    let stroke = fill.to_uppercase();
    // (the space is named, a Default space stands in for it)
    fn names(o: &Object, space: &[u8], default: &[u8], depth: usize, out: &mut (bool, bool)) {
        let d = match o {
            Object::Name(n) => return out.0 |= n == space,
            Object::Array(a) if depth < 64 => {
                return a
                    .iter()
                    .for_each(|v| names(v, space, default, depth + 1, out));
            }
            Object::Dictionary(d) => d,
            Object::Stream(s) => &s.dict,
            _ => return,
        };
        out.1 |= d.has(default);
        for (k, v) in d.iter() {
            // An ICC profile's /Alternate is never painted with.
            if k != b"Alternate" && depth < 64 {
                names(v, space, default, depth + 1, out);
            }
        }
    }
    let mut found = (false, false);
    for o in doc.objects.values() {
        names(o, &space, &default, 0, &mut found);
    }
    let (mut used, covered) = found;
    let mut contents: Vec<Vec<u8>> = pdf::page_ids(doc)
        .into_iter()
        .filter_map(|p| crate::interp::page_content(doc, p))
        .collect();
    contents.extend(doc.objects.values().filter_map(|o| {
        let s = o.as_stream().ok()?;
        (s.dict.get(b"Subtype").and_then(Object::as_name).ok() == Some(b"Form")).then(|| {
            s.decompressed_content()
                .unwrap_or_else(|_| s.content.clone())
        })
    }));
    for c in contents {
        for op in crate::content::parse(&c).unwrap_or_default() {
            let named = |o: &Object| {
                o.as_name()
                    .is_ok_and(|n| n == space || n == short.as_bytes())
            };
            used |= op.is(fill)
                || op.is(&stroke)
                || (matches!(op.name.as_slice(), b"cs" | b"CS") && op.args.first().is_some_and(named))
                // An inline image: /CS /RGB or /ColorSpace /DeviceRGB.
                || (op.is("BI")
                    && op.args.first().and_then(|d| d.as_dict().ok()).is_some_and(|d| {
                        [&b"CS"[..], b"ColorSpace"]
                            .iter()
                            .any(|k| d.get(k).is_ok_and(named))
                    }));
        }
    }
    (used, covered)
}

/// A PDF date ("D:20240131120000+01'00'", trailing parts optional) as the
/// Info dictionary and XMP spell the same instant.
fn date(s: &str) -> Option<(String, String)> {
    let s = s.trim().strip_prefix("D:").unwrap_or(s.trim());
    let digits = s.bytes().take_while(u8::is_ascii_digit).count();
    if digits < 4 || digits % 2 != 0 || digits > 14 {
        return None;
    }
    let field = |i: usize, default: u32| -> u32 {
        s.get(i..i + 2)
            .filter(|_| i + 2 <= digits)
            .and_then(|v| v.parse().ok())
            .unwrap_or(default)
    };
    let year: u32 = s[..4].parse().ok()?;
    let (mo, d, h, mi, sec) = (
        field(4, 1),
        field(6, 1),
        field(8, 0),
        field(10, 0),
        field(12, 0),
    );
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) || h > 23 || mi > 59 || sec > 59 {
        return None;
    }
    let tz = &s[digits..];
    let (pdf_tz, xmp_tz) = match tz.chars().next() {
        None | Some('Z') => ("Z".to_string(), "Z".to_string()),
        Some(sign @ ('+' | '-')) => {
            // "+01'00'", "+01'00", "+0100" and "+01" all occur.
            let hm: String = tz[1..].chars().filter(|&c| c != '\'').collect();
            if !(hm.len() == 2 || hm.len() == 4) || !hm.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            let th: u32 = hm[..2].parse().ok()?;
            let tm: u32 = hm
                .get(2..)
                .filter(|m| !m.is_empty())
                .map_or(Some(0), |m| m.parse().ok())?;
            if th > 23 || tm > 59 {
                return None;
            }
            (
                format!("{sign}{th:02}'{tm:02}'"),
                format!("{sign}{th:02}:{tm:02}"),
            )
        }
        _ => return None,
    };
    Some((
        format!("D:{year:04}{mo:02}{d:02}{h:02}{mi:02}{sec:02}{pdf_tz}"),
        format!("{year:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{sec:02}{xmp_tz}"),
    ))
}

fn xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Replace every XMP packet with one built from the Info dictionary, and
/// write the Info fields back the way the XMP spells them so they agree.
/// `claim` adds the PDF/A identification.
fn write_metadata(doc: &mut Document, cat: ObjectId, claim: bool) -> Result<()> {
    let mut info = meta::info_dict(doc).cloned().unwrap_or_default();
    meta::strip(doc);
    let mut props = String::new();
    if claim {
        props.push_str("<pdfaid:part>2</pdfaid:part><pdfaid:conformance>B</pdfaid:conformance>\n");
    }
    // The packet is UTF-8 XML: control characters cannot go in either.
    let text = |doc: &Document, info: &Dictionary, k: &[u8]| -> Option<String> {
        let v = meta::decode_text(resolve(doc, info.get(k).ok()?))?;
        let v: String = v.chars().filter(|c| !c.is_control()).collect();
        let v = v.trim().to_string();
        (!v.is_empty()).then_some(v)
    };
    for (key, element) in [
        ("Title", "dc:title"),
        ("Author", "dc:creator"),
        ("Subject", "dc:description"),
        ("Keywords", "pdf:Keywords"),
        ("Creator", "xmp:CreatorTool"),
        ("Producer", "pdf:Producer"),
        ("CreationDate", "xmp:CreateDate"),
        ("ModDate", "xmp:ModifyDate"),
    ] {
        let v = text(doc, &info, key.as_bytes());
        info.remove(key.as_bytes());
        let Some(v) = v else { continue };
        let value = if key.ends_with("Date") {
            let Some((pdf_date, iso)) = date(&v) else {
                continue;
            };
            info.set(key, Object::string_literal(pdf_date));
            iso
        } else {
            info.set(key, meta::encode_text(&v));
            xml(&v)
        };
        let inner = match element {
            "dc:title" | "dc:description" => {
                format!("<rdf:Alt><rdf:li xml:lang=\"x-default\">{value}</rdf:li></rdf:Alt>")
            }
            "dc:creator" => format!("<rdf:Seq><rdf:li>{value}</rdf:li></rdf:Seq>"),
            _ => value,
        };
        props.push_str(&format!("<{element}>{inner}</{element}>\n"));
    }
    info.remove(b"Trapped");
    let info = doc.add_object(info);
    doc.trailer.set("Info", info);
    let packet = format!(
        "<?xpacket begin=\"\u{FEFF}\" id=\"W5M0MpCehiHzreSzNTczkc9d\"?>\n\
         <x:xmpmeta xmlns:x=\"adobe:ns:meta/\">\n\
         <rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\n\
         <rdf:Description rdf:about=\"\" \
         xmlns:pdfaid=\"http://www.aiim.org/pdfa/ns/id/\" \
         xmlns:dc=\"http://purl.org/dc/elements/1.1/\" \
         xmlns:xmp=\"http://ns.adobe.com/xap/1.0/\" \
         xmlns:pdf=\"http://ns.adobe.com/pdf/1.3/\">\n\
         {props}</rdf:Description>\n</rdf:RDF>\n</x:xmpmeta>\n<?xpacket end=\"w\"?>"
    );
    // Uncompressed: PDF/A forbids filters on the metadata stream.
    let xmp = doc.add_object(Stream::new(
        dictionary! { "Type" => "Metadata", "Subtype" => "XML" },
        packet.into_bytes(),
    ));
    doc.get_dictionary_mut(cat)?.set("Metadata", xmp);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures;
    use crate::testutil::qpdf_check;
    use lopdf::content::Operation;

    /// Convert with the bundled fonts `plan` asks for, as the engine does.
    #[cfg(feature = "typeset")]
    fn convert_with_fonts(pdf: Vec<u8>) -> (Vec<u8>, PdfaReport) {
        let mut inputs = vec![pdf];
        let mut o = PdfaOptions::default();
        for key in plan(&inputs[0], None).unwrap() {
            let (fam, style) = key.split_once('-').unwrap();
            let cap = |s: &str| s[..1].to_uppercase() + &s[1..];
            let style = if style == "bolditalic" {
                "BoldItalic".into()
            } else {
                cap(style)
            };
            o.bundled.insert(key.clone(), inputs.len() - 1);
            inputs.push(fixtures::bundled_font(&format!(
                "Liberation{}-{style}.ttf",
                cap(fam)
            )));
        }
        convert(&inputs, &o).unwrap()
    }

    fn xmp(doc: &Document) -> String {
        let cat = doc.catalog().unwrap();
        let id = cat.get(b"Metadata").unwrap().as_reference().unwrap();
        let s = doc.get_object(id).unwrap().as_stream().unwrap();
        assert!(!s.dict.has(b"Filter"), "the XMP packet is not compressed");
        String::from_utf8(s.content.clone()).unwrap()
    }

    #[cfg(feature = "typeset")]
    #[test]
    fn makes_a_text_pdf_archival() {
        // A watermark draws with a translucent Helvetica-Bold written inline in each page.
        let input = crate::stamp::watermark(
            &fixtures::text_pdf(2),
            &crate::stamp::WatermarkOptions {
                text: "DRAFT".into(),
                bold: true,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(plan(&input, None).unwrap(), ["sans-bold", "sans-regular"]);
        let (out, report) = convert_with_fonts(input);
        qpdf_check(&out);
        assert!(report.conformant, "{:?}", report.blockers);
        let fix = |font: &str, embedded: &str| FontFix {
            font: font.into(),
            embedded: embedded.into(),
        };
        let mut fonts = report.fonts.clone();
        fonts.sort_by(|a, b| a.font.cmp(&b.font));
        assert_eq!(
            fonts,
            [
                fix("Helvetica", "LiberationSans"),
                fix("Helvetica-Bold", "LiberationSans-Bold")
            ]
        );
        // Header, binary comment, file ID.
        assert!(out.starts_with(b"%PDF-1.7\n%"));
        assert!(out[10..14].iter().all(|&b| b > 127));
        let doc = pdf::load(&out, None).unwrap();
        assert!(doc.trailer.get(b"ID").unwrap().as_array().unwrap().len() == 2);
        // The output intent carries the sRGB profile.
        let cat = doc.catalog().unwrap();
        let intents = cat.get(b"OutputIntents").unwrap().as_array().unwrap();
        let intent = resolve(&doc, &intents[0]).as_dict().unwrap();
        assert_eq!(intent.get(b"S").unwrap().as_name().unwrap(), b"GTS_PDFA1");
        let profile = resolve(&doc, intent.get(b"DestOutputProfile").unwrap())
            .as_stream()
            .unwrap();
        assert_eq!(profile.dict.get(b"N").unwrap().as_i64().unwrap(), 3);
        assert_eq!(profile.decompressed_content().unwrap(), SRGB);
        // XMP identifies PDF/A-2b and agrees with the Info dictionary.
        let x = xmp(&doc);
        assert!(x.contains("<pdfaid:part>2</pdfaid:part>"), "{x}");
        assert!(x.contains("<pdfaid:conformance>B</pdfaid:conformance>"));
        assert!(x.contains("<rdf:li xml:lang=\"x-default\">Fixture</rdf:li>"));
        assert_eq!(meta::read(&doc).info["Title"], "Fixture");
        // Every font is embedded, and the text reads the same.
        for id in font_ids(&doc) {
            let f = Font::load(&doc, doc.get_dictionary(id).unwrap());
            assert!(f.embedded, "{}", f.base);
        }
        let text = crate::textedit::text_runs(&doc, &[0])
            .into_iter()
            .map(|r| r.text)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(text.contains("The quick brown fox"), "{text}");
    }

    #[cfg(feature = "typeset")]
    #[test]
    fn spells_out_a_standard_encoding() {
        // Times-Bold without /Encoding uses StandardEncoding: 0xAE is "fi".
        let mut b = fixtures::Builder::new();
        let font = b.doc.add_object(dictionary! {
            "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Times-Bold",
        });
        let page = b.page(
            (300.0, 300.0),
            vec![
                Operation::new("BT", vec![]),
                Operation::new("Tf", vec!["F2".into(), 12.into()]),
                Operation::new("Tj", vec![Object::string_literal(b"\xaere".to_vec())]),
                Operation::new("ET", vec![]),
            ],
            vec![],
        );
        let res = b.doc.get_dictionary_mut(page).unwrap();
        res.get_mut(b"Resources")
            .unwrap()
            .as_dict_mut()
            .unwrap()
            .get_mut(b"Font")
            .unwrap()
            .as_dict_mut()
            .unwrap()
            .set("F2", font);
        let (out, report) = convert_with_fonts(b.finish());
        qpdf_check(&out);
        assert!(report.conformant, "{:?}", report.blockers);
        let doc = pdf::load(&out, None).unwrap();
        let dicts: Vec<&Dictionary> = font_ids(&doc)
            .into_iter()
            .map(|id| doc.get_dictionary(id).unwrap())
            .collect();
        let times = dicts
            .iter()
            .find(|d| d.get(b"BaseFont").unwrap().as_name().unwrap() == b"LiberationSerif-Bold")
            .unwrap();
        let enc = resolve(&doc, times.get(b"Encoding").unwrap())
            .as_dict()
            .unwrap();
        let diff = enc.get(b"Differences").unwrap().as_array().unwrap();
        let at = diff
            .iter()
            .position(|o| o.as_i64().ok() == Some(0xAE))
            .unwrap();
        assert_eq!(diff[at + 1].as_name().unwrap(), b"fi");
        let f = Font::load(&doc, times);
        assert!((f.width(0xAE, 1) - 0.556).abs() < 0.002);
    }

    #[test]
    fn takes_out_scripts_and_attachments() {
        let mut b = fixtures::Builder::new();
        let page = b.text_page(&["Scripted"]);
        let js = b.doc.add_object(dictionary! {
            "S" => "JavaScript", "JS" => Object::string_literal("app.alert(1)"),
        });
        let file = b.doc.add_object(Stream::new(
            dictionary! { "Type" => "EmbeddedFile" },
            b"secret".to_vec(),
        ));
        let spec = dictionary! {
            "Type" => "Filespec", "F" => Object::string_literal("a.txt"),
            "EF" => dictionary! { "F" => file },
        };
        let link = b.doc.add_object(dictionary! {
            "Type" => "Annot", "Subtype" => "Link",
            "Rect" => vec![0.into(), 0.into(), 10.into(), 10.into()],
            "A" => dictionary! { "S" => "Launch", "F" => Object::string_literal("calc.exe") },
        });
        let clip = b.doc.add_object(dictionary! {
            "Type" => "Annot", "Subtype" => "FileAttachment",
            "Rect" => vec![0.into(), 0.into(), 10.into(), 10.into()], "FS" => spec.clone(),
        });
        let uri = b.doc.add_object(dictionary! {
            "Type" => "Annot", "Subtype" => "Link", "F" => 2,
            "Rect" => vec![20.into(), 0.into(), 30.into(), 10.into()],
            "A" => dictionary! { "S" => "URI", "URI" => Object::string_literal("https://example.org") },
        });
        let p = b.doc.get_dictionary_mut(page).unwrap();
        p.set("Annots", vec![link.into(), clip.into(), uri.into()]);
        p.set("AA", dictionary! { "O" => js });
        b.catalog_set("OpenAction", js.into());
        b.catalog_set(
            "Names",
            dictionary! {
                "JavaScript" => dictionary! { "Names" => vec![Object::string_literal("a"), js.into()] },
                "EmbeddedFiles" => dictionary! { "Names" => vec![Object::string_literal("a.txt"), spec.into()] },
            }
            .into(),
        );
        b.catalog_set(
            "AcroForm",
            dictionary! { "Fields" => vec![], "NeedAppearances" => true }.into(),
        );
        let (out, report) = convert(&[b.finish()], &PdfaOptions::default()).unwrap();
        qpdf_check(&out);
        assert_eq!(
            report.removed,
            ["actions:2", "attachments:2", "javascript:2"],
            "{report:?}"
        );
        let doc = pdf::load(&out, None).unwrap();
        for o in doc.objects.values() {
            if let Ok(d) = o.as_dict() {
                let s = d.get(b"S").and_then(Object::as_name).ok();
                assert!(!matches!(s, Some(b"JavaScript" | b"Launch")), "{d:?}");
                assert!(d.get_type().ok() != Some(b"EmbeddedFile"));
            }
        }
        assert!(
            !doc.objects
                .values()
                .any(|o| o.as_stream().is_ok_and(|s| s.content == b"secret"))
        );
        let page = pdf::page_ids(&doc)[0];
        let annots = doc.get_dictionary(page).unwrap().get(b"Annots").unwrap();
        let annots = annots.as_array().unwrap();
        assert_eq!(annots.len(), 2, "the link stays, without its action");
        let kept = resolve(&doc, &annots[1]).as_dict().unwrap();
        assert_eq!(
            kept.get(b"F").unwrap().as_i64().unwrap(),
            4,
            "printed, not hidden"
        );
        assert!(kept.has(b"A"), "a URI link is allowed");
        assert!(!doc.catalog().unwrap().has(b"OpenAction"));
        let form = resolve(&doc, doc.catalog().unwrap().get(b"AcroForm").unwrap());
        assert!(!form.as_dict().unwrap().has(b"NeedAppearances"));
    }

    #[test]
    fn says_what_it_cannot_fix() {
        let mut b = fixtures::Builder::new();
        let symbol = b.doc.add_object(dictionary! {
            "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Symbol",
        });
        let page = b.page(
            (300.0, 300.0),
            vec![
                Operation::new("k", vec![0.into(), 0.into(), 0.into(), 1.into()]),
                Operation::new("BT", vec![]),
                Operation::new("Tf", vec!["F1".into(), 12.into()]),
                Operation::new("Tj", vec![Object::string_literal("a")]),
                Operation::new("ET", vec![]),
            ],
            vec![],
        );
        b.doc
            .get_dictionary_mut(page)
            .unwrap()
            .get_mut(b"Resources")
            .unwrap()
            .as_dict_mut()
            .unwrap()
            .set("Font", dictionary! { "F1" => symbol });
        let (out, report) = convert(&[b.finish()], &PdfaOptions::default()).unwrap();
        qpdf_check(&out);
        assert!(!report.conformant);
        assert_eq!(report.blockers, ["the font Symbol is not embedded"]);
        // The file does not claim what it is not.
        let doc = pdf::load(&out, None).unwrap();
        assert!(!xmp(&doc).contains("pdfaid:part"));
        // Its CMYK colour has a profile.
        let page = pdf::page_ids(&doc)[0];
        let res = pdf::inherited(&doc, page, b"Resources").unwrap();
        let spaces = resolve(&doc, res.as_dict().unwrap().get(b"ColorSpace").unwrap());
        let cmyk = spaces.as_dict().unwrap().get(b"DefaultCMYK").unwrap();
        let profile = resolve(&doc, &cmyk.as_array().unwrap()[1])
            .as_stream()
            .unwrap();
        assert_eq!(profile.decompressed_content().unwrap(), CMYK);
        // Helvetica without its Liberation stand-in stays a blocker too.
        let (_, r) = convert(&[fixtures::text_pdf(1)], &PdfaOptions::default()).unwrap();
        assert_eq!(r.blockers, ["the font Helvetica is not embedded"]);
    }

    #[test]
    fn asks_for_the_password() {
        let locked = crate::security::encrypt(
            &fixtures::text_pdf(1),
            &crate::security::EncryptOptions {
                user_password: "pw".into(),
                ..Default::default()
            },
        )
        .unwrap();
        let err = convert(std::slice::from_ref(&locked), &PdfaOptions::default()).unwrap_err();
        assert_eq!(err.code(), "password_required");
        let o = PdfaOptions {
            password: Some("pw".into()),
            ..Default::default()
        };
        let (out, _) = convert(&[locked], &o).unwrap();
        assert!(!pdf::load(&out, None).unwrap().trailer.has(b"Encrypt"));
    }

    #[test]
    fn dates() {
        assert_eq!(
            date("D:20240131120501+01'00'").unwrap(),
            (
                "D:20240131120501+01'00'".into(),
                "2024-01-31T12:05:01+01:00".into()
            )
        );
        assert_eq!(date("D:2024").unwrap().1, "2024-01-01T00:00:00Z");
        assert_eq!(
            date("D:20240131-0530").unwrap().1,
            "2024-01-31T00:00:00-05:30"
        );
        assert!(date("yesterday").is_none());
        assert!(date("D:20241331").is_none());
    }

    #[test]
    fn profiles_are_the_pinned_ones() {
        assert_eq!(SRGB.len(), 456);
        assert_eq!(&SRGB[12..20], b"mntrRGB ");
        assert_eq!(&SRGB[36..40], b"acsp");
        assert_eq!(CMYK.len(), 8464);
        assert_eq!(&CMYK[16..20], b"CMYK");
    }
}
