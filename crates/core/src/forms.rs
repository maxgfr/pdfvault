//! Interactive forms: list the fields, fill them, and flatten them (draw each
//! widget's appearance into the page and drop the fields, so values can no
//! longer be edited).

use crate::error::{Error, Result};
use crate::pdf::{self, n, num, resolve};
use crate::stamp;
use lopdf::{Document, Object, ObjectId, Stream, dictionary};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FlattenReport {
    pub flattened: usize,
    /// Widgets without an appearance stream, left out of the page.
    pub missing_appearance: usize,
}

pub(crate) type M = [f32; 6];

pub(crate) fn transform_rect(m: &M, r: [f32; 4]) -> [f32; 4] {
    let pts = [(r[0], r[1]), (r[2], r[1]), (r[0], r[3]), (r[2], r[3])];
    let mut b = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
    for (x, y) in pts {
        let (tx, ty) = (m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5]);
        b = [b[0].min(tx), b[1].min(ty), b[2].max(tx), b[3].max(ty)];
    }
    b
}

fn array4(doc: &Document, o: Option<&Object>) -> Option<[f32; 4]> {
    let a = resolve(doc, o?).as_array().ok()?;
    let v: Vec<f32> = a.iter().filter_map(|x| num(resolve(doc, x))).collect();
    (v.len() == 4).then(|| {
        [
            v[0].min(v[2]),
            v[1].min(v[3]),
            v[0].max(v[2]),
            v[1].max(v[3]),
        ]
    })
}

/// The appearance stream to draw: /AP /N, picking the /AS state when /N is a
/// dictionary of states.
fn appearance(doc: &Document, annot: &lopdf::Dictionary) -> Option<ObjectId> {
    let ap = resolve(doc, annot.get(b"AP").ok()?).as_dict().ok()?;
    match ap.get(b"N").ok()? {
        Object::Reference(id) => match doc.get_object(*id).ok()? {
            Object::Stream(_) => Some(*id),
            Object::Dictionary(states) => {
                let state = annot.get(b"AS").ok()?.as_name().ok()?;
                states.get(state).ok()?.as_reference().ok()
            }
            _ => None,
        },
        Object::Dictionary(states) => {
            let state = annot.get(b"AS").ok()?.as_name().ok()?;
            states.get(state).ok()?.as_reference().ok()
        }
        _ => None,
    }
}

