//! The one entry point the wasm façade, and through it the CLI and the site,
//! call: `run(op, inputs, options_json)`. Keeping a single door means both
//! front ends run byte-for-byte the same code.

use crate::error::{Error, Result};
use crate::*;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

/// One produced file. `suffix` and `ext` let the caller name it, e.g.
/// `report.pdf` → `report-compressed.pdf`.
#[derive(Debug, Clone)]
pub struct OutFile {
    pub suffix: String,
    pub ext: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, Default)]
pub struct Output {
    pub files: Vec<OutFile>,
    pub report: Value,
}

fn file(suffix: &str, ext: &str, bytes: Vec<u8>) -> OutFile {
    OutFile {
        suffix: suffix.into(),
        ext: ext.into(),
        bytes,
    }
}

fn pdf(suffix: &str, bytes: Vec<u8>) -> Output {
    Output {
        files: vec![file(suffix, "pdf", bytes)],
        report: Value::Null,
    }
}

fn parse<T: DeserializeOwned + Default>(options: &Value) -> Result<T> {
    if options.is_null() {
        return Ok(T::default());
    }
    serde_json::from_value(options.clone()).map_err(|e| Error::option(e.to_string()))
}

fn one(inputs: &[Vec<u8>], op: &str) -> Result<Vec<u8>> {
    match inputs {
        [only] => Ok(only.clone()),
        _ => Err(Error::option(format!(
            "{op} takes exactly one input, got {}",
            inputs.len()
        ))),
    }
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct Common {
    password: Option<String>,
    /// A page selection string for most ops; other shapes belong to the op.
    pages: Option<Value>,
    angle: Option<i64>,
}

impl Common {
    fn page_spec(&self) -> Option<String> {
        self.pages
            .as_ref()
            .and_then(Value::as_str)
            .map(str::to_owned)
    }
}

/// Names of every operation, for the façades' self-description.
pub const OPS: &[&str] = &[
    "compress",
    "merge",
    "select",
    "delete",
    "split",
    "rotate",
    "blank",
    "crop",
    "nup",
    "watermark",
    "number",
    "header-footer",
    "stamp",
    "edit",
    "arrange",
    "form-fields",
    "meta-read",
    "meta",
    "encrypt",
    "decrypt",
    "security",
    "flatten",
    "repair",
    "info",
    "img2pdf",
    "image",
    "image-compress",
    "image-strip",
    "rebuild-raster",
    "text-layer",
    "markdown",
    "epub",
    "kdf",
    "zip",
];

pub fn run(op: &str, inputs: Vec<Vec<u8>>, options: &str) -> Result<Output> {
    let options: Value = if options.trim().is_empty() {
        Value::Null
    } else {
        serde_json::from_str(options)?
    };
    let c: Common = parse(&options)?;
    let pw = c.password.as_deref();
    Ok(match op {
        "compress" => {
            let o: compress::CompressOptions = parse(&options)?;
            let (bytes, report) = compress::compress(&one(&inputs, op)?, &o)?;
            Output {
                files: vec![file("compressed", "pdf", bytes)],
                report: serde_json::to_value(report)?,
            }
        }
        "merge" => pdf("merged", pages::merge(&inputs, pw)?),
        "select" => {
            let spec = c
                .page_spec()
                .ok_or_else(|| Error::option("pages is required"))?;
            pdf("pages", pages::select(&one(&inputs, op)?, &spec, pw)?)
        }
        "delete" => {
            let spec = c
                .page_spec()
                .ok_or_else(|| Error::option("pages is required"))?;
            pdf("deleted", pages::delete(&one(&inputs, op)?, &spec, pw)?)
        }
        "split" => {
            let mode: pages::SplitMode = serde_json::from_value(options.clone())
                .map_err(|e| Error::option(e.to_string()))?;
            let parts = pages::split(&one(&inputs, op)?, &mode, pw)?;
            let names: Vec<String> = parts.iter().map(|p| p.0.clone()).collect();
            Output {
                files: parts
                    .into_iter()
                    .map(|(name, bytes)| file(&format!("part-{name}"), "pdf", bytes))
                    .collect(),
                report: json!({ "parts": names }),
            }
        }
        "rotate" => pdf(
            "rotated",
            pages::rotate(
                &one(&inputs, op)?,
                c.angle.unwrap_or(90),
                c.page_spec().as_deref(),
                pw,
            )?,
        ),
        "blank" => pdf(
            "blank",
            pages::insert_blank(&one(&inputs, op)?, &parse(&options)?)?,
        ),
        "crop" => pdf(
            "cropped",
            pages::crop(&one(&inputs, op)?, &parse(&options)?)?,
        ),
        "nup" => pdf("nup", pages::nup(&one(&inputs, op)?, &parse(&options)?)?),
        "watermark" => pdf(
            "watermarked",
            stamp::watermark(&one(&inputs, op)?, &parse(&options)?)?,
        ),
        "number" => pdf(
            "numbered",
            stamp::number(&one(&inputs, op)?, &parse(&options)?)?,
        ),
        "header-footer" => pdf(
            "header-footer",
            stamp::header_footer(&one(&inputs, op)?, &parse(&options)?)?,
        ),
        "stamp" => match inputs.as_slice() {
            [doc, img] => pdf("signed", stamp::stamp_image(doc, img, &parse(&options)?)?),
            _ => return Err(Error::option("stamp takes a PDF and an image")),
        },
        "edit" => {
            let (bytes, report) = edit::edit(&inputs, &parse(&options)?)?;
            Output {
                files: vec![file("edited", "pdf", bytes)],
                report: serde_json::to_value(report)?,
            }
        }
        "arrange" => pdf(
            "arranged",
            pages::arrange(&one(&inputs, op)?, &parse(&options)?)?,
        ),
        "form-fields" => Output {
            files: vec![],
            report: json!({ "fields": forms::list_fields(&one(&inputs, op)?, pw)? }),
        },
        "meta-read" => {
            let doc = pdf::load(&one(&inputs, op)?, pw)?;
            Output {
                files: vec![],
                report: serde_json::to_value(meta::read(&doc))?,
            }
        }
        "meta" => pdf(
            "metadata",
            meta::edit(&one(&inputs, op)?, &parse(&options)?)?,
        ),
        "encrypt" => pdf(
            "protected",
            security::encrypt(&one(&inputs, op)?, &parse(&options)?)?,
        ),
        "decrypt" => {
            let p = pw.ok_or_else(|| Error::option("password is required"))?;
            pdf("unlocked", security::decrypt(&one(&inputs, op)?, p)?)
        }
        "security" => Output {
            files: vec![],
            report: serde_json::to_value(security::inspect(&one(&inputs, op)?)?)?,
        },
        "flatten" => {
            let (bytes, report) = forms::flatten(&one(&inputs, op)?, pw)?;
            Output {
                files: vec![file("flattened", "pdf", bytes)],
                report: serde_json::to_value(report)?,
            }
        }
        "repair" => {
            let (bytes, report) = info::repair(&one(&inputs, op)?, pw)?;
            Output {
                files: vec![file("repaired", "pdf", bytes)],
                report: serde_json::to_value(report)?,
            }
        }
        "info" => Output {
            files: vec![],
            report: serde_json::to_value(info::info(&one(&inputs, op)?, pw)?)?,
        },
        "img2pdf" => pdf("images", images::img2pdf(&inputs, &parse(&options)?)?),
        "image" | "image-compress" => {
            let o: images::ImageOptions = parse(&options)?;
            let (bytes, report) =
                images::transform(&one(&inputs, op)?, &o, op == "image-compress")?;
            let ext = report.format.map(|f| f.extension()).unwrap_or("bin");
            let suffix = if op == "image-compress" {
                "compressed"
            } else {
                "edited"
            };
            Output {
                files: vec![file(suffix, ext, bytes)],
                report: serde_json::to_value(report)?,
            }
        }
        "image-strip" => {
            let (bytes, report) = images::strip(&one(&inputs, op)?)?;
            let ext = report.format.map(|f| f.extension()).unwrap_or("bin");
            Output {
                files: vec![file("clean", ext, bytes)],
                report: serde_json::to_value(report)?,
            }
        }
        "rebuild-raster" => {
            #[derive(Deserialize, Default)]
            #[serde(default)]
            struct O {
                password: Option<String>,
                pages: Vec<compress::PageImage>,
            }
            let o: O = parse(&options)?;
            let (src, jpegs) = inputs
                .split_first()
                .ok_or_else(|| Error::option("rebuild-raster needs the PDF first"))?;
            if jpegs.len() != o.pages.len() {
                return Err(Error::option("one JPEG per page description is required"));
            }
            let pages: Vec<_> = o.pages.into_iter().zip(jpegs.iter().cloned()).collect();
            pdf(
                "compressed",
                compress::rebuild_from_page_images(src, o.password.as_deref(), &pages)?,
            )
        }
        "text-layer" => {
            #[derive(Deserialize, Default)]
            #[serde(default)]
            struct O {
                password: Option<String>,
                pages: Vec<textlayer::OcrPage>,
            }
            let o: O = parse(&options)?;
            pdf(
                "searchable",
                textlayer::add_text_layer(&one(&inputs, op)?, o.password.as_deref(), &o.pages)?,
            )
        }
        "markdown" => {
            #[derive(Deserialize, Default)]
            #[serde(default)]
            struct O {
                pages: Vec<markdown::PageText>,
                #[serde(flatten)]
                options: markdown::MarkdownOptions,
            }
            let o: O = parse(&options)?;
            let md = markdown::to_markdown(&o.pages, &o.options);
            Output {
                files: vec![file("text", "md", md.into_bytes())],
                report: Value::Null,
            }
        }
        "epub" => {
            let o: epub::EpubOptions = parse(&options)?;
            let md = String::from_utf8(one(&inputs, op)?)
                .map_err(|_| Error::option("the Markdown input is not UTF-8"))?;
            Output {
                files: vec![file("book", "epub", epub::build(&md, &o))],
                report: Value::Null,
            }
        }
        "kdf" => {
            let key = kdf::derive_key(&one(&inputs, op)?, &parse(&options)?)?;
            Output {
                files: vec![file("key", "bin", key.to_vec())],
                report: Value::Null,
            }
        }
        "zip" => {
            #[derive(Deserialize, Default)]
            #[serde(default)]
            struct O {
                names: Vec<String>,
            }
            let o: O = parse(&options)?;
            if o.names.len() != inputs.len() || inputs.is_empty() {
                return Err(Error::option("zip needs one name per input"));
            }
            let mut seen = std::collections::HashSet::new();
            for n in &o.names {
                if n.is_empty()
                    || n.contains('/')
                    || n.contains('\\')
                    || n.starts_with('.')
                    || !seen.insert(n)
                {
                    return Err(Error::option(format!("bad or duplicate file name '{n}'")));
                }
            }
            let files: Vec<(String, Vec<u8>)> = o.names.into_iter().zip(inputs).collect();
            Output {
                files: vec![file("files", "zip", epub::zip(&files))],
                report: Value::Null,
            }
        }
        other => return Err(Error::option(format!("unknown operation '{other}'"))),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures;

    #[test]
    fn dispatches() {
        let out = run(
            "compress",
            vec![fixtures::photo_pdf()],
            r#"{"preset":"strong"}"#,
        )
        .unwrap();
        assert_eq!(out.files.len(), 1);
        assert_eq!(out.report["preset"], "strong");
        let out = run(
            "split",
            vec![fixtures::text_pdf(3)],
            r#"{"mode":"every","every":1}"#,
        )
        .unwrap();
        assert_eq!(out.files.len(), 3);
        assert_eq!(out.files[2].suffix, "part-3-3");
        let info = run("info", vec![fixtures::text_pdf(2)], "").unwrap();
        assert_eq!(info.report["pages"], 2);
        assert!(run("nope", vec![], "").is_err());
        assert_eq!(
            run("compress", vec![], "").unwrap_err().code(),
            "invalid_option"
        );
        assert_eq!(
            run("compress", vec![b"junk".to_vec()], "")
                .unwrap_err()
                .code(),
            "invalid_pdf"
        );
        assert_eq!(
            run("rotate", vec![fixtures::text_pdf(1)], r#"{"angle": "x"}"#)
                .unwrap_err()
                .code(),
            "invalid_option"
        );
    }

    #[test]
    fn every_op_is_listed() {
        for op in OPS {
            let err = run(op, vec![], "{}").err();
            assert!(
                !matches!(&err, Some(Error::InvalidOption(m)) if m.starts_with("unknown operation")),
                "{op}"
            );
        }
    }

    #[test]
    fn markdown_then_epub() {
        let md = run(
            "markdown",
            vec![],
            r#"{"pages":[{"spans":[{"text":"Title","x":0,"y":0,"size":20},{"text":"Body.","x":0,"y":40,"size":10}]}]}"#,
        )
        .unwrap();
        let text = String::from_utf8(md.files[0].bytes.clone()).unwrap();
        assert_eq!(text, "# Title\n\nBody.\n");
        let e = run("epub", vec![text.into_bytes()], r#"{"title":"T"}"#).unwrap();
        assert!(e.files[0].bytes.starts_with(b"PK"));
        let z = run(
            "zip",
            vec![b"a".to_vec(), b"b".to_vec()],
            r#"{"names":["a.txt","b.txt"]}"#,
        )
        .unwrap();
        assert!(z.files[0].bytes.starts_with(b"PK"));
        assert!(run("zip", vec![b"a".to_vec()], r#"{"names":["../x"]}"#).is_err());
    }
}
