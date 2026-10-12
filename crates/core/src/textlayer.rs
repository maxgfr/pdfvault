//! Invisible text over scanned pages, from OCR results, so the PDF becomes
//! searchable and selectable while looking exactly the same.

use crate::error::{Error, Result};
use crate::pdf::{self, n};
use crate::stamp;
use lopdf::{Object, dictionary};
use serde::Deserialize;

/// One recognized word, in pixels of the rendered page image (origin top-left).
#[derive(Debug, Clone, Deserialize)]
pub struct OcrWord {
    pub text: String,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OcrPage {
    /// 0-based page index.
    pub page: usize,
    /// Size of the image the OCR ran on.
    pub width: f32,
    pub height: f32,
    pub words: Vec<OcrWord>,
}

pub fn add_text_layer(input: &[u8], password: Option<&str>, pages: &[OcrPage]) -> Result<Vec<u8>> {
    let mut doc = pdf::load(input, password)?;
    let ids = pdf::page_ids(&doc);
    for p in pages {
        let id = *ids
            .get(p.page)
            .ok_or_else(|| Error::option(format!("page {} does not exist", p.page + 1)))?;
        if p.words.is_empty() || p.width <= 0.0 || p.height <= 0.0 {
            continue;
        }
        let font = pdf::add_resource(
            &mut doc,
            id,
            b"Font",
            "PVOcr",
            dictionary! { "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica", "Encoding" => "WinAnsiEncoding" }
                .into(),
        )?;
        let font = String::from_utf8_lossy(&font).into_owned();
        let (frame, vw, vh) = stamp::visual_frame(&doc, id);
        let (sx, sy) = (vw / p.width, vh / p.height);
        let mut ops: Vec<u8> = Vec::new();
        ops.extend(
            format!(
                "q {} {} {} {} {} {} cm BT 3 Tr\n",
                n(frame[0]),
                n(frame[1]),
                n(frame[2]),
                n(frame[3]),
                n(frame[4]),
                n(frame[5])
            )
            .into_bytes(),
        );
        for w in &p.words {
            let text = stamp::win_ansi(w.text.trim());
            if text.is_empty() || w.w <= 0.0 || w.h <= 0.0 {
                continue;
            }
            let (wpt, hpt) = (w.w * sx, w.h * sy);
            let size = (hpt * 0.85).max(1.0);
            let x = w.x * sx;
            let baseline = vh - (w.y + w.h) * sy + hpt * 0.18;
            let natural = stamp::text_width(&text, size, false).max(0.01);
            let scale = (100.0 * wpt / natural).clamp(1.0, 1000.0);
            ops.extend(
                format!(
                    "/{font} {} Tf {} Tz 1 0 0 1 {} {} Tm ",
                    n(size),
                    n(scale),
                    n(x),
                    n(baseline)
                )
                .into_bytes(),
            );
            ops.extend(pdf::pdf_string(&text));
            ops.extend(b" Tj\n");
        }
        ops.extend(b"ET Q\n");
        pdf::append_content(&mut doc, id, ops)?;
    }
    pdf::save(&mut doc)
}

/// Whether a page already draws text (then OCR is usually unnecessary).
pub fn page_has_text(doc: &lopdf::Document, page: lopdf::ObjectId) -> bool {
    let content = doc.get_page_content(page);
    let Ok(c) = lopdf::content::Content::decode(&content) else {
        return false;
    };
    c.operations.iter().any(|op| {
        matches!(op.operator.as_str(), "Tj" | "TJ" | "'" | "\"")
            && op.operands.iter().any(|o| match o {
                Object::String(s, _) => !s.is_empty(),
                Object::Array(a) => !a.is_empty(),
                _ => false,
            })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures;

    #[test]
    fn layer_is_searchable() {
        let input = fixtures::lineart_pdf();
        let page = OcrPage {
            page: 0,
            width: 1000.0,
            height: 1000.0,
            words: vec![
                OcrWord {
                    text: "Invoice".into(),
                    x: 100.0,
                    y: 100.0,
                    w: 200.0,
                    h: 40.0,
                },
                OcrWord {
                    text: "total".into(),
                    x: 320.0,
                    y: 100.0,
                    w: 120.0,
                    h: 40.0,
                },
            ],
        };
        let out = add_text_layer(&input, None, &[page]).unwrap();
        let doc = pdf::load(&out, None).unwrap();
        let text = doc.extract_text(&[1]).unwrap();
        assert!(text.contains("Invoice") && text.contains("total"), "{text}");
        assert!(page_has_text(&doc, pdf::page_ids(&doc)[0]));
        let scan = pdf::load(&input, None).unwrap();
        assert!(!page_has_text(&scan, pdf::page_ids(&scan)[0]));
        crate::testutil::qpdf_check(&out);
    }
}
