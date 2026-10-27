//! pdfvault-ocr: text recognition on rendered pages with the PP-OCRv6 small
//! models (Apache-2.0, PaddlePaddle), 50 languages: Latin scripts with their
//! accents, Chinese and Japanese. The ONNX models and the character
//! dictionary are loaded from bytes the caller supplies; nothing is fetched
//! at run time.
//!
//! Detection is DB (a text probability map, thresholded into regions, each
//! grown back to the text it covers). Each region becomes a box turned to its
//! line's slant, so a page photographed askew reads like a straight one;
//! recognition reads each box as one line with greedy CTC decoding. Word boxes
//! come from where each character's frames fall along the line.

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
/// Slants below this are scanner noise: the box stays square to the page.
const MIN_TILT: f32 = 0.5 * std::f32::consts::PI / 180.0;
/// Slants beyond this are not a tilted line (a lone tall glyph, or vertical
/// text, which is not read): the box stays square to the page.
const MAX_TILT: f32 = 20.0 * std::f32::consts::PI / 180.0;

pub struct Ocr {
    det: Model,
    rec: Model,
    /// CTC classes: 0 is the blank, then the dictionary, then a space.
    classes: Vec<String>,
}

/// Axis-aligned bounds in input pixels, `x1`/`y1` exclusive.
#[derive(Debug, Clone, Copy)]
struct Rect {
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
}

/// A box in input pixels, possibly turned: its top-left corner, the unit
/// direction `u` its lines run in, and its size along `u` (`w`) and across
/// (`h`, downwards: `u` turned a quarter clockwise).
#[derive(Debug, Clone, Copy)]
struct Quad {
    ox: f32,
    oy: f32,
    ux: f32,
    uy: f32,
    w: f32,
    h: f32,
}

impl Quad {
    fn square(x0: f32, y0: f32, x1: f32, y1: f32) -> Self {
        Quad {
            ox: x0,
            oy: y0,
            ux: 1.0,
            uy: 0.0,
            w: x1 - x0,
            h: y1 - y0,
        }
    }

    /// The input pixel `a` along the box and `b` across it from its corner.
    fn at(&self, a: f32, b: f32) -> (f32, f32) {
        (
            self.ox + a * self.ux - b * self.uy,
            self.oy + a * self.uy + b * self.ux,
        )
    }

    /// Where a point lies along and across the direction `(ux, uy)`.
    fn project((x, y): (f32, f32), (ux, uy): (f32, f32)) -> (f32, f32) {
        (x * ux + y * uy, -x * uy + y * ux)
    }

    /// The axis-aligned bounds of the part `a0..a1` along, `b0..b1` across.
    fn bounds(&self, a0: f32, a1: f32, b0: f32, b1: f32) -> Rect {
        let corners = [
            self.at(a0, b0),
            self.at(a1, b0),
            self.at(a0, b1),
            self.at(a1, b1),
        ];
        let mut r = Rect {
            x0: f32::MAX,
            y0: f32::MAX,
            x1: f32::MIN,
            y1: f32::MIN,
        };
        for (x, y) in corners {
            (r.x0, r.y0, r.x1, r.y1) = (r.x0.min(x), r.y0.min(y), r.x1.max(x), r.y1.max(y));
        }
        r
    }
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
        for quad in self.detect(&img)? {
            if let Some(line) = self.read_line(&img, quad)? {
                lines.push((quad, line));
            }
        }
        // Ordered once read: boxes with no text (a page edge against the
        // table) would otherwise lump real lines into one row.
        Ok(reading_order(lines))
    }

    fn detect(&self, img: &Image) -> Result<Vec<Quad>, String> {
        let scale = (DET_MAX_SIDE / img.w.max(img.h) as f32).min(1.0);
        let round32 = |v: usize| ((v as f32 * scale / 32.0).round() as usize).max(1) * 32;
        let (dw, dh) = (round32(img.w), round32(img.h));
        let full = Quad::square(0.0, 0.0, img.w as f32, img.h as f32);
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
        Ok(components(&prob, mw, mh)
            .into_iter()
            .filter(|c| c.score >= BOX_THRESH)
            .filter_map(|c| c.quad(mw, sx, sy))
            .collect())
    }

    fn read_line(&self, img: &Image, q: Quad) -> Result<Option<Line>, String> {
        let width =
            ((REC_HEIGHT as f32 * q.w / q.h).round() as usize).clamp(REC_HEIGHT / 3, REC_MAX_WIDTH);
        let input = img.sample(q, width, REC_HEIGHT, |_, v| v / 127.5 - 1.0);
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
        // Positions are along (a) and across (b) the line until the end.
        let k = q.w / frames as f32;
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
        let ink = Ink::new(img, q);
        let mut spans: Vec<(String, [f32; 4])> = Vec::new();
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
                    let (a0, a1) = (left(s) * k, right(j - 1) * k);
                    let span = ink.tighten(a0, a1).unwrap_or([a0, a1, 0.0, q.h]);
                    spans.push((text, span));
                    start = None;
                }
                _ => {}
            }
        }
        if spans.is_empty() {
            return Ok(None);
        }
        // One baseline per line: every word gets the line's ink height, or a
        // word without descenders would sit on a line of its own.
        let b0 = spans.iter().map(|s| s.1[2]).fold(f32::MAX, f32::min);
        let b1 = spans.iter().map(|s| s.1[3]).fold(f32::MIN, f32::max);
        let words: Vec<Word> = spans
            .into_iter()
            .map(|(text, [a0, a1, ..])| {
                let r = q.bounds(a0, a1, b0, b1);
                Word {
                    text,
                    x: r.x0,
                    y: r.y0,
                    w: r.x1 - r.x0,
                    h: r.y1 - r.y0,
                }
            })
            .collect();
        let text = words
            .iter()
            .map(|w| w.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        Ok(Some(Line { text, words }))
    }
}

