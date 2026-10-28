//! Fonts as the content interpreter needs them: how a shown string splits
//! into character codes, how far each code advances, what text it stands for
//! and how tall the font is. Outlines are the renderer's business; positions
//! come from the widths the PDF declares (or the standard 14 metrics).

use crate::glyphlist;
use crate::pdf;
use crate::std14;
use hayro_cmap::{BfString, CMap};
use lopdf::{Dictionary, Document, Object};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Family {
    Sans,
    Serif,
    Mono,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Simple,
    Type3,
    Cid,
}

#[derive(Debug)]
enum Widths {
    /// Glyph-space widths for codes 0..=255.
    Simple(Vec<f32>),
    Cid {
        map: HashMap<u32, f32>,
        dw: f32,
        /// Vertical advances (w1y) for vertical writing.
        w1: HashMap<u32, f32>,
        dw1: f32,
    },
}

#[derive(Debug)]
pub struct Font {
    /// PostScript name without the subset tag.
    pub base: String,
    /// The family as an installed copy would be named ("Times New Roman").
    pub family_name: String,
    pub family: Family,
    pub bold: bool,
    pub italic: bool,
    pub embedded: bool,
    /// The program holds only the glyphs the document used (a `ABCDEF+` tag).
    pub subset: bool,
    pub kind: Kind,
    widths: Widths,
    simple_unicode: Vec<Option<String>>,
    to_unicode: Option<CMap>,
    cmap: Option<CMap>,
    /// Two-byte codes that are CIDs (Identity-H/V).
    identity: bool,
    vertical: bool,
    /// Glyph space to text space (0.001 except for Type 3 fonts).
    scale: f32,
    /// Above and below the baseline, in text space for a 1-point font.
    pub ascent: f32,
    pub descent: f32,
    /// Codes or widths had to be guessed: positions may be off.
    pub uncertain: bool,
}

fn strip_subset(name: &str) -> (&str, bool) {
    let b = name.as_bytes();
    if b.len() > 7 && b[6] == b'+' && b[..6].iter().all(u8::is_ascii_uppercase) {
        (&name[7..], true)
    } else {
        (name, false)
    }
}

fn guess_family(name: &str, flags: i64) -> Family {
    let n = name.to_ascii_lowercase();
    let has = |words: &[&str]| words.iter().any(|w| n.contains(w));
    if flags & 1 != 0
        || has(&[
            "courier",
            "mono",
            "consol",
            "menlo",
            "typewriter",
            "code",
            "fixed",
        ])
    {
        Family::Mono
    } else if (flags & 2 != 0 && !n.contains("sans"))
        || (!n.contains("sans")
            && has(&[
                "times",
                "serif",
                "georgia",
                "garamond",
                "roman",
                "minion",
                "cambria",
                "book",
                "palat",
                "baskerville",
                "caslon",
                "bodoni",
                "century",
                "cmr",
                "nimbusrom",
            ]))
    {
        Family::Serif
    } else {
        Family::Sans
    }
}

/// Which standard 14 metrics apply to a font name, if any.
enum Std {
    Latin(usize),
    Courier,
    Symbol,
    Zapf,
}

fn std_metrics(name: &str, bold: bool, italic: bool) -> Option<(Std, usize)> {
    let n: String = name
        .to_ascii_lowercase()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect();
    if n.starts_with("helvetica") || n.starts_with("arial") {
        Some((Std::Latin(bold as usize), 0))
    } else if n.starts_with("times") {
        Some((Std::Latin(2 + bold as usize + 2 * italic as usize), 1))
    } else if n.starts_with("courier") {
        Some((Std::Courier, 2))
    } else if n.starts_with("symbol") {
        Some((Std::Symbol, 3))
    } else if n.contains("dingbats") {
        Some((Std::Zapf, 4))
    } else {
        None
    }
}

fn latin_width(table: usize, c: char) -> Option<f32> {
    let u = u16::try_from(c as u32).ok()?;
    let i = std14::UNICODES.binary_search(&u).ok()?;
    let w = std14::LATIN[table][i];
    (w > 0).then_some(w as f32)
}

