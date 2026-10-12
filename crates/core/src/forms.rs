//! Flatten interactive forms: draw each widget's appearance into the page and
//! drop the fields, so values can no longer be edited.

use crate::error::Result;
use crate::pdf::{self, n, num, resolve};
use lopdf::{Document, Object, ObjectId};
use serde::Serialize;

#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FlattenReport {
    pub flattened: usize,
    /// Widgets without an appearance stream, left out of the page.
    pub missing_appearance: usize,
}

type M = [f32; 6];

fn transform_rect(m: &M, r: [f32; 4]) -> [f32; 4] {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures;

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
