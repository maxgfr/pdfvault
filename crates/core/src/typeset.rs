//! Writing text in any script: shaped with HarfRust, split into
//! left-to-right and right-to-left runs, each character drawn with the first
//! font that has it, and every font embedded as a subset (Type 0, Identity-H,
//! with a ToUnicode map so the text can be copied and searched).

use crate::error::{Error, Result};
use crate::pdf::{self, n};
use lopdf::{Dictionary, Document, Object, ObjectId, Stream, StringFormat, dictionary};
use skrifa::instance::{LocationRef, Size};
use skrifa::metrics::Metrics;
use skrifa::raw::{FileRef, TableProvider};
use skrifa::string::StringId;
use skrifa::{FontRef, GlyphId, MetadataProvider, Tag};
use std::collections::BTreeMap;

struct Face<'a> {
    /// The font as the shaper reads it (it keeps its own copy of the data).
    shaper: harfrust::Font,
    /// The same font for metrics and names.
    meta: FontRef<'a>,
    cff: bool,
    data: &'a [u8],
    index: u32,
    upem: f32,
    remapper: subsetter::GlyphRemapper,
    /// New glyph id → (advance in 1/1000 em, text it stands for).
    glyphs: BTreeMap<u16, (f32, String)>,
    id: Option<ObjectId>,
}

/// One glyph placed on a line, in points from the line's start.
#[derive(Debug, Clone)]
pub struct Placed {
    pub face: usize,
    pub gid: u16,
    pub x: f32,
    pub dy: f32,
    pub text: String,
}

#[derive(Debug, Clone, Default)]
pub struct Line {
    pub glyphs: Vec<Placed>,
    pub width: f32,
}

pub struct Typesetter<'a> {
    faces: Vec<Face<'a>>,
}

fn name_of(font: &FontRef, id: StringId) -> Option<String> {
    font.localized_strings(id)
        .english_or_first()
        .map(|s| s.chars().collect())
}

fn metrics_of(font: &FontRef) -> Metrics {
    font.metrics(Size::unscaled(), LocationRef::default())
}

/// The face of a font collection (`.ttc`) whose PostScript or full name is `name`.
pub fn face_index(data: &[u8], name: &str) -> u32 {
    let count = match FileRef::new(data) {
        Ok(FileRef::Collection(c)) => c.len(),
        _ => 1,
    };
    (0..count)
        .find(|&i| {
            FontRef::from_index(data, i).is_ok_and(|f| {
                [StringId::POSTSCRIPT_NAME, StringId::FULL_NAME]
                    .into_iter()
                    .any(|id| name_of(&f, id).as_deref() == Some(name))
            })
        })
        .unwrap_or(0)
}

fn postscript_name(font: &FontRef) -> String {
    let name = name_of(font, StringId::POSTSCRIPT_NAME).unwrap_or_else(|| "Font".into());
    let clean: String = name
        .chars()
        .filter(|c| c.is_ascii_graphic() && !"()<>[]{}/%#".contains(*c))
        .take(100)
        .collect();
    if clean.is_empty() {
        "Font".into()
    } else {
        clean
    }
}

fn subset_tag(glyphs: &BTreeMap<u16, (f32, String)>, name: &str) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in name
        .bytes()
        .chain(glyphs.keys().flat_map(|g| g.to_be_bytes()))
    {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    (0..6)
        .map(|i| (b'A' + ((h >> (i * 5)) % 26) as u8) as char)
        .collect()
}

fn utf16_hex(s: &str) -> String {
    s.encode_utf16().map(|u| format!("{u:04X}")).collect()
}