/// A base encoding as code → Unicode (0 = none).
fn base_table(name: &[u8]) -> &'static [u16; 256] {
    match name {
        b"WinAnsiEncoding" => &glyphlist::WIN_ANSI,
        b"MacRomanEncoding" => &glyphlist::MAC_ROMAN,
        _ => &glyphlist::STANDARD,
    }
}

/// The text a glyph name stands for (Adobe Glyph List, `uniXXXX`, `uXXXX`,
/// ligatures written `f_f_i`).
pub fn glyph_name_text(name: &[u8]) -> Option<String> {
    let name = name.split(|&c| c == b'.').next().unwrap_or(name);
    if name.is_empty() {
        return None;
    }
    if name.contains(&b'_') {
        let parts: Option<Vec<String>> = name.split(|&c| c == b'_').map(glyph_name_text).collect();
        return parts.map(|p| p.concat());
    }
    let hex =
        |h: &[u8]| -> Option<u32> { u32::from_str_radix(std::str::from_utf8(h).ok()?, 16).ok() };
    if let Some(h) = name.strip_prefix(b"uni")
        && h.len() >= 4
        && h.len() % 4 == 0
        && h.iter().all(u8::is_ascii_hexdigit)
    {
        let units: Option<Vec<u16>> = h.chunks(4).map(|c| hex(c).map(|v| v as u16)).collect();
        return String::from_utf16(&units?).ok();
    }
    if let Some(h) = name.strip_prefix(b"u")
        && (4..=6).contains(&h.len())
        && h.iter().all(u8::is_ascii_hexdigit)
    {
        return char::from_u32(hex(h)?).map(String::from);
    }
    glyphlist::unicode_of(std::str::from_utf8(name).ok()?)
        .and_then(|u| char::from_u32(u as u32))
        .map(String::from)
}

/// The family name an embedded font program gives itself.
#[cfg(feature = "typeset")]
fn program_family(doc: &Document, descriptor: &Dictionary) -> Option<String> {
    use skrifa::raw::types::NameId;
    let stream = [&b"FontFile2"[..], b"FontFile3"]
        .iter()
        .find_map(|k| get(doc, descriptor, k))?
        .as_stream()
        .ok()?;
    let data = stream
        .decompressed_content()
        .unwrap_or_else(|_| stream.content.clone());
    let font = harfrust::Font::new(data, 0)?;
    let names = font.names();
    let name = names
        .get(NameId::TYPOGRAPHIC_FAMILY_NAME)
        .or_else(|| names.get(NameId::FAMILY_NAME))?;
    let family: String = name.english_or_first()?.name().chars().collect();
    let family = strip_subset(family.trim()).0.to_string();
    (!family.is_empty()).then_some(family)
}

#[cfg(not(feature = "typeset"))]
fn program_family(_: &Document, _: &Dictionary) -> Option<String> {
    None
}

/// Ligatures from the Alphabetic Presentation Forms block, spelled out.
pub fn unligate(s: &str) -> String {
    if !s.chars().any(|c| ('\u{FB00}'..='\u{FB06}').contains(&c)) {
        return s.to_string();
    }
    s.chars()
        .map(|c| match c {
            '\u{FB00}' => "ff".to_string(),
            '\u{FB01}' => "fi".to_string(),
            '\u{FB02}' => "fl".to_string(),
            '\u{FB03}' => "ffi".to_string(),
            '\u{FB04}' => "ffl".to_string(),
            '\u{FB05}' => "\u{17F}t".to_string(),
            '\u{FB06}' => "st".to_string(),
            other => other.to_string(),
        })
        .collect()
}

