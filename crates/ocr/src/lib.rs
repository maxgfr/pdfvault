//! pdfvault-ocr: text recognition on rendered pages with the PP-OCRv6 small
//! models (Apache-2.0, PaddlePaddle), 50 languages: Latin scripts with their
//! accents, Chinese and Japanese. The ONNX models and the character
//! dictionary are loaded from bytes the caller supplies; nothing is fetched
//! at run time.
//!
//! Detection is DB (a text probability map, thresholded into boxes, each
//! grown back to the text it covers); recognition reads each box as one line
//! with greedy CTC decoding. Word boxes come from where each character's
//! frames fall along the line.

use rten::Model;
use rten_tensor::NdTensor;
use rten_tensor::prelude::*;
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

// Post-processing parameters from the models' inference.yml.
const DET_THRESH: f32 = 0.2;
const BOX_THRESH: f32 = 0.45;
const UNCLIP_RATIO: f32 = 1.4;
const MIN_BOX: f32 = 3.0;
/// The detector sees the page at its own resolution, up to this side.
const DET_MAX_SIDE: f32 = 2400.0;
const DET_MEAN: [f32; 3] = [0.485, 0.456, 0.406];
const DET_STD: [f32; 3] = [0.229, 0.224, 0.225];
const REC_HEIGHT: usize = 48;
const REC_MAX_WIDTH: usize = 3200;
/// Lines read with less mean confidence than this are noise, not text.
const REC_MIN_SCORE: f32 = 0.5;

pub struct Ocr {
    det: Model,
    rec: Model,
    /// CTC classes: 0 is the blank, then the dictionary, then a space.
    classes: Vec<String>,
}

/// A text line in input pixels, `x1`/`y1` exclusive.
#[derive(Debug, Clone, Copy)]
struct Rect {
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
}

impl Ocr {
    /// `config` is the recognizer's inference.yml, which holds its characters.
    pub fn new(detection: Vec<u8>, recognition: Vec<u8>, config: &str) -> Result<Self, String> {
        let det = Model::load(detection).map_err(|e| format!("detection model: {e}"))?;
        let rec = Model::load(recognition).map_err(|e| format!("recognition model: {e}"))?;
        let mut classes = vec![String::new()];
        classes.extend(character_dict(config));
        if classes.len() < 2 {
            return Err("recognition config has no character_dict".into());
        }
        classes.push(" ".into());
        Ok(Ocr { det, rec, classes })
    }

    /// Recognize text in an RGBA (or RGB, or gray) image, lines in reading order.
    pub fn recognize(&self, pixels: &[u8], width: u32, height: u32) -> Result<Vec<Line>, String> {
        let img = Image::new(pixels, width as usize, height as usize)?;
        let mut lines = Vec::new();
        for rect in self.detect(&img)? {
            if let Some(line) = self.read_line(&img, rect)? {
                lines.push(line);
            }
        }
        Ok(lines)
    }

    fn detect(&self, img: &Image) -> Result<Vec<Rect>, String> {
        let scale = (DET_MAX_SIDE / img.w.max(img.h) as f32).min(1.0);
        let round32 = |v: usize| ((v as f32 * scale / 32.0).round() as usize).max(1) * 32;
        let (dw, dh) = (round32(img.w), round32(img.h));
        let full = Rect {
            x0: 0.0,
            y0: 0.0,
            x1: img.w as f32,
            y1: img.h as f32,
        };
        let input = img.sample(full, dw, dh, |c, v| (v / 255.0 - DET_MEAN[c]) / DET_STD[c]);
        let out: NdTensor<f32, 4> = self
            .det
            .run_one(input.view().into(), None)
            .map_err(|e| format!("detection: {e}"))?
            .try_into()
            .map_err(|_| "detection: unexpected output".to_string())?;
        let [_, _, mh, mw] = out.shape();
        let prob = out.to_vec();
        let (sx, sy) = (img.w as f32 / mw as f32, img.h as f32 / mh as f32);

        let mut rects: Vec<Rect> = components(&prob, mw, mh)
            .into_iter()
            .filter_map(|c| {
                let (w, h) = ((c.x1 - c.x0 + 1) as f32, (c.y1 - c.y0 + 1) as f32);
                if w.min(h) < MIN_BOX || c.score < BOX_THRESH {
                    return None;
                }
                // DB detects a shrunk core of each text; grow it back.
                let d = w * h * UNCLIP_RATIO / (2.0 * (w + h));
                let r = Rect {
                    x0: ((c.x0 as f32 - d) * sx).max(0.0),
                    y0: ((c.y0 as f32 - d) * sy).max(0.0),
                    x1: ((c.x1 as f32 + 1.0 + d) * sx).min(img.w as f32),
                    y1: ((c.y1 as f32 + 1.0 + d) * sy).min(img.h as f32),
                };
                (r.x1 - r.x0 >= MIN_BOX && r.y1 - r.y0 >= MIN_BOX).then_some(r)
            })
            .collect();
        // Reading order: top to bottom, left to right within a row.
        rects.sort_by(|a, b| a.y0.total_cmp(&b.y0));
        let mut ordered: Vec<Rect> = Vec::with_capacity(rects.len());
        let mut row: Vec<Rect> = Vec::new();
        for r in rects {
            let same_row = row.first().is_some_and(|f| {
                let overlap = f.y1.min(r.y1) - f.y0.max(r.y0);
                overlap > 0.5 * (f.y1 - f.y0).min(r.y1 - r.y0)
            });
            if !same_row {
                row.sort_by(|a, b| a.x0.total_cmp(&b.x0));
                ordered.append(&mut row);
            }
            row.push(r);
        }
        row.sort_by(|a, b| a.x0.total_cmp(&b.x0));
        ordered.append(&mut row);
        Ok(ordered)
    }

