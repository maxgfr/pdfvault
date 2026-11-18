//! pdfvault-office: Word, Excel and PowerPoint (OOXML) to PDF, built on
//! office2pdf (Office XML → Typst → PDF). Lives in its own lazy wasm module
//! because Typst is large; the core module never pays for it.

use std::io::Cursor;

use office2pdf::config::{
    ConvertOptions, Format, PaperSize, PdfStandard, SlideRange, XlsxUiScript,
};
use office2pdf::error::{ConvertError, ConvertWarning};
use pdfvault_core::dispatch::{OutFile, Output};
use pdfvault_core::{Error, Result};
use serde::Deserialize;
use serde_json::{Value, json};

pub const OPS: &[&str] = &["office2pdf"];

/// Largest office file accepted, before decompression.
const MAX_INPUT: usize = 200 * 1024 * 1024;

const OLE2_MAGIC: [u8; 8] = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];
/// The stream an encrypted OOXML file keeps inside its OLE container
/// ("EncryptedPackage" in UTF-16LE).
const ENCRYPTED_PACKAGE: &[u8] = b"E\0n\0c\0r\0y\0p\0t\0e\0d\0P\0a\0c\0k\0a\0g\0e\0";

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct OfficeOptions {
    /// a4, letter or legal; the document's own size when absent.
    pub paper_size: Option<String>,
    /// Force landscape (true) or portrait (false); the document's own when absent.
    pub landscape: Option<bool>,
    /// Emit PDF/A-2b.
    pub pdf_a: bool,
    /// Emit a tagged (structured) PDF.
    pub tagged: bool,
    /// XLSX: only these sheets, by name.
    pub sheets: Option<Vec<String>>,
    /// PPTX: a slide range such as "3" or "2-5" (1-based).
    pub slides: Option<String>,
    /// PPTX: include slides marked hidden.
    pub include_hidden_slides: bool,
    /// Family appended last to every font chain ("Liberation Sans"); office2pdf's
    /// own when absent.
    pub fallback_font: Option<String>,
}

/// Which OOXML package `bytes` holds, from the parts it contains.
pub fn detect(bytes: &[u8]) -> Result<Format> {
    if bytes.starts_with(&OLE2_MAGIC) {
        let what = if contains(bytes, ENCRYPTED_PACKAGE) {
            "password-protected Office files cannot be converted; remove the password in Office first"
        } else {
            "legacy .doc, .xls and .ppt files are not supported; save as .docx, .xlsx or .pptx"
        };
        return Err(Error::Unsupported(what.into()));
    }
    let archive = zip::ZipArchive::new(Cursor::new(bytes))
        .map_err(|_| Error::Unsupported("not a .docx, .xlsx or .pptx file".into()))?;
    let has = |name: &str| archive.index_for_name(name).is_some();
    if has("word/document.xml") {
        Ok(Format::Docx)
    } else if has("xl/workbook.xml") {
        Ok(Format::Xlsx)
    } else if has("ppt/presentation.xml") {
        Ok(Format::Pptx)
    } else {
        Err(Error::Unsupported(
            "this archive is not a Word, Excel or PowerPoint document".into(),
        ))
    }
}

fn contains(hay: &[u8], needle: &[u8]) -> bool {
    hay.windows(needle.len()).any(|w| w == needle)
}

fn format_name(f: Format) -> &'static str {
    match f {
        Format::Docx => "docx",
        Format::Xlsx => "xlsx",
        Format::Pptx => "pptx",
    }
}