pub fn flatten(input: &[u8], password: Option<&str>) -> Result<(Vec<u8>, FlattenReport)> {
    let mut doc = pdf::load(input, password)?;
    let mut report = FlattenReport::default();
    for page in pdf::page_ids(&doc) {
        let Some(annots) = doc
            .get_dictionary(page)
            .ok()
            .and_then(|p| p.get(b"Annots").ok())
            .cloned()
        else {
            continue;
        };
        let Ok(list) = resolve(&doc, &annots).as_array().cloned() else {
            continue;
        };
        let mut kept = vec![];
        let mut ops = String::new();
        for a in list {
            let Ok(d) = resolve(&doc, &a).as_dict().cloned() else {
                continue;
            };
            if d.get(b"Subtype").and_then(Object::as_name).ok() != Some(b"Widget") {
                kept.push(a);
                continue;
            }
            let flags = d.get(b"F").and_then(Object::as_i64).unwrap_or(0);
            // Hidden (2) or NoView (32) widgets are not drawn.
            if flags & (2 | 32) != 0 {
                continue;
            }
            let (Some(rect), Some(ap)) = (array4(&doc, d.get(b"Rect").ok()), appearance(&doc, &d))
            else {
                report.missing_appearance += 1;
                continue;
            };
            let Ok(stream) = doc.get_object(ap).and_then(Object::as_stream) else {
                continue;
            };
            let bbox = array4(&doc, stream.dict.get(b"BBox").ok()).unwrap_or([0.0, 0.0, 1.0, 1.0]);
            let matrix: M = stream
                .dict
                .get(b"Matrix")
                .ok()
                .and_then(|m| resolve(&doc, m).as_array().ok())
                .map(|a| a.iter().filter_map(num).collect::<Vec<_>>())
                .filter(|v| v.len() == 6)
                .map(|v| [v[0], v[1], v[2], v[3], v[4], v[5]])
                .unwrap_or([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
            // PDF 32000 12.5.5: map the transformed BBox onto Rect.
            let t = transform_rect(&matrix, bbox);
            let (tw, th) = ((t[2] - t[0]).max(1e-3), (t[3] - t[1]).max(1e-3));
            let (sx, sy) = ((rect[2] - rect[0]) / tw, (rect[3] - rect[1]) / th);
            let a_m = [sx, 0.0, 0.0, sy, rect[0] - t[0] * sx, rect[1] - t[1] * sy];
            if let Ok(Object::Stream(s)) = doc.get_object_mut(ap) {
                s.dict.set("Type", "XObject");
                s.dict.set("Subtype", "Form");
            }
            let name =
                pdf::add_resource(&mut doc, page, b"XObject", "PVFm", Object::Reference(ap))?;
            ops.push_str(&format!(
                "q {} {} {} {} {} {} cm /{} Do Q\n",
                n(a_m[0]),
                n(a_m[1]),
                n(a_m[2]),
                n(a_m[3]),
                n(a_m[4]),
                n(a_m[5]),
                String::from_utf8_lossy(&name)
            ));
            report.flattened += 1;
        }
        if !ops.is_empty() {
            pdf::append_content(&mut doc, page, ops.into_bytes())?;
        }
        let p = doc.get_dictionary_mut(page)?;
        if kept.is_empty() {
            p.remove(b"Annots");
        } else {
            p.set("Annots", kept);
        }
    }
    if let Ok(cat) = doc.catalog_mut() {
        cat.remove(b"AcroForm");
    }
    Ok((pdf::save(&mut doc)?, report))
}

// ---------------------------------------------------------------- fill

/// One widget of a terminal form field, as the editor draws it.
#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FieldInfo {
    /// Fully qualified name (`parent.child`), the key [`fill`] expects.
    pub name: String,
    /// "text", "checkbox", "radio" or "choice".
    pub kind: &'static str,
    /// Text of a text or choice field; for buttons, the selected state name
    /// ("Off" when nothing is chosen).
    pub value: String,
    /// Export values of a choice field.
    pub options: Vec<String>,
    /// The state this checkbox or radio widget sets when chosen.
    pub on_state: Option<String>,
    pub page: usize,
    /// x, y, width, height in visual page space (points, origin at the
    /// bottom-left of the page as displayed).
    pub rect: [f32; 4],
    pub multiline: bool,
    pub read_only: bool,
    pub max_len: Option<i64>,
}

const FF_READ_ONLY: i64 = 1;
const FF_MULTILINE: i64 = 1 << 12;
const FF_RADIO: i64 = 1 << 15;
const FF_PUSHBUTTON: i64 = 1 << 16;

struct FieldNode {
    id: ObjectId,
    name: String,
    ft: Vec<u8>,
    flags: i64,
    widgets: Vec<ObjectId>,
}

fn walk(
    doc: &Document,
    id: ObjectId,
    parent: &str,
    inherited: (Option<Vec<u8>>, Option<i64>),
    depth: usize,
    out: &mut Vec<FieldNode>,
) {
    let Ok(d) = doc.get_dictionary(id) else {
        return;
    };
    if depth > 32 {
        return;
    }
    let part = d.get(b"T").ok().and_then(|t| match resolve(doc, t) {
        Object::String(s, _) => Some(decode_text(s)),
        _ => None,
    });
    let name = match part {
        Some(p) if parent.is_empty() => p,
        Some(p) => format!("{parent}.{p}"),
        None => parent.to_owned(),
    };
    let ft = d
        .get(b"FT")
        .ok()
        .and_then(|o| o.as_name().ok())
        .map(<[u8]>::to_vec)
        .or(inherited.0);
    let flags = d
        .get(b"Ff")
        .ok()
        .and_then(|o| resolve(doc, o).as_i64().ok())
        .or(inherited.1);
    let kids: Vec<ObjectId> = d
        .get(b"Kids")
        .ok()
        .and_then(|k| resolve(doc, k).as_array().ok())
        .map(|a| a.iter().filter_map(|o| o.as_reference().ok()).collect())
        .unwrap_or_default();
    // Kids carrying /T are child fields; kids without it are this field's widgets.
    let child_fields: Vec<ObjectId> = kids
        .iter()
        .copied()
        .filter(|k| doc.get_dictionary(*k).is_ok_and(|kd| kd.has(b"T")))
        .collect();
    if !child_fields.is_empty() {
        for k in child_fields {
            walk(doc, k, &name, (ft.clone(), flags), depth + 1, out);
        }
        return;
    }
    let Some(ft) = ft else {
        return;
    };
    out.push(FieldNode {
        id,
        name,
        ft,
        flags: flags.unwrap_or(0),
        widgets: if kids.is_empty() { vec![id] } else { kids },
    });
}

fn field_nodes(doc: &Document) -> Vec<FieldNode> {
    let fields = doc
        .catalog()
        .ok()
        .and_then(|c| c.get(b"AcroForm").ok())
        .and_then(|a| resolve(doc, a).as_dict().ok())
        .and_then(|a| a.get(b"Fields").ok())
        .and_then(|f| resolve(doc, f).as_array().ok())
        .cloned()
        .unwrap_or_default();
    let mut out = vec![];
    for f in fields {
        if let Ok(id) = f.as_reference() {
            walk(doc, id, "", (None, None), 0, &mut out);
        }
    }
    out
}

/// PDF text string → Rust string: UTF-16BE with a BOM, else PDFDocEncoding
/// (read as Latin-1, which agrees with it on printable text).
pub fn decode_text(b: &[u8]) -> String {
    if let Some(rest) = b.strip_prefix(&[0xFE, 0xFF]) {
        let units: Vec<u16> = rest
            .as_chunks::<2>()
            .0
            .iter()
            .map(|c| u16::from_be_bytes(*c))
            .collect();
        String::from_utf16_lossy(&units)
    } else {
        b.iter().map(|&c| c as char).collect()
    }
}

/// Rust string → PDF text string: a literal for plain ASCII, UTF-16BE otherwise.
pub fn text_string(s: &str) -> Object {
    if s.chars()
        .all(|c| (' '..='~').contains(&c) || c == '\n' || c == '\r')
    {
        Object::string_literal(s)
    } else {
        let mut b = vec![0xFE, 0xFF];
        for u in s.encode_utf16() {
            b.extend(u.to_be_bytes());
        }
        Object::String(b, lopdf::StringFormat::Hexadecimal)
    }
}

fn kind_of(node: &FieldNode) -> Option<&'static str> {
    match node.ft.as_slice() {
        b"Tx" => Some("text"),
        b"Ch" => Some("choice"),
        b"Btn" if node.flags & FF_PUSHBUTTON != 0 => None,
        b"Btn" if node.flags & FF_RADIO != 0 => Some("radio"),
        b"Btn" => Some("checkbox"),
        _ => None,
    }
}