/// Top to bottom, then left to right within a row, both measured along the
/// page's slant (that of its longest line), so tilted rows stay rows.
fn reading_order<T>(mut items: Vec<(Quad, T)>) -> Vec<T> {
    let dir = items
        .iter()
        .max_by(|a, b| a.0.w.total_cmp(&b.0.w))
        .map_or((1.0, 0.0), |(q, _)| (q.ux, q.uy));
    // Each box as its span across the slant, and its start along it.
    let key = |q: &Quad| {
        let (_, mid) = Quad::project(q.at(q.w / 2.0, q.h / 2.0), dir);
        let (along, _) = Quad::project(q.at(0.0, q.h / 2.0), dir);
        (mid - q.h / 2.0, mid + q.h / 2.0, along)
    };
    items.sort_by(|a, b| key(&a.0).0.total_cmp(&key(&b.0).0));
    let mut ordered = Vec::with_capacity(items.len());
    let mut row: Vec<(Quad, T)> = Vec::new();
    let mut flush = |row: &mut Vec<(Quad, T)>| {
        row.sort_by(|a, b| key(&a.0).2.total_cmp(&key(&b.0).2));
        ordered.extend(row.drain(..).map(|(_, t)| t));
    };
    for item in items {
        let same_row = row.first().is_some_and(|(f, _)| {
            let ((f0, f1, _), (q0, q1, _)) = (key(f), key(&item.0));
            f1.min(q1) - f0.max(q0) > 0.5 * (f1 - f0).min(q1 - q0)
        });
        if !same_row {
            flush(&mut row);
        }
        row.push(item);
    }
    flush(&mut row);
    ordered
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

    /// Channel `c` (0 red, 1 green, 2 blue) at a point between pixel
    /// centres, interpolated; points outside take the nearest edge.
    fn bilinear(&self, x: f32, y: f32, c: usize) -> f32 {
        let (maxx, maxy) = ((self.w - 1) as f32, (self.h - 1) as f32);
        let (fx, fy) = ((x - 0.5).clamp(0.0, maxx), (y - 0.5).clamp(0.0, maxy));
        let (x0, y0, tx, ty) = (fx as usize, fy as usize, fx.fract(), fy.fract());
        let (x1, y1) = ((x0 + 1).min(self.w - 1), (y0 + 1).min(self.h - 1));
        let px = |x: usize, y: usize| self.rgb[(y * self.w + x) * 3 + c] as f32;
        let top = px(x0, y0) * (1.0 - tx) + px(x1, y0) * tx;
        let bottom = px(x0, y1) * (1.0 - tx) + px(x1, y1) * tx;
        top * (1.0 - ty) + bottom * ty
    }

    /// `q` resampled upright to `ow`×`oh` as a [1, 3, oh, ow] tensor in BGR
    /// order (the models were trained on BGR), each value mapped by `f`.
    fn sample(
        &self,
        q: Quad,
        ow: usize,
        oh: usize,
        f: impl Fn(usize, f32) -> f32,
    ) -> NdTensor<f32, 4> {
        let mut data = vec![0.0; 3 * ow * oh];
        let (sa, sb) = (q.w / ow as f32, q.h / oh as f32);
        for oy in 0..oh {
            for ox in 0..ow {
                let (x, y) = q.at((ox as f32 + 0.5) * sa, (oy as f32 + 0.5) * sb);
                for c in 0..3 {
                    data[(c * oh + oy) * ow + ox] = f(c, self.bilinear(x, y, 2 - c));
                }
            }
        }
        NdTensor::from_data([1, 3, oh, ow], data)
    }
}