/// A family name from a PostScript name: "TimesNewRomanPS-BoldMT" gives
/// "Times New Roman", "HelveticaNeue-Light" "Helvetica Neue".
pub fn family_of(postscript: &str) -> String {
    let stem = postscript.split(['-', ',']).next().unwrap_or(postscript);
    let mut stem = stem.to_string();
    for suffix in ["PSMT", "PS", "MT"] {
        if stem.len() > suffix.len() + 2 && stem.ends_with(suffix) {
            stem.truncate(stem.len() - suffix.len());
            break;
        }
    }
    let mut out = String::new();
    let chars: Vec<char> = stem.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        let next_lower = chars.get(i + 1).is_some_and(|n| n.is_lowercase());
        let prev = if i > 0 { Some(chars[i - 1]) } else { None };
        if i > 0
            && c.is_uppercase()
            && (prev.is_some_and(|p| p.is_lowercase())
                || next_lower && prev.is_some_and(|p| p.is_uppercase()))
        {
            out.push(' ');
        }
        out.push(c);
    }
    out
}

fn bf_text(b: BfString) -> String {
    match b {
        BfString::Char(c) => c.to_string(),
        BfString::String(s) => s,
    }
}

fn parse_cmap(doc: &Document, obj: &Object) -> Option<CMap> {
    let stream = pdf::resolve(doc, obj).as_stream().ok()?;
    let data = stream
        .decompressed_content()
        .unwrap_or_else(|_| stream.content.clone());
    CMap::parse(&data, |_| None)
}

fn get<'a>(doc: &'a Document, d: &'a Dictionary, key: &[u8]) -> Option<&'a Object> {
    d.get(key).ok().map(|o| pdf::resolve(doc, o))
}

fn num_of(doc: &Document, d: &Dictionary, key: &[u8]) -> Option<f32> {
    get(doc, d, key).and_then(pdf::num)
}

fn rect_of(doc: &Document, d: &Dictionary, key: &[u8]) -> Option<[f32; 4]> {
    let a = get(doc, d, key)?.as_array().ok()?;
    let v: Vec<f32> = a
        .iter()
        .filter_map(|o| pdf::num(pdf::resolve(doc, o)))
        .collect();
    (v.len() == 4).then(|| {
        [
            v[0].min(v[2]),
            v[1].min(v[3]),
            v[0].max(v[2]),
            v[1].max(v[3]),
        ]
    })
}

