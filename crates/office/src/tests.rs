use super::*;
use pdfvault_core::epub::zip;
use pdfvault_core::fixtures::{bundled_font, docx, legacy_doc, pptx, pptx_hidden_slide, xlsx};
use pdfvault_core::{interp, pdf};
use std::process::Command;

/// Every PDF we emit must pass `qpdf --check` (core's helper is crate-private).
fn qpdf_check(bytes: &[u8], name: &str) {
    let dir = std::env::temp_dir().join(format!("pdfvault-office-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("{name}.pdf"));
    std::fs::write(&path, bytes).unwrap();
    let out = Command::new("qpdf")
        .arg("--check")
        .arg(&path)
        .output()
        .expect("qpdf must be installed to run the tests");
    let _ = std::fs::remove_file(&path);
    assert!(
        out.status.code() == Some(0),
        "qpdf --check failed ({:?}):\n{}{}",
        out.status.code(),
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

/// The fonts the office2pdf tool passes: Liberation, Carlito and Caladea.
fn fonts() -> Vec<Vec<u8>> {
    let mut names = vec![];
    for family in [
        "LiberationSans",
        "LiberationSerif",
        "LiberationMono",
        "Carlito",
        "Caladea",
    ] {
        for style in ["Regular", "Bold", "Italic", "BoldItalic"] {
            names.push(format!("{family}-{style}.ttf"));
        }
    }
    names.iter().map(|n| bundled_font(n)).collect()
}

fn convert_with(doc: Vec<u8>, options: &str) -> (Vec<u8>, Value) {
    let mut inputs = vec![doc];
    inputs.extend(fonts());
    let mut out = run("office2pdf", inputs, options).unwrap();
    assert_eq!(out.files.len(), 1);
    assert_eq!(out.files[0].ext, "pdf");
    (out.files.remove(0).bytes, out.report)
}

fn convert(doc: Vec<u8>) -> (Vec<u8>, Value) {
    convert_with(doc, r#"{"fallbackFont":"Liberation Sans"}"#)
}

/// The text of each page, as the core's interpreter reads it back.
fn text_of(bytes: &[u8]) -> Vec<String> {
    let doc = pdf::load(bytes, None).unwrap();
    pdf::page_ids(&doc)
        .into_iter()
        .map(|p| {
            interp::interpret(&doc, p)
                .glyphs
                .iter()
                .filter_map(|g| g.text.clone())
                .collect::<String>()
        })
        .collect()
}

/// Text without the spaces a layout may or may not draw as glyphs.
fn squeeze(s: &str) -> String {
    s.chars().filter(|c| !c.is_whitespace()).collect()
}

/// The first byte offset where `a` and `b` differ, with some context.
fn first_difference(a: &[u8], b: &[u8]) -> String {
    let i = a
        .iter()
        .zip(b)
        .position(|(x, y)| x != y)
        .unwrap_or(a.len().min(b.len()));
    let ctx = |s: &[u8]| {
        String::from_utf8_lossy(&s[i.saturating_sub(80)..(i + 80).min(s.len())]).into_owned()
    };
    format!(
        "lengths {} / {}, first difference at {i}:\n{}\n---\n{}",
        a.len(),
        b.len(),
        ctx(a),
        ctx(b)
    )
}

fn assert_deterministic(doc: fn() -> Vec<u8>, name: &str) -> (Vec<u8>, Value) {
    let (a, report) = convert(doc());
    let (b, _) = convert(doc());
    assert!(a.starts_with(b"%PDF-"));
    assert!(
        a == b,
        "two conversions differ: {}",
        first_difference(&a, &b)
    );
    qpdf_check(&a, name);
    (a, report)
}

#[test]
fn docx_converts_deterministically_with_its_text() {
    let (pdf, report) = assert_deterministic(docx, "docx");
    assert_eq!(report["format"], "docx");
    assert_eq!(report["pages"], 1);
    let text = squeeze(&text_of(&pdf).concat());
    for s in [
        "Quarterlyreport",
        "twelvepercent",
        "café",
        "Paper",
        "Postage",
        "Q2",
        "135",
    ] {
        assert!(text.contains(s), "{s:?} missing from {text:?}");
    }
    // Calibri and Cambria come out as their metric-compatible stand-ins,
    // not as a fallback the host happens to have.
    assert!(contains(&pdf, b"Carlito"), "Carlito is not embedded");
    assert!(contains(&pdf, b"Caladea"), "Caladea is not embedded");
    // The picture is drawn.
    assert!(contains(&pdf, b"/Image"), "the PNG is missing");
}

#[test]
fn xlsx_converts_every_sheet_or_the_ones_named() {
    let (pdf, report) = assert_deterministic(xlsx, "xlsx");
    assert_eq!(report["format"], "xlsx");
    let pages = text_of(&pdf);
    assert_eq!(report["pages"], pages.len());
    let all = squeeze(&pages.concat());
    for s in ["Quarterlysummary", "Total", "335", "Postage", "120"] {
        assert!(all.contains(s), "{s:?} missing from {all:?}");
    }

    let (only, report) = convert_with(xlsx(), r#"{"sheets":["Data"]}"#);
    qpdf_check(&only, "xlsx-data");
    assert_eq!(report["pages"], 1);
    let text = squeeze(&text_of(&only).concat());
    assert!(text.contains("Postage"), "{text:?}");
    assert!(!text.contains("Quarterlysummary"), "{text:?}");
}

#[test]
fn pptx_converts_every_slide_or_a_range() {
    let (pdf, report) = assert_deterministic(pptx, "pptx");
    assert_eq!(report["format"], "pptx");
    assert_eq!(report["pages"], 2);
    let pages: Vec<String> = text_of(&pdf).iter().map(|p| squeeze(p)).collect();
    assert!(pages[0].contains("Roadmap"), "{pages:?}");
    assert!(pages[0].contains("ShiptheOfficeconverter"), "{pages:?}");
    assert!(pages[1].contains("Nextsteps"), "{pages:?}");

    let (second, report) = convert_with(pptx(), r#"{"slides":"2"}"#);
    qpdf_check(&second, "pptx-2");
    assert_eq!(report["pages"], 1);
    let text = squeeze(&text_of(&second).concat());
    assert!(
        text.contains("Nextsteps") && !text.contains("Roadmap"),
        "{text:?}"
    );

    let e = run("office2pdf", vec![pptx()], r#"{"slides":"3-1"}"#).unwrap_err();
    assert_eq!(e.code(), "invalid_option");
}

#[test]
fn hidden_slides_are_left_out_unless_asked_for() {
    let (_, report) = convert(pptx_hidden_slide());
    assert_eq!(report["pages"], 1);
    let (pdf, report) = convert_with(pptx_hidden_slide(), r#"{"includeHiddenSlides":true}"#);
    assert_eq!(report["pages"], 2);
    assert!(squeeze(&text_of(&pdf)[1]).contains("Nextsteps"));
}

#[test]
fn paper_size_and_orientation_apply() {
    let size = |bytes: &[u8]| {
        let doc = pdf::load(bytes, None).unwrap();
        let page = pdf::page_ids(&doc)[0];
        let b = doc
            .get_dictionary(page)
            .unwrap()
            .get(b"MediaBox")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .map(|o| o.as_float().unwrap_or_else(|_| o.as_i64().unwrap() as f32))
            .collect::<Vec<f32>>();
        ((b[2] - b[0]).round(), (b[3] - b[1]).round())
    };
    // The document's own A4 portrait, then US Letter landscape.
    let (a4, _) = convert(docx());
    assert_eq!(size(&a4), (595.0, 842.0));
    let (letter, _) = convert_with(docx(), r#"{"paperSize":"letter","landscape":true}"#);
    assert_eq!(size(&letter), (792.0, 612.0));
}

#[test]
fn only_pdf_a_carries_a_date() {
    // Plain output has no date, so it is reproducible. PDF/A needs one and
    // office2pdf stamps the wall clock: those bytes change every second.
    let (plain, _) = convert(docx());
    assert!(!contains(&plain, b"CreationDate"));
    let (pdfa, _) = convert_with(docx(), r#"{"pdfA":true}"#);
    assert!(contains(&pdfa, b"CreationDate") || contains(&pdfa, b"xmp:CreateDate"));
    qpdf_check(&pdfa, "pdfa");
}

#[test]
fn refuses_legacy_and_encrypted_files() {
    let e = run("office2pdf", vec![legacy_doc()], "").unwrap_err();
    assert_eq!(e.code(), "unsupported");
    assert_eq!(
        e.to_string(),
        "unsupported: legacy .doc, .xls and .ppt files are not supported; save as .docx, .xlsx or .pptx"
    );
    let mut encrypted = legacy_doc();
    encrypted.extend(ENCRYPTED_PACKAGE);
    let e = run("office2pdf", vec![encrypted], "").unwrap_err();
    assert_eq!(e.code(), "unsupported");
    assert!(e.to_string().contains("password"), "{e}");
}

#[test]
fn refuses_non_office_input() {
    let e = run("office2pdf", vec![b"%PDF-1.7 nope".to_vec()], "").unwrap_err();
    assert_eq!(e.code(), "unsupported");
    let epub = zip(&[("mimetype".into(), b"application/epub+zip".to_vec())]);
    let e = run("office2pdf", vec![epub], "").unwrap_err();
    assert_eq!(e.code(), "unsupported");
    let e = run("office2pdf", vec![], "").unwrap_err();
    assert_eq!(e.code(), "invalid_option");
    let e = run("office2pdf", vec![docx()], r#"{"paperSize":"a9"}"#).unwrap_err();
    assert_eq!(e.code(), "invalid_option");
}