fn field_value(doc: &Document, node: &FieldNode) -> String {
    match pdf::inherited(doc, node.id, b"V") {
        Some(Object::String(s, _)) => decode_text(&s),
        Some(Object::Name(n)) => String::from_utf8_lossy(&n).into_owned(),
        Some(Object::Array(a)) => a
            .first()
            .and_then(|o| o.as_str().ok())
            .map(decode_text)
            .unwrap_or_default(),
        _ => String::new(),
    }
}

/// The non-Off state of a button widget's normal appearance.
fn on_state(doc: &Document, widget: ObjectId) -> Option<String> {
    let d = doc.get_dictionary(widget).ok()?;
    let ap = resolve(doc, d.get(b"AP").ok()?).as_dict().ok()?;
    let states = resolve(doc, ap.get(b"N").ok()?).as_dict().ok()?;
    states
        .iter()
        .map(|(k, _)| k)
        .find(|k| k.as_slice() != b"Off")
        .map(|k| String::from_utf8_lossy(k).into_owned())
}

fn choice_options(doc: &Document, node: &FieldNode) -> Vec<String> {
    let Some(Object::Array(arr)) = pdf::inherited(doc, node.id, b"Opt") else {
        return vec![];
    };
    arr.iter()
        .filter_map(|o| match resolve(doc, o) {
            Object::String(s, _) => Some(decode_text(s)),
            // [export, display] pairs: /V holds the export value.
            Object::Array(pair) => pair.first().and_then(|e| e.as_str().ok()).map(decode_text),
            _ => None,
        })
        .collect()
}