    fn read_line(&self, img: &Image, r: Rect) -> Result<Option<Line>, String> {
        let (cw, ch) = (r.x1 - r.x0, r.y1 - r.y0);
        let width =
            ((REC_HEIGHT as f32 * cw / ch).round() as usize).clamp(REC_HEIGHT / 3, REC_MAX_WIDTH);
        let input = img.sample(r, width, REC_HEIGHT, |_, v| v / 127.5 - 1.0);
        let out: NdTensor<f32, 3> = self
            .rec
            .run_one(input.view().into(), None)
            .map_err(|e| format!("recognition: {e}"))?
            .try_into()
            .map_err(|_| "recognition: unexpected output".to_string())?;
        let [_, frames, n] = out.shape();
        if n != self.classes.len() {
            return Err(format!(
                "recognition model has {n} classes, the dictionary {}",
                self.classes.len()
            ));
        }
        let probs = out.to_vec();

        // Greedy CTC: best class per frame, repeats merged, blanks dropped.
        // Each character keeps the frames it spans, for word positions.
        let mut chars: Vec<(usize, usize, usize)> = Vec::new(); // (class, first frame, last frame)
        let (mut score, mut prev) = (0.0, 0);
        for t in 0..frames {
            let row = &probs[t * n..(t + 1) * n];
            let (best, p) =
                row.iter().enumerate().fold(
                    (0, f32::MIN),
                    |acc, (i, &p)| if p > acc.1 { (i, p) } else { acc },
                );
            if best != 0 {
                match chars.last_mut() {
                    Some(last) if best == prev => last.2 = t,
                    _ => {
                        chars.push((best, t, t));
                        score += p;
                    }
                }
            }
            prev = best;
        }
        if chars.is_empty() || score / (chars.len() as f32) < REC_MIN_SCORE {
            return Ok(None);
        }

        // A character owns the frames up to halfway to its neighbours; a word
        // spans its characters, then shrinks to the ink inside that span.
        let k = cw / frames as f32;
        let left = |j: usize| {
            if j == 0 {
                0.0
            } else {
                (chars[j - 1].2 + 1 + chars[j].1) as f32 / 2.0
            }
        };
        let right = |j: usize| {
            chars
                .get(j + 1)
                .map_or(frames as f32, |n| (chars[j].2 + 1 + n.1) as f32 / 2.0)
        };
        let ink = Ink::new(img, r);
        let mut words = Vec::new();
        let mut start: Option<usize> = None;
        for j in 0..=chars.len() {
            let space = chars
                .get(j)
                .is_none_or(|c| self.classes[c.0].trim().is_empty());
            match (space, start) {
                (false, None) => start = Some(j),
                (true, Some(s)) => {
                    let text: String = chars[s..j]
                        .iter()
                        .map(|c| self.classes[c.0].as_str())
                        .collect();
                    let (x0, x1) = (r.x0 + left(s) * k, r.x0 + right(j - 1) * k);
                    let b = ink.tighten(x0, x1).unwrap_or(Rect {
                        x0,
                        y0: r.y0,
                        x1,
                        y1: r.y1,
                    });
                    words.push(Word {
                        text,
                        x: b.x0,
                        y: b.y0,
                        w: b.x1 - b.x0,
                        h: b.y1 - b.y0,
                    });
                    start = None;
                }
                _ => {}
            }
        }
        if words.is_empty() {
            return Ok(None);
        }
        // One baseline per line: every word gets the line's ink height, or a
        // word without descenders would sit on a line of its own.
        let y0 = words.iter().map(|w| w.y).fold(f32::MAX, f32::min);
        let y1 = words.iter().map(|w| w.y + w.h).fold(f32::MIN, f32::max);
        for w in &mut words {
            (w.y, w.h) = (y0, y1 - y0);
        }
        let text = words
            .iter()
            .map(|w| w.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        Ok(Some(Line { text, words }))
    }
}

/// The `character_dict` list of a PaddleOCR inference.yml, whose items are
/// plain or single-quoted scalars (`''` is a quote).
fn character_dict(yml: &str) -> Vec<String> {
    yml.lines()
        .skip_while(|l| l.trim_end() != "  character_dict:")
        .skip(1)
        .map_while(|l| l.strip_prefix("  - "))
        .map(
            |v| match v.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')) {
                Some(quoted) => quoted.replace("''", "'"),
                None => v.to_string(),
            },
        )
        .collect()
}