impl Font {
    pub fn load(doc: &Document, dict: &Dictionary) -> Font {
        let subtype = get(doc, dict, b"Subtype")
            .and_then(|o| o.as_name().ok())
            .unwrap_or(b"Type1")
            .to_vec();
        let raw_name = get(doc, dict, b"BaseFont")
            .and_then(|o| o.as_name().ok())
            .map(|n| String::from_utf8_lossy(n).into_owned())
            .unwrap_or_default();
        let (base, subset) = strip_subset(&raw_name);
        let base = base.to_string();

        let descendant = if subtype == b"Type0" {
            get(doc, dict, b"DescendantFonts")
                .and_then(|o| o.as_array().ok())
                .and_then(|a| a.first())
                .and_then(|o| pdf::resolve(doc, o).as_dict().ok())
        } else {
            None
        };
        let desc_dict = descendant.unwrap_or(dict);
        let descriptor = get(doc, desc_dict, b"FontDescriptor").and_then(|o| o.as_dict().ok());
        let flags = descriptor
            .and_then(|d| num_of(doc, d, b"Flags"))
            .unwrap_or(0.0) as i64;
        let weight = descriptor.and_then(|d| num_of(doc, d, b"FontWeight"));
        let italic_angle = descriptor
            .and_then(|d| num_of(doc, d, b"ItalicAngle"))
            .unwrap_or(0.0);
        let lower = base.to_ascii_lowercase();
        let bold = ["bold", "black", "heavy", "semibold", "demi"]
            .iter()
            .any(|w| lower.contains(w))
            || weight.is_some_and(|w| w >= 600.0)
            || flags & (1 << 18) != 0;
        let italic = lower.contains("italic")
            || lower.contains("oblique")
            || flags & 64 != 0
            || italic_angle.abs() > 0.5;
        let embedded = descriptor
            .is_some_and(|d| d.has(b"FontFile") || d.has(b"FontFile2") || d.has(b"FontFile3"))
            || subtype == b"Type3";
        let std = std_metrics(&base, bold, italic);

        let family_name = descriptor
            .and_then(|d| get(doc, d, b"FontFamily"))
            .and_then(|o| o.as_str().ok())
            .map(crate::forms::decode_text)
            .filter(|s| !s.trim().is_empty())
            .or_else(|| descriptor.and_then(|d| program_family(doc, d)))
            .unwrap_or_else(|| family_of(&base));
        let mut font = Font {
            family: guess_family(&base, flags),
            family_name,
            base,
            bold,
            italic,
            embedded,
            subset,
            kind: Kind::Simple,
            widths: Widths::Simple(vec![]),
            simple_unicode: vec![],
            to_unicode: dict.get(b"ToUnicode").ok().and_then(|o| parse_cmap(doc, o)),
            cmap: None,
            identity: false,
            vertical: false,
            scale: 0.001,
            ascent: 0.8,
            descent: -0.2,
            uncertain: false,
        };

        // Vertical extent: the descriptor, else the bounding box, else the
        // standard metrics, else a typical Latin font.
        let (mut asc, mut desc) = (None, None);
        if let Some(d) = descriptor {
            asc = num_of(doc, d, b"Ascent").filter(|v| *v > 0.0);
            desc = num_of(doc, d, b"Descent").filter(|v| *v < 0.0);
            if let Some(b) = rect_of(doc, d, b"FontBBox") {
                asc = asc.or((b[3] > 0.0).then_some(b[3]));
                desc = desc.or((b[1] < 0.0).then_some(b[1]));
            }
        }
        if let Some((_, v)) = &std {
            let (a, d) = std14::VERTICAL[*v];
            asc = asc.or(Some(a as f32));
            desc = desc.or(Some(d as f32));
        }

        if subtype == b"Type0" {
            font.kind = Kind::Cid;
            let enc = dict.get(b"Encoding").ok();
            font.identity = matches!(
                enc.map(|o| pdf::resolve(doc, o)),
                Some(Object::Name(n)) if n == b"Identity-H" || n == b"Identity-V"
            );
            let cmap = match enc.map(|o| pdf::resolve(doc, o)) {
                Some(Object::Name(n)) if n == b"Identity-H" => Some(CMap::identity_h()),
                Some(Object::Name(n)) if n == b"Identity-V" => Some(CMap::identity_v()),
                Some(Object::Stream(_)) => enc.and_then(|o| parse_cmap(doc, o)),
                _ => None,
            };
            if cmap.is_none() {
                font.uncertain = true;
            }
            font.vertical = matches!(
                enc.map(|o| pdf::resolve(doc, o)),
                Some(Object::Name(n)) if n.ends_with(b"-V")
            ) || cmap.as_ref().is_some_and(|c| {
                matches!(
                    c.metadata().writing_mode,
                    Some(hayro_cmap::WritingMode::Vertical)
                )
            });
            font.cmap = Some(cmap.unwrap_or_else(CMap::identity_h));
            let dw = num_of(doc, desc_dict, b"DW").unwrap_or(1000.0);
            let mut map = HashMap::new();
            if let Some(Object::Array(w)) = get(doc, desc_dict, b"W") {
                let w: Vec<&Object> = w.iter().map(|o| pdf::resolve(doc, o)).collect();
                let mut i = 0;
                while i + 1 < w.len() {
                    let Some(first) = pdf::num(w[i]) else { break };
                    let first = first as u32;
                    if let Object::Array(list) = w[i + 1] {
                        for (k, v) in list.iter().enumerate() {
                            if let Some(v) = pdf::num(pdf::resolve(doc, v)) {
                                map.insert(first + k as u32, v);
                            }
                        }
                        i += 2;
                    } else if i + 2 < w.len() {
                        let (Some(last), Some(v)) = (pdf::num(w[i + 1]), pdf::num(w[i + 2])) else {
                            break;
                        };
                        // Guard against absurd ranges in broken files.
                        for c in first..=(last as u32).min(first + 65535) {
                            map.insert(c, v);
                        }
                        i += 3;
                    } else {
                        break;
                    }
                }
            }
            let dw2 = get(doc, desc_dict, b"DW2")
                .and_then(|o| o.as_array().ok())
                .and_then(|a| a.get(1))
                .and_then(pdf::num)
                .unwrap_or(-1000.0);
            let mut w1 = HashMap::new();
            if let Some(Object::Array(w)) = get(doc, desc_dict, b"W2") {
                let w: Vec<&Object> = w.iter().map(|o| pdf::resolve(doc, o)).collect();
                let mut i = 0;
                while i + 1 < w.len() {
                    let Some(first) = pdf::num(w[i]) else { break };
                    let first = first as u32;
                    if let Object::Array(list) = w[i + 1] {
                        for (k, chunk) in list.chunks(3).enumerate() {
                            if let Some(v) = chunk.first().and_then(pdf::num) {
                                w1.insert(first + k as u32, v);
                            }
                        }
                        i += 2;
                    } else if i + 4 < w.len() {
                        let (Some(last), Some(v)) = (pdf::num(w[i + 1]), pdf::num(w[i + 2])) else {
                            break;
                        };
                        for c in first..=(last as u32).min(first + 65535) {
                            w1.insert(c, v);
                        }
                        i += 5;
                    } else {
                        break;
                    }
                }
            }
            font.widths = Widths::Cid {
                map,
                dw,
                w1,
                dw1: dw2,
            };
        } else {
            if subtype == b"Type3" {
                font.kind = Kind::Type3;
                let m: Vec<f32> = get(doc, dict, b"FontMatrix")
                    .and_then(|o| o.as_array().ok())
                    .map(|a| {
                        a.iter()
                            .filter_map(|o| pdf::num(pdf::resolve(doc, o)))
                            .collect()
                    })
                    .unwrap_or_default();
                if m.len() == 6 && m[0].abs() > 1e-9 {
                    font.scale = m[0].abs();
                    if let Some(b) = rect_of(doc, dict, b"FontBBox")
                        && b[3] > b[1]
                    {
                        let k = m[3].abs().max(1e-9) / font.scale;
                        asc = Some(b[3] * k).filter(|v| *v > 0.0);
                        desc = Some(b[1] * k).filter(|v| *v < 0.0);
                    }
                } else {
                    font.uncertain = true;
                }
            }
            font.simple_unicode = simple_unicode(doc, dict, &font, &std);
            let first = num_of(doc, dict, b"FirstChar").unwrap_or(0.0).max(0.0) as usize;
            let missing = descriptor
                .and_then(|d| num_of(doc, d, b"MissingWidth"))
                .unwrap_or(0.0);
            let mut w = vec![f32::NAN; 256];
            if let Some(Object::Array(list)) = get(doc, dict, b"Widths") {
                for (k, v) in list.iter().enumerate() {
                    if let (Some(slot), Some(v)) =
                        (w.get_mut(first + k), pdf::num(pdf::resolve(doc, v)))
                    {
                        *slot = v;
                    }
                }
            }
            for (code, slot) in w.iter_mut().enumerate() {
                if !slot.is_nan() {
                    continue;
                }
                let from_std = match &std {
                    Some((Std::Latin(t), _)) => font.simple_unicode[code]
                        .as_ref()
                        .and_then(|s| s.chars().next())
                        .and_then(|c| latin_width(*t, c)),
                    Some((Std::Courier, _)) => Some(600.0),
                    Some((Std::Symbol, _)) => Some(std14::SYMBOL[code] as f32).filter(|w| *w > 0.0),
                    Some((Std::Zapf, _)) => Some(std14::ZAPF[code] as f32).filter(|w| *w > 0.0),
                    None => None,
                };
                *slot = match from_std {
                    Some(v) => v,
                    None => {
                        if missing == 0.0 && font.kind != Kind::Type3 && std.is_none() {
                            // No width anywhere: the positions after this glyph are a guess.
                            font.uncertain |= dict.get(b"Widths").is_err();
                        }
                        missing
                    }
                };
            }
            font.widths = Widths::Simple(w);
        }

        let to_text = |v: f32| v * font.scale;
        font.ascent = asc
            .map(to_text)
            .filter(|v| (0.3..=1.5).contains(v))
            .unwrap_or(0.8);
        font.descent = desc
            .map(to_text)
            .filter(|v| (-0.8..=0.0).contains(v))
            .unwrap_or(-0.2);
        font
    }