/// Which pixels of a line box are ink, the box set upright at the input's
/// resolution: darker than halfway between its lightest and darkest, when
/// it has contrast at all.
struct Ink {
    w: usize,
    h: usize,
    mask: Vec<bool>,
}

impl Ink {
    fn new(img: &Image, q: Quad) -> Self {
        let (w, h) = ((q.w.ceil() as usize).max(1), (q.h.ceil() as usize).max(1));
        let gray: Vec<f32> = (0..w * h)
            .map(|i| {
                let (x, y) = q.at((i % w) as f32 + 0.5, (i / w) as f32 + 0.5);
                (img.bilinear(x, y, 0) * 3.0 + img.bilinear(x, y, 1) * 6.0 + img.bilinear(x, y, 2))
                    / 10.0
            })
            .collect();
        let (lo, hi) = gray
            .iter()
            .fold((f32::MAX, f32::MIN), |(lo, hi), &g| (g.min(lo), g.max(hi)));
        let mask = if hi - lo < 40.0 {
            Vec::new()
        } else {
            let mid = (lo + hi) / 2.0;
            gray.iter().map(|&g| g < mid).collect()
        };
        Ink { w, h, mask }
    }

    /// The ink's extent between `a0` and `a1` along the line, as
    /// `[a0, a1, b0, b1]` (along, then across), if there is any.
    fn tighten(&self, a0: f32, a1: f32) -> Option<[f32; 4]> {
        if self.mask.is_empty() {
            return None;
        }
        let a = (a0.max(0.0) as usize).min(self.w);
        let b = (a1.ceil() as usize).min(self.w);
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
        (bx0 < bx1).then_some([bx0 as f32, bx1 as f32, by0 as f32, by1 as f32])
    }
}

/// A connected region of the thresholded map: its pixels and mean probability.
struct Component {
    pixels: Vec<usize>,
    score: f32,
}