fn to_unicode(glyphs: &BTreeMap<u16, (f32, String)>) -> Vec<u8> {
    let mut out = String::from(
        "/CIDInit /ProcSet findresource begin\n12 dict begin\nbegincmap\n\
         /CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n\
         /CMapName /Adobe-Identity-UCS def\n/CMapType 2 def\n\
         1 begincodespacerange\n<0000> <FFFF>\nendcodespacerange\n",
    );
    let entries: Vec<_> = glyphs.iter().filter(|(_, (_, t))| !t.is_empty()).collect();
    for chunk in entries.chunks(100) {
        out.push_str(&format!("{} beginbfchar\n", chunk.len()));
        for (gid, (_, text)) in chunk {
            out.push_str(&format!("<{gid:04X}> <{}>\n", utf16_hex(text)));
        }
        out.push_str("endbfchar\n");
    }
    out.push_str("endcmap\nCMapName currentdict /CMap defineresource pop\nend\nend\n");
    out.into_bytes()
}

impl Default for Typesetter<'_> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> Typesetter<'a> {
    pub fn new() -> Self {
        Typesetter { faces: vec![] }
    }

    /// Register a TrueType or OpenType font; returns its slot.
    pub fn add_face(&mut self, data: &'a [u8], index: u32) -> Result<usize> {
        let not_a_font =
            || Error::option("not a TrueType or OpenType font (WOFF files need converting)");
        let meta = FontRef::from_index(data, index).map_err(|_| not_a_font())?;
        let cff = meta.cff().is_ok();
        if meta.glyf().is_err() && !cff {
            return Err(Error::Unsupported(
                "fonts with CFF2 or bitmap outlines".into(),
            ));
        }
        // fsType 2 alone: "restricted license embedding".
        if meta.os2().is_ok_and(|os2| os2.fs_type() & 0xF == 2) {
            return Err(Error::option(format!(
                "the font {} does not allow embedding",
                postscript_name(&meta)
            )));
        }
        let shaper = harfrust::Font::new(data.to_vec(), index).ok_or_else(not_a_font)?;
        let upem = metrics_of(&meta).units_per_em.max(1) as f32;
        let mut remapper = subsetter::GlyphRemapper::new();
        remapper.remap(0);
        self.faces.push(Face {
            shaper,
            meta,
            cff,
            data,
            index,
            upem,
            remapper,
            glyphs: BTreeMap::new(),
            id: None,
        });
        Ok(self.faces.len() - 1)
    }

    pub fn covers(&self, face: usize, c: char) -> bool {
        self.faces[face].meta.charmap().map(c).is_some()
    }

    /// Vertical metrics of a face, for a 1-point font: (ascent, descent).
    pub fn metrics(&self, face: usize) -> (f32, f32) {
        let f = &self.faces[face];
        let m = metrics_of(&f.meta);
        (m.ascent / f.upem, m.descent / f.upem)
    }

    /// Lay out one line of `text` at `size` points. Each character takes the
    /// first of `faces` that has it; characters none of them has are an error.
    pub fn layout(&mut self, faces: &[usize], text: &str, size: f32) -> Result<Line> {
        let mut line = Line::default();
        if text.is_empty() || faces.is_empty() {
            return Ok(line);
        }
        let mut missing: Vec<char> = vec![];
        let bidi = unicode_bidi::ParagraphBidiInfo::new(text, None);
        let (levels, runs) = bidi.visual_runs(0..text.len());
        let mut pen = 0.0f32;
        for run in runs {
            let rtl = levels[run.start].is_rtl();
            // Split the run where the font changes; spaces and marks stay with
            // the text around them.
            let mut segments: Vec<(usize, std::ops::Range<usize>)> = vec![];
            for (i, c) in text[run.clone()].char_indices() {
                let at = run.start + i;
                let neutral = c.is_whitespace() || c.is_control();
                let pick = faces.iter().copied().find(|&f| self.covers(f, c));
                let face = match (pick, segments.last()) {
                    (_, Some((last, _))) if neutral && self.covers(*last, c) => Some(*last),
                    (Some(f), _) => Some(f),
                    (None, Some((last, _))) if neutral => Some(*last),
                    (None, _) => {
                        if !neutral && !missing.contains(&c) {
                            missing.push(c);
                        }
                        segments.last().map(|s| s.0).or(Some(faces[0]))
                    }
                };
                let face = face.unwrap_or(faces[0]);
                match segments.last_mut() {
                    Some((f, r)) if *f == face => r.end = at + c.len_utf8(),
                    _ => segments.push((face, at..at + c.len_utf8())),
                }
            }
            if rtl {
                segments.reverse();
            }
            for (face, range) in segments {
                pen = self.shape(&mut line, face, &text[range], rtl, size, pen)?;
            }
        }
        if !missing.is_empty() {
            let list: String = missing.iter().collect();
            return Err(Error::MissingGlyphs(list));
        }
        line.width = pen;
        Ok(line)
    }

