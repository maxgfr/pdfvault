//! Redaction in the content itself: what lies under a box is removed from the
//! page (glyphs, the pixels of images, shapes drawn entirely inside it,
//! annotations and form fields over it) and everything else stays as it was,
//! text included. Pages this cannot be sure about are left to the renderer,
//! which turns them into images instead.

use crate::compress;
use crate::error::Result;
use crate::interp::{self, Page};
use crate::scrub::Removal;
use lopdf::{Document, Object, ObjectId};
use std::collections::HashSet;

/// Share of a glyph's box a redaction box must cover for the glyph to go.
/// A glyph mostly outside the box stays visible, so it stays in the text too.
const GLYPH_COVER: f32 = 0.2;

/// What `boxes` (visual space, normalized) take off the page. `None` when the
/// page cannot be read with certainty.
pub fn removal(page: &Page, boxes: &[[f32; 4]]) -> Option<Removal> {
    if !page.reliable() {
        return None;
    }
    let mut r = Removal::default();
    for (i, g) in page.glyphs.iter().enumerate() {
        let a = interp::area(&g.bbox);
        let hit = boxes.iter().any(|b| {
            if a <= 1e-6 {
                let c = g.center();
                c[0] >= b[0] && c[0] <= b[2] && c[1] >= b[1] && c[1] <= b[3]
            } else {
                interp::area(&interp::intersect(&g.bbox, b)) >= a * GLYPH_COVER
            }
        });
        if hit {
            r.glyphs.insert(i);
        }
    }
    for (i, im) in page.images.iter().enumerate() {
        let under: Vec<[f32; 4]> = boxes
            .iter()
            .filter(|b| interp::area(&interp::intersect(&im.bbox, b)) > 0.0)
            .copied()
            .collect();
        if under.is_empty() {
            continue;
        }
        if under.iter().any(|b| interp::contains(b, &im.bbox)) {
            r.images.insert(i);
        } else {
            r.blackout.insert(i, under);
        }
    }
    for (i, p) in page.paths.iter().enumerate() {
        if !p.clip && boxes.iter().any(|b| interp::contains(b, &p.bbox)) {
            r.paths.insert(i);
        }
    }
    for (i, f) in page.forms.iter().enumerate() {
        if f.child.is_some() && boxes.iter().any(|b| interp::contains(b, &f.bbox)) {
            r.forms.insert(i);
        }
    }
    Some(r)
}

/// Remove the annotations (links, comments, form fields) that overlap a box.
/// Returns how many went.
pub fn remove_annotations(
    doc: &mut Document,
    page: ObjectId,
    to_visual: &interp::Matrix,
    boxes: &[[f32; 4]],
) -> Result<usize> {
    let annots: Vec<Object> = match doc.get_dictionary(page)?.get(b"Annots") {
        Ok(Object::Array(a)) => a.clone(),
        Ok(Object::Reference(id)) => doc.get_object(*id)?.as_array()?.clone(),
        _ => return Ok(0),
    };
    let mut gone: HashSet<ObjectId> = HashSet::new();
    let mut kept = vec![];
    for a in annots {
        let id = a.as_reference().ok();
        let dict = match &a {
            Object::Reference(r) => doc.get_dictionary(*r).ok(),
            Object::Dictionary(d) => Some(d),
            _ => None,
        };
        let rect = dict
            .and_then(|d| d.get(b"Rect").ok())
            .and_then(|r| r.as_array().ok())
            .map(|v| v.iter().filter_map(crate::pdf::num).collect::<Vec<f32>>())
            .filter(|v| v.len() == 4);
        let hit = rect.is_some_and(|v| {
            let b = interp::map_box(
                to_visual,
                [
                    v[0].min(v[2]),
                    v[1].min(v[3]),
                    v[0].max(v[2]),
                    v[1].max(v[3]),
                ],
            );
            boxes
                .iter()
                .any(|x| interp::area(&interp::intersect(&b, x)) > 0.0)
        });
        if hit {
            if let Some(id) = id {
                gone.insert(id);
                if let Some(Ok(Object::Reference(popup))) = dict.map(|d| d.get(b"Popup")) {
                    gone.insert(*popup);
                }
            }
        } else {
            kept.push(a);
        }
    }
    let kept: Vec<Object> = kept
        .into_iter()
        .filter(|a| a.as_reference().map(|r| !gone.contains(&r)).unwrap_or(true))
        .collect();
    let n = gone.len();
    if n > 0 {
        doc.get_dictionary_mut(page)?.set("Annots", kept);
        compress::prune_fields(doc, &gone);
    }
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures;
    use crate::pdf;
    use crate::scrub;
    use crate::testutil::qpdf_check;

    #[test]
    fn takes_out_the_covered_words_only() {
        let mut doc = pdf::load(&fixtures::text_pdf(1), None).unwrap();
        let id = pdf::page_ids(&doc)[0];
        let page = interp::interpret(&doc, id);
        // A box over "quick brown" on the second line.
        let line: Vec<_> = page
            .glyphs
            .iter()
            .filter(|g| (g.origin[1] - 702.0).abs() < 0.5)
            .collect();
        let text: String = line.iter().filter_map(|g| g.text.clone()).collect();
        let at = text.find("quick").unwrap();
        let b = [
            line[at].bbox[0],
            line[at].bbox[1],
            line[at + 10].bbox[2],
            line[at].bbox[3],
        ];
        let r = removal(&page, &[b]).unwrap();
        assert_eq!(r.glyphs.len(), 11);
        scrub::apply(&mut doc, id, &page, &r).unwrap();
        let out = pdf::save(&mut doc).unwrap();
        qpdf_check(&out);
        let text = pdf::load(&out, None).unwrap().extract_text(&[1]).unwrap();
        assert!(!text.contains("quick") && !text.contains("brown"), "{text}");
        assert!(
            text.contains("fox jumps") && text.contains("Second line"),
            "{text}"
        );
    }

    #[test]
    fn removes_fields_under_the_box() {
        let mut doc = pdf::load(&fixtures::form_pdf(), None).unwrap();
        let id = pdf::page_ids(&doc)[0];
        let page = interp::interpret(&doc, id);
        let n = remove_annotations(
            &mut doc,
            id,
            &page.to_visual,
            &[[100.0, 670.0, 400.0, 710.0]],
        )
        .unwrap();
        assert_eq!(n, 1);
        let out = pdf::save(&mut doc).unwrap();
        let fields = crate::forms::list_fields(&out, None).unwrap();
        assert_eq!(fields.len(), 1, "only the checkbox is left");
    }
}