/// The input as packed RGB.
struct Image {
    rgb: Vec<u8>,
    w: usize,
    h: usize,
}

impl Image {
    fn new(pixels: &[u8], w: usize, h: usize) -> Result<Self, String> {
        let n = w * h;
        if n == 0 {
            return Err("empty image".into());
        }
        let rgb = match pixels.len() / n {
            _ if !pixels.len().is_multiple_of(n) => {
                return Err("pixel buffer does not match the size".into());
            }
            1 => pixels.iter().flat_map(|&g| [g, g, g]).collect(),
            3 => pixels.to_vec(),
            4 => pixels
                .as_chunks::<4>()
                .0
                .iter()
                .flat_map(|p| [p[0], p[1], p[2]])
                .collect(),
            c => return Err(format!("unsupported {c}-channel image")),
        };
        Ok(Image { rgb, w, h })
    }

    /// Bilinear resample of `r` to `ow`×`oh` as a [1, 3, oh, ow] tensor in
    /// BGR order (the models were trained on BGR), each value mapped by `f`.
    fn sample(
        &self,
        r: Rect,
        ow: usize,
        oh: usize,
        f: impl Fn(usize, f32) -> f32,
    ) -> NdTensor<f32, 4> {
        let mut data = vec![0.0; 3 * ow * oh];
        let (sx, sy) = ((r.x1 - r.x0) / ow as f32, (r.y1 - r.y0) / oh as f32);
        let (maxx, maxy) = (self.w - 1, self.h - 1);
        for oy in 0..oh {
            let fy = (r.y0 + (oy as f32 + 0.5) * sy - 0.5).clamp(0.0, maxy as f32);
            let (y0, ty) = (fy as usize, fy.fract());
            let y1 = (y0 + 1).min(maxy);
            for ox in 0..ow {
                let fx = (r.x0 + (ox as f32 + 0.5) * sx - 0.5).clamp(0.0, maxx as f32);
                let (x0, tx) = (fx as usize, fx.fract());
                let x1 = (x0 + 1).min(maxx);
                for c in 0..3 {
                    let px = |x: usize, y: usize| self.rgb[(y * self.w + x) * 3 + (2 - c)] as f32;
                    let top = px(x0, y0) * (1.0 - tx) + px(x1, y0) * tx;
                    let bottom = px(x0, y1) * (1.0 - tx) + px(x1, y1) * tx;
                    data[(c * oh + oy) * ow + ox] = f(c, top * (1.0 - ty) + bottom * ty);
                }
            }
        }
        NdTensor::from_data([1, 3, oh, ow], data)
    }
}

/// Which pixels of a line box are ink: darker than halfway between the
/// box's lightest and darkest, when the box has contrast at all.
struct Ink {
    x0: usize,
    y0: usize,
    w: usize,
    h: usize,
    mask: Vec<bool>,
}

impl Ink {
    fn new(img: &Image, r: Rect) -> Self {
        let (x0, y0) = (r.x0 as usize, r.y0 as usize);
        let (w, h) = (
            (r.x1.ceil() as usize).min(img.w) - x0,
            (r.y1.ceil() as usize).min(img.h) - y0,
        );
        let gray: Vec<u8> = (0..w * h)
            .map(|i| {
                let p = &img.rgb[((y0 + i / w) * img.w + x0 + i % w) * 3..][..3];
                ((p[0] as u16 * 3 + p[1] as u16 * 6 + p[2] as u16) / 10) as u8
            })
            .collect();
        let (lo, hi) = gray
            .iter()
            .fold((255, 0), |(lo, hi), &g| (g.min(lo), g.max(hi)));
        let mask = if hi - lo < 40 {
            Vec::new()
        } else {
            let mid = (lo as u16 + hi as u16) / 2;
            gray.iter().map(|&g| (g as u16) < mid).collect()
        };
        Ink { x0, y0, w, h, mask }
    }