    fn shape(
        &mut self,
        line: &mut Line,
        face: usize,
        seg: &str,
        rtl: bool,
        size: f32,
        mut pen: f32,
    ) -> Result<f32> {
        let f = &mut self.faces[face];
        let mut buf = harfrust::Buffer::new();
        buf.push_str(seg);
        buf.set_direction(if rtl {
            harfrust::Direction::RightToLeft
        } else {
            harfrust::Direction::LeftToRight
        });
        buf.guess_segment_properties();
        harfrust::shape(
            &harfrust::ShaperFont::new(&f.shaper),
            &mut buf,
            harfrust::ShapeOptions::new(),
        )
        .map_err(|e| Error::Unsupported(format!("could not shape the text: {e}")))?;
        let infos = buf.glyph_infos();
        let pos = buf.glyph_positions();
        let mut clusters: Vec<usize> = infos.iter().map(|g| g.cluster as usize).collect();
        clusters.sort_unstable();
        clusters.dedup();
        let mut seen = std::collections::HashSet::new();
        let k = size / f.upem;
        let advances = f
            .meta
            .glyph_metrics(Size::unscaled(), LocationRef::default());
        for (g, p) in infos.iter().zip(pos) {
            let c = g.cluster as usize;
            let next = clusters
                .iter()
                .copied()
                .find(|&x| x > c)
                .unwrap_or(seg.len());
            let text = if seen.insert(c) {
                seg.get(c..next).unwrap_or("").to_string()
            } else {
                String::new()
            };
            let old = g.glyph_id as u16;
            let gid = f.remapper.remap(old);
            let advance = advances
                .advance_width(GlyphId::new(g.glyph_id))
                .unwrap_or(0.0)
                * 1000.0
                / f.upem;
            let entry = f.glyphs.entry(gid).or_insert((advance, String::new()));
            if entry.1.is_empty() {
                entry.1 = text.clone();
            }
            line.glyphs.push(Placed {
                face,
                gid,
                x: pen + p.x_offset as f32 * k,
                dy: p.y_offset as f32 * k,
                text,
            });
            pen += p.x_advance as f32 * k;
        }
        Ok(pen)
    }

    /// Content operators drawing `line` with its start at (`x`, `y`) on the
    /// baseline, in the coordinates the caller's content is in.
    pub fn draw(
        &mut self,
        doc: &mut Document,
        page: ObjectId,
        line: &Line,
        x: f32,
        y: f32,
        size: f32,
    ) -> Result<Vec<u8>> {
        let mut out = vec![];
        let mut i = 0;
        while i < line.glyphs.len() {
            let face = line.glyphs[i].face;
            let id = *self.faces[face]
                .id
                .get_or_insert_with(|| doc.new_object_id());
            let name = pdf::add_resource(doc, page, b"Font", "PVU", Object::Reference(id))?;
            out.extend(b"BT /");
            out.extend(&name);
            out.extend(format!(" {} Tf\n", n(size)).into_bytes());
            // A run of glyphs in this face, cut wherever a glyph sits off the baseline.
            let mut j = i;
            while j < line.glyphs.len() && line.glyphs[j].face == face {
                let start = j;
                let dy = line.glyphs[j].dy;
                out.extend(
                    format!("1 0 0 1 {} {} Tm [", n(x + line.glyphs[j].x), n(y + dy)).into_bytes(),
                );
                loop {
                    let g = &line.glyphs[j];
                    out.extend(format!("<{:04X}>", g.gid).into_bytes());
                    let next = line.glyphs.get(j + 1);
                    match next {
                        Some(nx) if nx.face == face && (nx.dy - dy).abs() < 1e-3 => {
                            let natural = self.faces[face].glyphs[&g.gid].0 * size / 1000.0;
                            let wanted = nx.x - g.x;
                            let adj = (natural - wanted) * 1000.0 / size;
                            if adj.abs() > 0.01 {
                                out.extend(format!(" {} ", n(adj)).into_bytes());
                            }
                            j += 1;
                        }
                        _ => {
                            j += 1;
                            break;
                        }
                    }
                }
                out.extend(b"] TJ\n");
                debug_assert!(j > start);
            }
            out.extend(b"ET\n");
            i = j;
        }
        Ok(out)
    }