    /// Split shown bytes into `(code, start, len)`.
    pub fn codes(&self, s: &[u8]) -> Vec<(u32, usize, usize)> {
        let Some(cmap) = &self.cmap else {
            return s
                .iter()
                .enumerate()
                .map(|(i, &b)| (b as u32, i, 1))
                .collect();
        };
        let mut out = vec![];
        let mut i = 0;
        while i < s.len() {
            let mut code = 0u32;
            let mut found = None;
            for n in 0..4.min(s.len() - i) {
                code = (code << 8) | s[i + n] as u32;
                if cmap.lookup_cid_code(code, (n + 1) as u8).is_some() {
                    found = Some((code, n + 1));
                    break;
                }
            }
            let (code, len) = found.unwrap_or((s[i] as u32, 1));
            out.push((code, i, len));
            i += len;
        }
        out
    }

    fn cid(&self, code: u32, len: usize) -> u32 {
        self.cmap
            .as_ref()
            .and_then(|c| c.lookup_cid_code(code, len as u8))
            .unwrap_or(code)
    }

    /// Horizontal width of a glyph, in text space for a 1-point font.
    pub fn width(&self, code: u32, len: usize) -> f32 {
        let w = match &self.widths {
            Widths::Simple(w) => w.get(code as usize).copied().unwrap_or(0.0),
            Widths::Cid { map, dw, .. } => map.get(&self.cid(code, len)).copied().unwrap_or(*dw),
        };
        w * self.scale
    }

