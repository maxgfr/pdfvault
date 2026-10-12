//! pdfvault-ocr: text recognition on rendered pages with the ocrs models.
//! The models (CC-BY-SA 4.0, by Robert Knight) are loaded from bytes the
//! caller supplies; nothing is fetched at run time.

use ocrs::{ImageSource, OcrEngine, OcrEngineParams, TextItem};
use rten::Model;
use serde::Serialize;

/// One recognized word, in pixels of the input image (origin top-left).
#[derive(Debug, Clone, Serialize)]
pub struct Word {
    pub text: String,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

#[derive(Debug, Clone, Serialize)]
pub struct Line {
    pub text: String,
    pub words: Vec<Word>,
}

pub struct Ocr {
    engine: OcrEngine,
}

impl Ocr {
    pub fn new(detection: Vec<u8>, recognition: Vec<u8>) -> Result<Self, String> {
        let detection_model =
            Model::load(detection).map_err(|e| format!("detection model: {e}"))?;
        let recognition_model =
            Model::load(recognition).map_err(|e| format!("recognition model: {e}"))?;
        let engine = OcrEngine::new(OcrEngineParams {
            detection_model: Some(detection_model),
            recognition_model: Some(recognition_model),
            ..Default::default()
        })
        .map_err(|e| e.to_string())?;
        Ok(Ocr { engine })
    }

    /// Recognize text in an RGBA (or RGB, or gray) image, lines in reading order.
    pub fn recognize(&self, pixels: &[u8], width: u32, height: u32) -> Result<Vec<Line>, String> {
        let source = ImageSource::from_bytes(pixels, (width, height)).map_err(|e| e.to_string())?;
        let input = self
            .engine
            .prepare_input(source)
            .map_err(|e| e.to_string())?;
        let words = self
            .engine
            .detect_words(&input)
            .map_err(|e| e.to_string())?;
        let lines = self.engine.find_text_lines(&input, &words);
        let recognized = self
            .engine
            .recognize_text(&input, &lines)
            .map_err(|e| e.to_string())?;
        Ok(recognized
            .into_iter()
            .flatten()
            .map(|line| {
                let words = line
                    .words()
                    .filter_map(|w| {
                        let text = w.to_string().trim().to_string();
                        if text.is_empty() {
                            return None;
                        }
                        let r = w.bounding_rect();
                        Some(Word {
                            text,
                            x: r.left() as f32,
                            y: r.top() as f32,
                            w: r.width() as f32,
                            h: r.height() as f32,
                        })
                    })
                    .collect::<Vec<_>>();
                Line {
                    text: line.to_string().trim().to_string(),
                    words,
                }
            })
            .filter(|l| !l.words.is_empty())
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn models() -> (Vec<u8>, Vec<u8>) {
        let dir = std::env::var("PDFVAULT_MODELS")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../models"));
        let read = |name: &str| {
            std::fs::read(dir.join(name))
                .unwrap_or_else(|_| panic!("{name} missing: run `bun run fetch-models` first"))
        };
        (read("text-detection.rten"), read("text-recognition.rten"))
    }

    #[test]
    fn reads_a_rendered_page() {
        let (det, rec) = models();
        let ocr = Ocr::new(det, rec).unwrap();
        let pdf = pdfvault_core::fixtures::text_pdf(1);
        let doc = pdfvault_render::open(&pdf, None).unwrap();
        let img = pdfvault_render::render_page(&doc, 0, 150.0).unwrap();
        let lines = ocr
            .recognize(img.as_raw(), img.width(), img.height())
            .unwrap();
        let text: Vec<&str> = lines.iter().map(|l| l.text.as_str()).collect();
        assert_eq!(text[0], "Page 1", "{text:?}");
        assert!(text[1].contains("quick brown fox"), "{text:?}");
        let first = &lines[0].words[0];
        // "Page 1" is drawn at x = 72pt = 150px at 150 DPI.
        assert!((first.x - 150.0).abs() < 12.0, "{first:?}");
    }

    #[test]
    fn rejects_bad_models() {
        assert!(Ocr::new(vec![1, 2, 3], vec![4, 5, 6]).is_err());
    }
}