/// Widget id → (page index, page id), from each page's /Annots.
fn widget_pages(doc: &Document) -> HashMap<ObjectId, (usize, ObjectId)> {
    let mut map = HashMap::new();
    for (i, page) in pdf::page_ids(doc).into_iter().enumerate() {
        let annots = doc
            .get_dictionary(page)
            .ok()
            .and_then(|p| p.get(b"Annots").ok())
            .and_then(|a| resolve(doc, a).as_array().ok())
            .cloned()
            .unwrap_or_default();
        for a in annots {
            if let Ok(id) = a.as_reference() {
                map.insert(id, (i, page));
            }
        }
    }
    map
}

pub(crate) fn invert(m: &M) -> M {
    let det = m[0] * m[3] - m[1] * m[2];
    let det = if det.abs() < 1e-9 { 1.0 } else { det };
    let (a, b, c, d) = (m[3] / det, -m[1] / det, -m[2] / det, m[0] / det);
    [a, b, c, d, -(a * m[4] + c * m[5]), -(b * m[4] + d * m[5])]
}

/// Every fillable widget, in page order.
pub fn list_fields(input: &[u8], password: Option<&str>) -> Result<Vec<FieldInfo>> {
    let doc = pdf::load(input, password)?;
    let pages = widget_pages(&doc);
    let mut out = vec![];
    for node in field_nodes(&doc) {
        let Some(kind) = kind_of(&node) else {
            continue;
        };
        let value = field_value(&doc, &node);
        let options = if kind == "choice" {
            choice_options(&doc, &node)
        } else {
            vec![]
        };
        let max_len = pdf::inherited(&doc, node.id, b"MaxLen").and_then(|o| o.as_i64().ok());
        for w in &node.widgets {
            let Some(&(page, page_id)) = pages.get(w) else {
                continue;
            };
            let rect = doc
                .get_dictionary(*w)
                .ok()
                .and_then(|d| d.get(b"Rect").ok());
            let Some(r) = array4(&doc, rect) else {
                continue;
            };
            let (frame, _, _) = stamp::visual_frame(&doc, page_id);
            let v = transform_rect(&invert(&frame), r);
            out.push(FieldInfo {
                name: node.name.clone(),
                kind,
                value: value.clone(),
                options: options.clone(),
                on_state: if matches!(kind, "checkbox" | "radio") {
                    on_state(&doc, *w)
                } else {
                    None
                },
                page,
                rect: [v[0], v[1], v[2] - v[0], v[3] - v[1]],
                multiline: node.flags & FF_MULTILINE != 0,
                read_only: node.flags & FF_READ_ONLY != 0,
                max_len,
            });
        }
    }
    out.sort_by_key(|f| f.page);
    Ok(out)
}

/// A value for [`fill`]: text for text and choice fields, a boolean or a
/// state name for checkboxes, a state name for radio groups.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum FieldValue {
    Bool(bool),
    Text(String),
}

/// The font size of a /DA string such as `/Helv 0 Tf 0 g`; 0 means auto.
fn da_size(da: &str) -> f32 {
    let t: Vec<&str> = da.split_whitespace().collect();
    t.iter()
        .position(|w| *w == "Tf")
        .and_then(|i| i.checked_sub(1))
        .and_then(|i| t[i].parse::<f32>().ok())
        .unwrap_or(0.0)
}