fn convert_options(o: &OfficeOptions, fonts: &[Vec<u8>]) -> Result<ConvertOptions> {
    let paper_size = o
        .paper_size
        .as_deref()
        .map(PaperSize::parse)
        .transpose()
        .map_err(Error::option)?;
    let slide_range = o
        .slides
        .as_deref()
        .map(SlideRange::parse)
        .transpose()
        .map_err(Error::option)?;
    Ok(ConvertOptions {
        sheet_names: o.sheets.clone(),
        // pdfvault's users print from Latin-script Excel far more often than
        // from a Korean one (office2pdf's default).
        xlsx_ui_script: XlsxUiScript::Latin,
        slide_range,
        include_hidden_slides: o.include_hidden_slides,
        pdf_standard: o.pdf_a.then_some(PdfStandard::PdfA2b),
        paper_size,
        font_paths: vec![],
        font_bytes: fonts.to_vec(),
        last_resort_font_family: o.fallback_font.clone(),
        landscape: o.landscape,
        tagged: o.tagged,
        pdf_ua: false,
        streaming: false,
        streaming_chunk_size: None,
    })
}

fn map_error(e: ConvertError) -> Error {
    match e {
        ConvertError::UnsupportedEncryption => Error::Unsupported(
            "password-protected or legacy Office files cannot be converted".into(),
        ),
        ConvertError::UnsupportedFormat(m) => Error::Unsupported(m),
        ConvertError::Parse(m) => Error::Unsupported(format!("could not read the document: {m}")),
        ConvertError::Render(m) => {
            Error::Unsupported(format!("could not lay out the document: {m}"))
        }
        ConvertError::Io(e) => Error::Unsupported(e.to_string()),
    }
}

fn warning_json(w: &ConvertWarning) -> Value {
    let kind = match w {
        ConvertWarning::UnsupportedElement { .. } => "unsupported_element",
        ConvertWarning::PartialElement { .. } => "partial_element",
        ConvertWarning::FallbackUsed { .. } => "fallback_used",
        ConvertWarning::ParseSkipped { .. } => "parse_skipped",
    };
    json!({ "kind": kind, "message": w.to_string() })
}

/// Convert one OOXML document. `fonts` are TrueType/OpenType files the
/// document may draw with; nothing is read from the host.
pub fn office_to_pdf(
    bytes: &[u8],
    fonts: &[Vec<u8>],
    o: &OfficeOptions,
) -> Result<(Vec<u8>, Value)> {
    if bytes.len() > MAX_INPUT {
        return Err(Error::Limit(format!(
            "office files are limited to {} MiB",
            MAX_INPUT / 1024 / 1024
        )));
    }
    let format = detect(bytes)?;
    let options = convert_options(o, fonts)?;
    let result = office2pdf::convert_bytes(bytes, format, &options).map_err(map_error)?;
    let pages = match &result.metrics {
        Some(m) => m.page_count as usize,
        None => pdfvault_core::pdf::page_ids(&pdfvault_core::pdf::load(&result.pdf, None)?).len(),
    };
    let warnings: Vec<Value> = result.warnings.iter().map(warning_json).collect();
    let report = json!({
        "format": format_name(format),
        "pages": pages,
        "warnings": warnings,
    });
    Ok((result.pdf, report))
}

/// Same contract as [`pdfvault_core::dispatch::run`]. `office2pdf` takes the
/// document first, then any number of font files.
pub fn run(op: &str, inputs: Vec<Vec<u8>>, options: &str) -> Result<Output> {
    let v: Value = if options.trim().is_empty() {
        Value::Null
    } else {
        serde_json::from_str(options).map_err(|e| Error::option(e.to_string()))?
    };
    match op {
        "office2pdf" => {
            let o: OfficeOptions = if v.is_null() {
                OfficeOptions::default()
            } else {
                serde_json::from_value(v).map_err(|e| Error::option(e.to_string()))?
            };
            let (doc, fonts) = inputs
                .split_first()
                .ok_or_else(|| Error::option("office2pdf takes the document as its first input"))?;
            let (pdf, report) = office_to_pdf(doc, fonts, &o)?;
            Ok(Output {
                files: vec![OutFile {
                    suffix: String::new(),
                    ext: "pdf".into(),
                    bytes: pdf,
                }],
                report,
            })
        }
        other => Err(Error::option(format!("unknown operation '{other}'"))),
    }
}

#[cfg(test)]
mod tests;