impl Component {
    /// The region as a box in input pixels (`sx`, `sy` per map pixel), turned
    /// to its slant and grown back to the text DB shrank: its principal axis
    /// gives the slant, its extents along and across give the size.
    fn quad(&self, mw: usize, sx: f32, sy: f32) -> Option<Quad> {
        let pts: Vec<(f32, f32)> = self
            .pixels
            .iter()
            .map(|&i| (((i % mw) as f32 + 0.5) * sx, ((i / mw) as f32 + 0.5) * sy))
            .collect();
        let n = pts.len() as f32;
        let (mx, my) = pts.iter().fold((0.0, 0.0), |(x, y), p| (x + p.0, y + p.1));
        let (mx, my) = (mx / n, my / n);
        let (cxx, cyy, cxy) = pts.iter().fold((0.0, 0.0, 0.0), |(a, b, c), (x, y)| {
            let (dx, dy) = (x - mx, y - my);
            (a + dx * dx, b + dy * dy, c + dx * dy)
        });
        let mut tilt = 0.5 * (2.0 * cxy).atan2(cxx - cyy);
        if !(MIN_TILT..=MAX_TILT).contains(&tilt.abs()) {
            tilt = 0.0;
        }
        let dir = (tilt.cos(), tilt.sin());
        let (mut a0, mut a1, mut b0, mut b1) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
        for &p in &pts {
            let (a, b) = Quad::project(p, dir);
            (a0, a1, b0, b1) = (a0.min(a), a1.max(a), b0.min(b), b1.max(b));
        }
        // Extents run between pixel centres; add the pixel they stand on.
        let (pa, pb) = (
            dir.0.abs() * sx + dir.1.abs() * sy,
            dir.1.abs() * sx + dir.0.abs() * sy,
        );
        let (w, h) = (a1 - a0 + pa, b1 - b0 + pb);
        let (wm, hm) = (w / pa, h / pb);
        if wm.min(hm) < MIN_BOX {
            return None;
        }
        // DB detects a shrunk core of each text; grow it back.
        let d = wm * hm * UNCLIP_RATIO / (2.0 * (wm + hm));
        let (da, db) = (d * pa, d * pb);
        let (a, b) = (a0 - pa / 2.0 - da, b0 - pb / 2.0 - db);
        let (ox, oy) = (a * dir.0 - b * dir.1, a * dir.1 + b * dir.0);
        Some(Quad {
            ox,
            oy,
            ux: dir.0,
            uy: dir.1,
            w: w + 2.0 * da,
            h: h + 2.0 * db,
        })
    }
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
        let (mut pixels, mut sum) = (Vec::new(), 0.0);
        while let Some(i) = stack.pop() {
            let (x, y) = (i % w, i / w);
            pixels.push(i);
            sum += prob[i];
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
        let score = sum / pixels.len() as f32;
        out.push(Component { pixels, score });
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

    const BODY: [&str; 6] = [
        "Invoice for the month of March",
        "The quick brown fox jumps over the lazy dog.",
        "Second line of body text, with numbers 1284.",
        "Payment is due within thirty days of receipt.",
        "Thank you for your continued trust in us.",
        "Kind regards from the accounting team.",
    ];

    /// The page as a phone would see it: turned by `degrees`, on a darker
    /// tabletop, lit unevenly from one side.
    fn photo_of(page: &image::RgbaImage, degrees: f32) -> (Vec<u8>, u32, u32) {
        let (pw, ph) = (page.width() as f32, page.height() as f32);
        let (w, h) = ((pw * 1.15) as u32, (ph * 1.15) as u32);
        let (s, c) = degrees.to_radians().sin_cos();
        let (cx, cy) = (w as f32 / 2.0, h as f32 / 2.0);
        let mut out = Vec::with_capacity((w * h * 3) as usize);
        for y in 0..h {
            for x in 0..w {
                // Inverse rotation about the centre, back into the page.
                let (dx, dy) = (x as f32 - cx, y as f32 - cy);
                let (u, v) = (c * dx + s * dy + pw / 2.0, -s * dx + c * dy + ph / 2.0);
                let light = 0.75 + 0.25 * x as f32 / w as f32;
                let rgb = if u >= 0.0 && v >= 0.0 && u < pw && v < ph {
                    let p = page.get_pixel(u as u32, v as u32).0;
                    [p[0], p[1], p[2]]
                } else {
                    [96, 78, 60]
                };
                out.extend(rgb.map(|v| (v as f32 * light) as u8));
            }
        }
        (out, w, h)
    }

    #[test]
    fn reads_a_tilted_photo_of_a_page() {
        let ocr = ocr();
        let mut b = pdfvault_core::fixtures::Builder::new();
        b.text_page(&BODY);
        let doc = pdfvault_render::open(&b.finish(), None).unwrap();
        let page = pdfvault_render::render_page(&doc, 0, 150.0).unwrap();
        for degrees in [-12.0, -6.0, 4.0, 10.0] {
            let (pixels, w, h) = photo_of(&page, degrees);
            let lines = ocr.recognize(&pixels, w, h).unwrap();
            let text: Vec<&str> = lines.iter().map(|l| l.text.as_str()).collect();
            for expected in BODY {
                assert!(
                    text.iter().any(|t| t.contains(expected)),
                    "{degrees}°: {expected:?} not in {text:#?}"
                );
            }
            // Lines stay in reading order, one per printed line.
            let order: Vec<usize> = BODY
                .iter()
                .map(|e| text.iter().position(|t| t.contains(e)).unwrap())
                .collect();
            assert!(order.is_sorted(), "{degrees}°: {text:#?}");
        }
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