/// A plain Helvetica appearance for a text or choice widget.
fn text_appearance(
    doc: &mut Document,
    widget: ObjectId,
    text: &str,
    multiline: bool,
) -> Result<()> {
    let Some(r) = array4(doc, doc.get_dictionary(widget)?.get(b"Rect").ok()) else {
        return Ok(());
    };
    let (w, h) = (r[2] - r[0], r[3] - r[1]);
    let da = pdf::inherited(doc, widget, b"DA")
        .and_then(|o| {
            o.as_str()
                .map(|s| String::from_utf8_lossy(s).into_owned())
                .ok()
        })
        .unwrap_or_default();
    let quad = pdf::inherited(doc, widget, b"Q")
        .and_then(|o| o.as_i64().ok())
        .unwrap_or(0);
    let mut size = da_size(&da);
    if size <= 0.0 {
        size = if multiline {
            10.0
        } else {
            ((h - 4.0) * 0.75).clamp(4.0, 12.0)
        };
    }
    let mut ops =
        format!("/Tx BMC\nq 1 1 {} {} re W n\n0 g\n", n(w - 2.0), n(h - 2.0)).into_bytes();
    let lines: Vec<&str> = if multiline {
        text.lines().collect()
    } else {
        vec![text.lines().next().unwrap_or("")]
    };
    for (i, line) in lines.iter().enumerate() {
        let bytes = stamp::win_ansi(line);
        let tw = stamp::text_width(&bytes, size, false);
        let x = match quad {
            1 => (w - tw) / 2.0,
            2 => w - 2.0 - tw,
            _ => 2.0,
        };
        let y = if multiline {
            h - 2.0 - size * (i as f32 + 1.0) * 1.15
        } else {
            (h - size) / 2.0 + size * 0.22
        };
        stamp::push_text(&mut ops, "Helv", size, x, y, &bytes);
    }
    ops.extend(b"Q\nEMC\n");
    let font = doc.add_object(stamp::font_dict(false));
    let ap = doc.add_object(Stream::new(
        dictionary! {
            "Type" => "XObject", "Subtype" => "Form",
            "BBox" => vec![0.into(), 0.into(), w.into(), h.into()],
            "Resources" => dictionary! { "Font" => dictionary! { "Helv" => font } },
        },
        ops,
    ));
    doc.get_dictionary_mut(widget)?
        .set("AP", dictionary! { "N" => ap });
    Ok(())
}