    /// Advance along the writing direction (negative, downwards, when vertical).
    pub fn advance(&self, code: u32, len: usize) -> f32 {
        match &self.widths {
            Widths::Cid { w1, dw1, .. } if self.vertical => {
                w1.get(&self.cid(code, len)).copied().unwrap_or(*dw1) * self.scale
            }
            _ => self.width(code, len),
        }
    }

    pub fn vertical(&self) -> bool {
        self.vertical
    }

    /// The text a code stands for, when the font says. Ligature characters
    /// (ﬁ, ﬀ…) come out as their letters.
    pub fn text(&self, code: u32, len: usize) -> Option<String> {
        let _ = len;
        let raw = match &self.to_unicode {
            Some(tu) => tu.lookup_bf_string(code).map(bf_text),
            None => None,
        }
        .or_else(|| {
            (self.kind != Kind::Cid)
                .then(|| self.simple_unicode.get(code as usize).cloned().flatten())
                .flatten()
        })?;
        Some(unligate(&raw))
    }

    /// The text a simple font's encoding gives a code, ToUnicode aside.
    #[cfg_attr(not(feature = "typeset"), allow(dead_code))]
    pub(crate) fn encoding_text(&self, code: u8) -> Option<&str> {
        self.simple_unicode.get(code as usize)?.as_deref()
    }

    /// Word spacing (Tw) applies to single-byte code 32 only.
    pub fn is_word_space(code: u32, len: usize) -> bool {
        code == 32 && len == 1
    }