    /// The ink's bounds between `x0` and `x1` (input pixels), if any.
    fn tighten(&self, x0: f32, x1: f32) -> Option<Rect> {
        if self.mask.is_empty() {
            return None;
        }
        let a = (x0 as usize).saturating_sub(self.x0).min(self.w);
        let b = (x1.ceil() as usize).saturating_sub(self.x0).min(self.w);
        let (mut bx0, mut by0, mut bx1, mut by1) = (usize::MAX, usize::MAX, 0, 0);
        for y in 0..self.h {
            for x in a..b {
                if self.mask[y * self.w + x] {
                    bx0 = bx0.min(x);
                    bx1 = bx1.max(x + 1);
                    by0 = by0.min(y);
                    by1 = by1.max(y + 1);
                }
            }
        }
        (bx0 < bx1).then(|| Rect {
            x0: (self.x0 + bx0) as f32,
            y0: (self.y0 + by0) as f32,
            x1: (self.x0 + bx1) as f32,
            y1: (self.y0 + by1) as f32,
        })
    }
}

/// A connected region of the thresholded map: bounds (inclusive) and mean probability.
struct Component {
    x0: usize,
    y0: usize,
    x1: usize,
    y1: usize,
    score: f32,
}

/// 8-connected regions of `prob > DET_THRESH`.
fn components(prob: &[f32], w: usize, h: usize) -> Vec<Component> {
    let mut seen = vec![false; w * h];
    let mut out = Vec::new();
    let mut stack = Vec::new();
    for start in 0..w * h {
        if seen[start] || prob[start] <= DET_THRESH {
            continue;
        }
        seen[start] = true;
        stack.push(start);
        let (mut c, mut sum, mut count) = (
            Component {
                x0: usize::MAX,
                y0: usize::MAX,
                x1: 0,
                y1: 0,
                score: 0.0,
            },
            0.0,
            0usize,
        );
        while let Some(i) = stack.pop() {
            let (x, y) = (i % w, i / w);
            c.x0 = c.x0.min(x);
            c.x1 = c.x1.max(x);
            c.y0 = c.y0.min(y);
            c.y1 = c.y1.max(y);
            sum += prob[i];
            count += 1;
            for ny in y.saturating_sub(1)..=(y + 1).min(h - 1) {
                for nx in x.saturating_sub(1)..=(x + 1).min(w - 1) {
                    let j = ny * w + nx;
                    if !seen[j] && prob[j] > DET_THRESH {
                        seen[j] = true;
                        stack.push(j);
                    }
                }
            }
        }
        c.score = sum / count as f32;
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn ocr() -> Ocr {
        let dir = std::env::var("PDFVAULT_MODELS")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../models"));
        let read = |name: &str| {
            std::fs::read(dir.join(name))
                .unwrap_or_else(|_| panic!("{name} missing: run `bun run fetch-models` first"))
        };
        let config = String::from_utf8(read("ppocrv6-small-rec.yml")).unwrap();
        Ocr::new(
            read("ppocrv6-small-det.onnx"),
            read("ppocrv6-small-rec.onnx"),
            &config,
        )
        .unwrap()
    }

    #[test]
    fn reads_a_rendered_page() {
        let ocr = ocr();
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
    fn reads_accents() {
        use pdfvault_core::lopdf::content::Operation;
        use pdfvault_core::lopdf::{Object, StringFormat};
        let ocr = ocr();
        let mut b = pdfvault_core::fixtures::Builder::new();
        // WinAnsi bytes: "Été à Noël, garçon déçu."
        let line = b"\xC9t\xE9 \xE0 No\xEBl, gar\xE7on d\xE9\xE7u.".to_vec();
        b.page(
            (612.0, 200.0),
            vec![
                Operation::new("BT", vec![]),
                Operation::new("Tf", vec!["F1".into(), 18.into()]),
                Operation::new("Td", vec![72.into(), 100.into()]),
                Operation::new("Tj", vec![Object::String(line, StringFormat::Literal)]),
                Operation::new("ET", vec![]),
            ],
            vec![],
        );
        let doc = pdfvault_render::open(&b.finish(), None).unwrap();
        let img = pdfvault_render::render_page(&doc, 0, 200.0).unwrap();
        let lines = ocr
            .recognize(img.as_raw(), img.width(), img.height())
            .unwrap();
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert_eq!(lines[0].text, "Été à Noël, garçon déçu.");
        assert_eq!(lines[0].words.len(), 5);
    }

    #[test]
    fn parses_the_character_dict() {
        let yml = "PostProcess:\n  name: CTCLabelDecode\n  character_dict:\n  - '!'\n  - ''''\n  - a\n  - é\n  - '\u{3000}'\nPreProcess:\n";
        assert_eq!(character_dict(yml), ["!", "'", "a", "é", "\u{3000}"]);
        // Every character, plus the CTC blank and the space.
        assert_eq!(ocr().classes.len(), 18_710);
    }

    #[test]
    fn rejects_bad_models() {
        assert!(Ocr::new(vec![1, 2, 3], vec![4, 5, 6], "a").is_err());
    }
}