/// Set field values by fully qualified name. Returns how many fields changed.
pub fn fill(doc: &mut Document, values: &BTreeMap<String, FieldValue>) -> Result<usize> {
    if values.is_empty() {
        return Ok(0);
    }
    let nodes = field_nodes(doc);
    let mut needs_viewer = false;
    for (name, value) in values {
        let node = nodes
            .iter()
            .find(|f| &f.name == name)
            .ok_or_else(|| Error::option(format!("no form field named '{name}'")))?;
        let kind = kind_of(node)
            .ok_or_else(|| Error::option(format!("field '{name}' cannot be filled")))?;
        if node.flags & FF_READ_ONLY != 0 {
            return Err(Error::option(format!("field '{name}' is read-only")));
        }
        if matches!(kind, "text" | "choice") {
            let text = match value {
                FieldValue::Text(s) => s.clone(),
                FieldValue::Bool(b) => b.to_string(),
            };
            needs_viewer |= text
                .chars()
                .zip(stamp::win_ansi(&text))
                .any(|(c, b)| b == b'?' && c != '?');
            doc.get_dictionary_mut(node.id)?
                .set("V", text_string(&text));
            for w in &node.widgets {
                text_appearance(doc, *w, &text, node.flags & FF_MULTILINE != 0)?;
            }
            continue;
        }
        // Checkbox: true picks each widget's own on state. Radio: a state name.
        let wanted = match value {
            FieldValue::Bool(true) => None,
            FieldValue::Bool(false) => Some("Off".to_owned()),
            FieldValue::Text(s) if s.is_empty() => Some("Off".to_owned()),
            FieldValue::Text(s) => Some(s.clone()),
        };
        let mut chosen = "Off".to_owned();
        for w in &node.widgets {
            let on = on_state(doc, *w).unwrap_or_else(|| "Yes".into());
            let state = match &wanted {
                Some(s) if *s != on => "Off".to_owned(),
                _ => on,
            };
            if state != "Off" {
                chosen = state.clone();
            }
            doc.get_dictionary_mut(*w)?
                .set("AS", Object::Name(state.into_bytes()));
        }
        if let Some(s) = &wanted
            && s != "Off"
            && chosen == "Off"
        {
            return Err(Error::option(format!("field '{name}' has no state '{s}'")));
        }
        doc.get_dictionary_mut(node.id)?
            .set("V", Object::Name(chosen.into_bytes()));
    }
    if needs_viewer {
        // Characters Helvetica cannot show: viewers redraw the fields from /V.
        match doc.catalog()?.get(b"AcroForm")?.clone() {
            Object::Reference(id) => doc.get_dictionary_mut(id)?.set("NeedAppearances", true),
            Object::Dictionary(mut d) => {
                d.set("NeedAppearances", true);
                doc.catalog_mut()?.set("AcroForm", d);
            }
            _ => {}
        }
    }
    Ok(values.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures;

    #[test]
    fn lists_and_fills_fields() {
        let input = fixtures::form_pdf();
        let fields = list_fields(&input, None).unwrap();
        assert_eq!(fields.len(), 2);
        let name = fields.iter().find(|f| f.name == "name").unwrap();
        assert_eq!((name.kind, name.value.as_str()), ("text", "Ada Lovelace"));
        assert_eq!(name.rect, [120.0, 680.0, 200.0, 20.0]);
        let agree = fields.iter().find(|f| f.name == "agree").unwrap();
        assert_eq!(agree.kind, "checkbox");
        assert_eq!(agree.on_state.as_deref(), Some("Yes"));

        let mut doc = pdf::load(&input, None).unwrap();
        let values = BTreeMap::from([
            ("name".to_owned(), FieldValue::Text("Grace Hopper".into())),
            ("agree".to_owned(), FieldValue::Bool(false)),
        ]);
        assert_eq!(fill(&mut doc, &values).unwrap(), 2);
        let out = pdf::save(&mut doc).unwrap();
        let after = list_fields(&out, None).unwrap();
        assert_eq!(
            after.iter().find(|f| f.name == "name").unwrap().value,
            "Grace Hopper"
        );
        assert_eq!(
            after.iter().find(|f| f.name == "agree").unwrap().value,
            "Off"
        );
        crate::testutil::qpdf_check(&out);

        let (flat, rep) = flatten(&out, None).unwrap();
        assert_eq!(rep.flattened, 2);
        assert!(fixtures::any_stream_contains(&flat, b"(Grace Hopper) Tj"));

        let bad = BTreeMap::from([("nope".to_owned(), FieldValue::Bool(true))]);
        assert!(fill(&mut pdf::load(&input, None).unwrap(), &bad).is_err());
    }

    #[test]
    fn flattens_widgets() {
        let (out, rep) = flatten(&fixtures::form_pdf(), None).unwrap();
        assert_eq!(rep.flattened, 2);
        let doc = pdf::load(&out, None).unwrap();
        assert!(!doc.catalog().unwrap().has(b"AcroForm"));
        let page = pdf::page_ids(&doc)[0];
        assert!(!doc.get_dictionary(page).unwrap().has(b"Annots"));
        let content = String::from_utf8_lossy(&doc.get_page_content(page)).into_owned();
        assert_eq!(content.matches(" Do Q").count(), 2, "{content}");
        crate::testutil::qpdf_check(&out);
    }

    #[test]
    fn no_form_is_fine() {
        let (_, rep) = flatten(&fixtures::text_pdf(1), None).unwrap();
        assert_eq!(rep.flattened, 0);
    }
}