    /// Embed the subsets of every face that was drawn.
    pub fn finish(self, doc: &mut Document) -> Result<()> {
        for f in self.faces {
            let Some(id) = f.id else { continue };
            let sub = subsetter::subset(f.data, f.index, &f.remapper)
                .map_err(|e| Error::Unsupported(format!("could not subset the font: {e}")))?;
            let cff = f.cff;
            // A CFF font is embedded as its bare CFF program.
            let program = if cff {
                FontRef::new(&sub)
                    .ok()
                    .and_then(|r| r.table_data(Tag::new(b"CFF ")))
                    .ok_or_else(|| Error::Unsupported("font subset without CFF".into()))?
                    .as_bytes()
                    .to_vec()
            } else {
                sub
            };
            let ps = postscript_name(&f.meta);
            let base = format!("{}+{ps}", subset_tag(&f.glyphs, &ps));
            let mut file = Stream::new(Dictionary::new(), program);
            if cff {
                file.dict.set("Subtype", "CIDFontType0C");
            }
            let _ = file.compress();
            let file_id = doc.add_object(file);
            let k = 1000.0 / f.upem;
            let m = metrics_of(&f.meta);
            let bb = m
                .bounds
                .map(|b| [b.x_min, b.y_min, b.x_max, b.y_max])
                .unwrap_or([0.0, m.descent, f.upem, m.ascent]);
            let italic = m.italic_angle;
            let mut flags = 4; // symbolic: glyphs are addressed by CID
            if italic != 0.0 {
                flags |= 64;
            }
            if m.is_monospace {
                flags |= 1;
            }
            let ascent = m.ascent * k;
            let descent = m.descent * k;
            let cap = m.cap_height.map(|v| v * k).unwrap_or(ascent);
            let descriptor = doc.add_object(dictionary! {
                "Type" => "FontDescriptor",
                "FontName" => Object::Name(base.clone().into_bytes()),
                "Flags" => flags,
                "FontBBox" => vec![
                    (bb[0] * k).into(), (bb[1] * k).into(),
                    (bb[2] * k).into(), (bb[3] * k).into(),
                ],
                "ItalicAngle" => italic,
                "Ascent" => ascent,
                "Descent" => descent,
                "CapHeight" => cap,
                "StemV" => 80,
                if cff { "FontFile3" } else { "FontFile2" } => file_id,
            });
            let count = f.remapper.num_gids();
            let widths: Vec<Object> = (0..count)
                .map(|g| Object::Real(f.glyphs.get(&g).map(|e| e.0).unwrap_or(0.0)))
                .collect();
            let mut cid = dictionary! {
                "Type" => "Font",
                "Subtype" => if cff { "CIDFontType0" } else { "CIDFontType2" },
                "BaseFont" => Object::Name(base.clone().into_bytes()),
                "CIDSystemInfo" => dictionary! {
                    "Registry" => Object::String(b"Adobe".to_vec(), StringFormat::Literal),
                    "Ordering" => Object::String(b"Identity".to_vec(), StringFormat::Literal),
                    "Supplement" => 0,
                },
                "FontDescriptor" => descriptor,
                "DW" => 0,
                "W" => vec![0.into(), Object::Array(widths)],
            };
            if !cff {
                cid.set("CIDToGIDMap", "Identity");
            }
            let cid_id = doc.add_object(cid);
            let mut cmap = Stream::new(Dictionary::new(), to_unicode(&f.glyphs));
            let _ = cmap.compress();
            let cmap_id = doc.add_object(cmap);
            let name = if cff {
                format!("{base}-Identity-H")
            } else {
                base
            };
            doc.objects.insert(
                id,
                dictionary! {
                    "Type" => "Font", "Subtype" => "Type0",
                    "BaseFont" => Object::Name(name.into_bytes()),
                    "Encoding" => "Identity-H",
                    "DescendantFonts" => vec![cid_id.into()],
                    "ToUnicode" => cmap_id,
                }
                .into(),
            );
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures;
    use crate::interp;
    use crate::testutil::qpdf_check;

    #[test]
    fn writes_unicode_text_that_reads_back() {
        let sans = fixtures::bundled_font("LiberationSans-Regular.ttf");
        let serif = fixtures::bundled_font("LiberationSerif-Bold.ttf");
        let mut doc = pdf::load(&fixtures::text_pdf(1), None).unwrap();
        let page = pdf::page_ids(&doc)[0];
        let mut ts = Typesetter::new();
        let a = ts.add_face(&sans, 0).unwrap();
        let b = ts.add_face(&serif, 0).unwrap();
        let text = "Ça coûte 5 € — Ωμέγα, Привет, ﬁn";
        let line = ts.layout(&[a], text, 18.0).unwrap();
        assert!(line.width > 100.0);
        let mut ops = ts.draw(&mut doc, page, &line, 72.0, 400.0, 18.0).unwrap();
        let bold = ts.layout(&[b], "Gras", 12.0).unwrap();
        ops.extend(ts.draw(&mut doc, page, &bold, 72.0, 380.0, 12.0).unwrap());
        pdf::append_content(&mut doc, page, ops).unwrap();
        ts.finish(&mut doc).unwrap();
        let out = pdf::save(&mut doc).unwrap();
        qpdf_check(&out);
        let doc = pdf::load(&out, None).unwrap();
        let p = interp::interpret(&doc, pdf::page_ids(&doc)[0]);
        let read: String = p
            .glyphs
            .iter()
            .filter(|g| (g.origin[1] - 400.0).abs() < 0.5)
            .filter_map(|g| g.text.clone())
            .collect();
        assert_eq!(read.replace(' ', ""), text.replace(' ', ""));
        let first = p
            .glyphs
            .iter()
            .find(|g| (g.origin[1] - 400.0).abs() < 0.5)
            .unwrap();
        assert!((first.origin[0] - 72.0).abs() < 0.01 && (first.size - 18.0).abs() < 0.01);
        assert!(p.reliable());
    }

    #[test]
    fn reports_characters_no_font_has() {
        let sans = fixtures::bundled_font("LiberationSans-Regular.ttf");
        let mut ts = Typesetter::new();
        let a = ts.add_face(&sans, 0).unwrap();
        match ts.layout(&[a], "Hello 世界 مرحبا", 12.0) {
            Err(Error::MissingGlyphs(chars)) => assert_eq!(chars, "世界مرحبا"),
            other => panic!("{:?}", other.map(|l| l.width)),
        }
        assert!(ts.add_face(b"not a font", 0).is_err());
    }

    #[test]
    fn right_to_left_runs_come_out_in_visual_order() {
        // Liberation Sans has Hebrew: the run is laid out right to left.
        let sans = fixtures::bundled_font("LiberationSans-Regular.ttf");
        let mut ts = Typesetter::new();
        let a = ts.add_face(&sans, 0).unwrap();
        let line = ts.layout(&[a], "abc שלום", 12.0).unwrap();
        let texts: Vec<&str> = line.glyphs.iter().map(|g| g.text.as_str()).collect();
        assert_eq!(&texts[..3], ["a", "b", "c"]);
        assert_eq!(
            texts.last(),
            Some(&"ש"),
            "the first Hebrew letter ends up rightmost"
        );
    }
}