    /// Every code this font maps to text, when that covers its whole repertoire
    /// (simple fonts that are not subsets). Subsets only hold what was used.
    pub fn charset(&self) -> Option<Vec<(String, u32)>> {
        if self.kind != Kind::Simple || self.subset {
            return None;
        }
        let Widths::Simple(w) = &self.widths else {
            return None;
        };
        Some(
            (0..256u32)
                .filter(|&c| w[c as usize] > 0.0)
                .filter_map(|c| Some((self.text(c, 1)?, c)))
                .collect(),
        )
    }

    /// Every code the ToUnicode map names, with its text: for a subset, the
    /// characters the whole document uses (not just one page). Codes without
    /// an explicit width in a CID font are left out, and a map that names far
    /// more codes than a subset holds is not trusted.
    pub fn mapped_codes(&self) -> Vec<(String, u32, usize)> {
        let Some(tu) = &self.to_unicode else {
            return vec![];
        };
        let mut out = vec![];
        match &self.widths {
            Widths::Simple(w) => {
                for c in 0..256u32 {
                    if w[c as usize] > 0.0
                        && let Some(b) = tu.lookup_bf_string(c)
                    {
                        out.push((unligate(&bf_text(b)), c, 1));
                    }
                }
            }
            Widths::Cid { map, .. } if self.identity => {
                for c in 0..=0xFFFFu32 {
                    if (map.is_empty() || map.contains_key(&c))
                        && let Some(b) = tu.lookup_bf_string(c)
                    {
                        out.push((unligate(&bf_text(b)), c, 2));
                        if out.len() > 4000 {
                            return vec![];
                        }
                    }
                }
            }
            Widths::Cid { .. } => {}
        }
        out
    }

    /// Bytes for `code` as the font reads them.
    pub fn code_bytes(&self, code: u32, len: usize) -> Vec<u8> {
        code.to_be_bytes()[4 - len.clamp(1, 4)..].to_vec()
    }
}

fn simple_unicode(
    doc: &Document,
    dict: &Dictionary,
    font: &Font,
    std: &Option<(Std, usize)>,
) -> Vec<Option<String>> {
    let symbolic_std = matches!(std, Some((Std::Symbol | Std::Zapf, _)));
    let mut base: Vec<Option<String>> = vec![None; 256];
    let mut differences: Vec<(usize, Vec<u8>)> = vec![];
    let fill = |base: &mut Vec<Option<String>>, name: &[u8]| {
        for (i, &u) in base_table(name).iter().enumerate() {
            base[i] = char::from_u32(u as u32)
                .filter(|_| u != 0)
                .map(String::from);
        }
    };
    match get(doc, dict, b"Encoding") {
        Some(Object::Name(n)) => fill(&mut base, n),
        Some(Object::Dictionary(d)) => {
            match get(doc, d, b"BaseEncoding").and_then(|o| o.as_name().ok()) {
                Some(n) => fill(&mut base, n),
                None if !symbolic_std && font.kind != Kind::Type3 => {
                    fill(&mut base, b"StandardEncoding")
                }
                None => {}
            }
            if let Some(Object::Array(a)) = get(doc, d, b"Differences") {
                let mut code = 0usize;
                for o in a {
                    match pdf::resolve(doc, o) {
                        Object::Integer(i) => code = (*i).clamp(0, 255) as usize,
                        Object::Name(n) => {
                            if code < 256 {
                                differences.push((code, n.clone()));
                            }
                            code += 1;
                        }
                        _ => {}
                    }
                }
            }
        }
        _ if !symbolic_std && font.kind != Kind::Type3 => fill(&mut base, b"StandardEncoding"),
        _ => {}
    }
    for (code, name) in differences {
        base[code] = glyph_name_text(&name);
    }
    base
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::{Object, dictionary};

    #[test]
    fn standard_fonts_without_widths() {
        let doc = Document::new();
        let d = dictionary! {
            "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Times-Bold",
            "Encoding" => "WinAnsiEncoding",
        };
        let f = Font::load(&doc, &d);
        assert!(!f.uncertain && f.bold && f.family == Family::Serif);
        // Times-Bold: A = 722, é (0xE9) = 444.
        assert!((f.width(65, 1) - 0.722).abs() < 1e-6);
        assert!((f.width(0xE9, 1) - 0.444).abs() < 1e-6);
        assert_eq!(f.text(0xE9, 1).as_deref(), Some("é"));
        assert!((f.ascent - 0.683).abs() < 1e-6);
        let c = Font::load(
            &doc,
            &dictionary! { "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Courier" },
        );
        assert_eq!(c.family, Family::Mono);
        assert!((c.width(b'i' as u32, 1) - 0.6).abs() < 1e-6);
    }

    #[test]
    fn differences_and_glyph_names() {
        let doc = Document::new();
        let d = dictionary! {
            "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "ABCDEF+Foo-Italic",
            "FirstChar" => 1, "Widths" => vec![500.into(), 250.into()],
            "Encoding" => dictionary! {
                "Type" => "Encoding",
                "Differences" => vec![1.into(), Object::Name(b"eacute".to_vec()), Object::Name(b"f_i".to_vec())],
            },
        };
        let f = Font::load(&doc, &d);
        assert!(f.subset && f.italic && f.base == "Foo-Italic");
        assert_eq!(f.text(1, 1).as_deref(), Some("é"));
        assert_eq!(f.text(2, 1).as_deref(), Some("fi"));
        assert!((f.width(2, 1) - 0.25).abs() < 1e-6);
        assert_eq!(glyph_name_text(b"uni00E9").as_deref(), Some("é"));
        assert_eq!(glyph_name_text(b"u1F600").as_deref(), Some("😀"));
        assert_eq!(glyph_name_text(b"A.sc").as_deref(), Some("A"));
        assert_eq!(glyph_name_text(b"g123"), None);
        assert!(f.charset().is_none(), "a subset has no full charset");
    }

    #[test]
    fn family_names_and_ligatures() {
        assert_eq!(family_of("TimesNewRomanPS-BoldMT"), "Times New Roman");
        assert_eq!(family_of("ArialMT"), "Arial");
        assert_eq!(family_of("HelveticaNeue-Light"), "Helvetica Neue");
        assert_eq!(family_of("Georgia-Bold"), "Georgia");
        assert_eq!(family_of("Avenir-Book"), "Avenir");
        assert_eq!(family_of("Arial,Bold"), "Arial");
        assert_eq!(unligate("chi\u{FB00}re \u{FB01}nale"), "chiffre finale");
    }

    #[test]
    fn cid_font_widths_and_codes() {
        let mut doc = Document::new();
        let tu = doc.add_object(lopdf::Stream::new(
            Dictionary::new(),
            b"/CIDInit /ProcSet findresource begin 12 dict begin begincmap\n\
              1 begincodespacerange <0000> <FFFF> endcodespacerange\n\
              2 beginbfchar <0003> <0041> <0004> <00E9> endbfchar\n\
              endcmap CMapName currentdict /CMap defineresource pop end end"
                .to_vec(),
        ));
        let d = dictionary! {
            "Type" => "Font", "Subtype" => "Type0", "BaseFont" => "XYZABC+Inter-Bold",
            "Encoding" => "Identity-H", "ToUnicode" => tu,
            "DescendantFonts" => vec![dictionary! {
                "Type" => "Font", "Subtype" => "CIDFontType2", "DW" => 600,
                "W" => vec![3.into(), vec![700.into(), 450.into()].into(), 10.into(), 12.into(), 300.into()],
            }.into()],
        };
        let f = Font::load(&doc, &d);
        assert_eq!(f.kind, Kind::Cid);
        assert_eq!(f.codes(&[0, 3, 0, 4]), vec![(3, 0, 2), (4, 2, 2)]);
        assert!((f.width(4, 2) - 0.45).abs() < 1e-6);
        assert!((f.width(11, 2) - 0.3).abs() < 1e-6);
        assert!((f.width(99, 2) - 0.6).abs() < 1e-6);
        assert_eq!(f.text(4, 2).as_deref(), Some("é"));
        assert!(f.bold);
    }
}
