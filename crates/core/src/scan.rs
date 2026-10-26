//! Document scanning: find the sheet of paper in a photo, straighten it, and
//! clean it up (paper turned white, or crisp black and white).

use crate::error::{Error, Result};
use crate::images::{self, Format};
use image::{DynamicImage, GenericImageView, GrayImage, Luma, Rgb, RgbImage};
use serde::{Deserialize, Serialize};

/// Corners in pixels of the upright photo: top-left, top-right, bottom-right, bottom-left.
pub type Quad = [[f32; 2]; 4];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Detection {
    pub corners: Quad,
    /// False when no sheet stood out and `corners` is a default inset rectangle.
    pub found: bool,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Filter {
    /// Pixels as photographed.
    Original,
    /// Levels stretched per channel: the paper turns white, the ink dark.
    #[default]
    Color,
    Gray,
    /// Black and white with a local threshold, robust to shadows.
    Bw,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PageOptions {
    /// Absent: detected.
    pub corners: Option<Quad>,
    /// Clockwise: 90, 180 or 270, after straightening.
    pub rotate: Option<u32>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ScanOptions {
    /// One entry per input, in order; missing entries are detected.
    pub pages: Vec<PageOptions>,
    pub filter: Filter,
    /// JPEG quality, 1-100 (black and white pages are PNG).
    pub quality: Option<u8>,
    /// Longest side of a page, in pixels.
    pub max_side: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScannedPage {
    pub corners: Quad,
    pub detected: bool,
    pub width: u32,
    pub height: u32,
}

/// A4 at 300 DPI.
const MAX_SIDE: u32 = 3508;
/// Detection works on a copy this size: plenty for finding four corners.
const DETECT_SIDE: u32 = 512;

/// Find the sheet: the largest bright region, reduced to four corners.
pub fn detect(img: &DynamicImage) -> Detection {
    let (w, h) = img.dimensions();
    let s = (DETECT_SIDE as f32 / w.max(h) as f32).min(1.0);
    let (tw, th) = (
        ((w as f32 * s).round() as u32).max(1),
        ((h as f32 * s).round() as u32).max(1),
    );
    let small = image::imageops::blur(&paper_likeness(&img.thumbnail_exact(tw, th).to_rgb8()), 2.0);
    let fallback = || {
        let (mx, my) = (w as f32 * 0.04, h as f32 * 0.04);
        Detection {
            corners: [
                [mx, my],
                [w as f32 - mx, my],
                [w as f32 - mx, h as f32 - my],
                [mx, h as f32 - my],
            ],
            found: false,
            width: w,
            height: h,
        }
    };
    let Some(hull) = largest_bright_region(&small) else {
        return fallback();
    };
    let q = quad_of(&hull);
    if area(&q) < 0.2 * (tw * th) as f32 {
        return fallback();
    }
    // A pixel and a half inward: the blur spreads the sheet's edge onto the desk.
    let c = [
        q.iter().map(|p| p[0]).sum::<f32>() / 4.0,
        q.iter().map(|p| p[1]).sum::<f32>() / 4.0,
    ];
    let q = q.map(|p| {
        let d = dist(p, c).max(1.0);
        [
            p[0] + (c[0] - p[0]) * 1.5 / d,
            p[1] + (c[1] - p[1]) * 1.5 / d,
        ]
    });
    let (sx, sy) = (w as f32 / tw as f32, h as f32 / th as f32);
    Detection {
        corners: q.map(|[x, y]| [x * sx, y * sy]),
        found: true,
        width: w,
        height: h,
    }
}

/// Paper is bright and colorless: brightness minus saturation.
fn paper_likeness(rgb: &RgbImage) -> GrayImage {
    GrayImage::from_fn(rgb.width(), rgb.height(), |x, y| {
        let [r, g, b] = rgb.get_pixel(x, y).0.map(i32::from);
        let luma = (299 * r + 587 * g + 114 * b) / 1000;
        let chroma = r.max(g).max(b) - r.min(g).min(b);
        Luma([(luma - chroma).clamp(0, 255) as u8])
    })
}

/// Erode (`keep` = all) or dilate (`keep` = any) a mask with a 3×3 square.
fn morph(mask: &[bool], w: usize, h: usize, erode: bool) -> Vec<bool> {
    (0..w * h)
        .map(|i| {
            let (x, y) = (i % w, i / w);
            let mut near = (y.saturating_sub(1)..(y + 2).min(h))
                .flat_map(|ny| (x.saturating_sub(1)..(x + 2).min(w)).map(move |nx| ny * w + nx));
            if erode {
                near.all(|j| mask[j])
            } else {
                near.any(|j| mask[j])
            }
        })
        .collect()
}

/// The convex hull of the largest region brighter than the Otsu threshold.
fn largest_bright_region(g: &GrayImage) -> Option<Vec<[f32; 2]>> {
    let (w, h) = (g.width() as usize, g.height() as usize);
    let t = otsu(g)?;
    let mut mask: Vec<bool> = g.as_raw().iter().map(|&p| p > t).collect();
    // An opening cuts the thin bridges that join the sheet to bright clutter.
    for erode in [true, true, false, false] {
        mask = morph(&mask, w, h, erode);
    }
    let mut label = vec![0u32; w * h];
    let (mut best, mut best_size, mut next) = (0u32, 0usize, 1u32);
    let mut stack = vec![];
    for start in 0..w * h {
        if !mask[start] || label[start] != 0 {
            continue;
        }
        let mut size = 0;
        label[start] = next;
        stack.push(start);
        while let Some(i) = stack.pop() {
            size += 1;
            let (x, y) = (i % w, i / w);
            let mut visit = |j: usize| {
                if mask[j] && label[j] == 0 {
                    label[j] = next;
                    stack.push(j);
                }
            };
            if x > 0 {
                visit(i - 1);
            }
            if x + 1 < w {
                visit(i + 1);
            }
            if y > 0 {
                visit(i - w);
            }
            if y + 1 < h {
                visit(i + w);
            }
        }
        if size > best_size {
            (best, best_size) = (next, size);
        }
        next += 1;
    }
    if best == 0 {
        return None;
    }
    // Each row's leftmost and rightmost pixel is all the hull needs.
    let mut pts = vec![];
    for y in 0..h {
        let row = &label[y * w..(y + 1) * w];
        if let (Some(a), Some(b)) = (
            row.iter().position(|&l| l == best),
            row.iter().rposition(|&l| l == best),
        ) {
            pts.push([a as f32, y as f32]);
            pts.push([b as f32 + 1.0, y as f32]);
            pts.push([a as f32, y as f32 + 1.0]);
            pts.push([b as f32 + 1.0, y as f32 + 1.0]);
        }
    }
    let hull = convex_hull(pts);
    (hull.len() >= 4).then_some(hull)
}

/// The threshold splitting the histogram in two, if the two halves differ enough.
fn otsu(g: &GrayImage) -> Option<u8> {
    let mut hist = [0u64; 256];
    for &p in g.as_raw() {
        hist[p as usize] += 1;
    }
    let total = g.as_raw().len() as f64;
    let sum: f64 = hist
        .iter()
        .enumerate()
        .map(|(i, &c)| i as f64 * c as f64)
        .sum();
    let (mut w0, mut sum0, mut best, mut best_var, mut gap) = (0.0, 0.0, 0u8, -1.0, 0.0);
    for (t, &c) in hist.iter().enumerate() {
        w0 += c as f64;
        sum0 += t as f64 * c as f64;
        let w1 = total - w0;
        if w0 == 0.0 || w1 == 0.0 {
            continue;
        }
        let (m0, m1) = (sum0 / w0, (sum - sum0) / w1);
        let var = w0 * w1 * (m0 - m1) * (m0 - m1);
        if var > best_var {
            (best, best_var, gap) = (t as u8, var, m1 - m0);
        }
    }
    (gap >= 24.0).then_some(best)
}

/// Monotone chain, counter-clockwise in image coordinates (y down).
fn convex_hull(mut pts: Vec<[f32; 2]>) -> Vec<[f32; 2]> {
    pts.sort_by(|a, b| a[0].total_cmp(&b[0]).then(a[1].total_cmp(&b[1])));
    pts.dedup();
    if pts.len() < 3 {
        return pts;
    }
    let cross = |o: [f32; 2], a: [f32; 2], b: [f32; 2]| {
        (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0])
    };
    let mut hull: Vec<[f32; 2]> = vec![];
    for pass in [false, true] {
        let start = hull.len();
        let iter: Box<dyn Iterator<Item = &[f32; 2]>> = if pass {
            Box::new(pts.iter().rev())
        } else {
            Box::new(pts.iter())
        };
        for &p in iter {
            while hull.len() >= start + 2
                && cross(hull[hull.len() - 2], hull[hull.len() - 1], p) <= 0.0
            {
                hull.pop();
            }
            hull.push(p);
        }
        hull.pop();
    }
    hull
}

/// Four hull points spanning the largest area, ordered TL, TR, BR, BL.
fn quad_of(hull: &[[f32; 2]]) -> Quad {
    let pick = |f: &dyn Fn(&[f32; 2]) -> f32| {
        *hull
            .iter()
            .max_by(|a, b| f(a).total_cmp(&f(b)))
            .expect("hull is not empty")
    };
    let mut q = [
        pick(&|p| -(p[0] + p[1])),
        pick(&|p| p[0] - p[1]),
        pick(&|p| p[0] + p[1]),
        pick(&|p| p[1] - p[0]),
    ];
    // Move one corner at a time to the hull point that grows the area most.
    for _ in 0..3 {
        for i in 0..4 {
            let mut best = (area(&q), q[i]);
            for &p in hull {
                let mut t = q;
                t[i] = p;
                let a = area(&t);
                if a > best.0 && is_convex(&t) {
                    best = (a, p);
                }
            }
            q[i] = best.1;
        }
    }
    q
}

fn area(q: &Quad) -> f32 {
    let mut s = 0.0;
    for i in 0..4 {
        let (a, b) = (q[i], q[(i + 1) % 4]);
        s += a[0] * b[1] - b[0] * a[1];
    }
    s.abs() / 2.0
}

/// Convex with corners turning one way, in the TL, TR, BR, BL winding.
fn is_convex(q: &Quad) -> bool {
    (0..4).all(|i| {
        let (a, b, c) = (q[i], q[(i + 1) % 4], q[(i + 2) % 4]);
        (b[0] - a[0]) * (c[1] - b[1]) - (b[1] - a[1]) * (c[0] - b[0]) > 0.0
    })
}

fn dist(a: [f32; 2], b: [f32; 2]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
}

/// The homography taking the output rectangle's corners to `q`.
fn homography(q: &Quad, w: f64, h: f64) -> Option<[f64; 8]> {
    let dst = [[0.0, 0.0], [w, 0.0], [w, h], [0.0, h]];
    let mut m = [[0f64; 9]; 8];
    for i in 0..4 {
        let [u, v] = dst[i];
        let (x, y) = (q[i][0] as f64, q[i][1] as f64);
        m[2 * i] = [u, v, 1.0, 0.0, 0.0, 0.0, -u * x, -v * x, x];
        m[2 * i + 1] = [0.0, 0.0, 0.0, u, v, 1.0, -u * y, -v * y, y];
    }
    // Gaussian elimination with partial pivoting.
    for c in 0..8 {
        let p = (c..8).max_by(|&a, &b| m[a][c].abs().total_cmp(&m[b][c].abs()))?;
        if m[p][c].abs() < 1e-12 {
            return None;
        }
        m.swap(c, p);
        let pivot = m[c];
        for (r, row) in m.iter_mut().enumerate() {
            if r != c {
                let f = row[c] / pivot[c];
                for (x, p) in row[c..].iter_mut().zip(&pivot[c..]) {
                    *x -= f * p;
                }
            }
        }
    }
    Some(std::array::from_fn(|i| m[i][8] / m[i][i]))
}

/// Straighten the quadrilateral `q` into an upright rectangle.
pub fn warp(img: &RgbImage, q: &Quad, max_side: u32) -> Result<RgbImage> {
    let (iw, ih) = img.dimensions();
    let inside = |p: &[f32; 2]| {
        p.iter().all(|v| v.is_finite())
            && (-1.0..=iw as f32 + 1.0).contains(&p[0])
            && (-1.0..=ih as f32 + 1.0).contains(&p[1])
    };
    if !q.iter().all(inside) {
        return Err(Error::option(format!(
            "scan corners fall outside the {iw}x{ih} photo"
        )));
    }
    if !is_convex(q) || area(q) < 64.0 {
        return Err(Error::option(
            "scan corners must form a convex shape, in the order top-left, top-right, bottom-right, bottom-left",
        ));
    }
    let mut w = (dist(q[0], q[1]) + dist(q[3], q[2])) / 2.0;
    let mut h = (dist(q[0], q[3]) + dist(q[1], q[2])) / 2.0;
    let s = (max_side as f32 / w.max(h)).min(1.0);
    (w, h) = ((w * s).round().max(1.0), (h * s).round().max(1.0));
    let (ow, oh) = (w as u32, h as u32);
    let hm = homography(q, w as f64, h as f64)
        .ok_or_else(|| Error::option("scan corners are degenerate"))?;
    let raw = img.as_raw();
    let at = |x: i64, y: i64, c: usize| {
        let (x, y) = (
            x.clamp(0, iw as i64 - 1) as usize,
            y.clamp(0, ih as i64 - 1) as usize,
        );
        raw[(y * iw as usize + x) * 3 + c] as f64
    };
    Ok(RgbImage::from_fn(ow, oh, |ox, oy| {
        let (u, v) = (ox as f64 + 0.5, oy as f64 + 0.5);
        let d = hm[6] * u + hm[7] * v + 1.0;
        let x = (hm[0] * u + hm[1] * v + hm[2]) / d - 0.5;
        let y = (hm[3] * u + hm[4] * v + hm[5]) / d - 0.5;
        let (x0, y0) = (x.floor(), y.floor());
        let (fx, fy) = (x - x0, y - y0);
        let (x0, y0) = (x0 as i64, y0 as i64);
        Rgb(std::array::from_fn(|c| {
            let top = at(x0, y0, c) * (1.0 - fx) + at(x0 + 1, y0, c) * fx;
            let bottom = at(x0, y0 + 1, c) * (1.0 - fx) + at(x0 + 1, y0 + 1, c) * fx;
            (top * (1.0 - fy) + bottom * fy).round().clamp(0.0, 255.0) as u8
        }))
    }))
}

/// Map the `lo`..`hi` percentiles of `values` onto 0..255.
fn stretch_table(values: impl Iterator<Item = u8>, lo: f64, hi: f64) -> [u8; 256] {
    let mut hist = [0u64; 256];
    let mut n = 0u64;
    for v in values {
        hist[v as usize] += 1;
        n += 1;
    }
    let level = |frac: f64| {
        let target = (n as f64 * frac) as u64;
        let mut acc = 0;
        hist.iter()
            .position(|&c| {
                acc += c;
                acc > target
            })
            .unwrap_or(255) as f64
    };
    let (a, b) = (level(lo), level(hi));
    std::array::from_fn(|v| {
        if b - a < 16.0 {
            v as u8
        } else {
            ((v as f64 - a) * 255.0 / (b - a)).round().clamp(0.0, 255.0) as u8
        }
    })
}

fn luma(img: &RgbImage) -> GrayImage {
    DynamicImage::ImageRgb8(img.clone()).to_luma8()
}

/// Black where the pixel is darker than its neighborhood (Sauvola). The
/// neighborhood statistics come from a quarter-size copy to bound memory.
fn black_and_white(g: &GrayImage) -> GrayImage {
    let (w, h) = g.dimensions();
    let (sw, sh) = (w.div_ceil(4).max(1), h.div_ceil(4).max(1));
    let small = image::imageops::resize(g, sw, sh, image::imageops::FilterType::Triangle);
    let (sw, sh) = (sw as usize, sh as usize);
    let mut sum = vec![0f64; (sw + 1) * (sh + 1)];
    let mut sq = vec![0f64; (sw + 1) * (sh + 1)];
    for y in 0..sh {
        let (mut rs, mut rq) = (0.0, 0.0);
        for x in 0..sw {
            let v = small.as_raw()[y * sw + x] as f64;
            rs += v;
            rq += v * v;
            let i = (y + 1) * (sw + 1) + x + 1;
            sum[i] = sum[i - sw - 1] + rs;
            sq[i] = sq[i - sw - 1] + rq;
        }
    }
    let r = (sw.max(sh) / 16).max(2);
    let box_of = |t: &[f64], x0: usize, y0: usize, x1: usize, y1: usize| {
        t[y1 * (sw + 1) + x1] - t[y0 * (sw + 1) + x1] - t[y1 * (sw + 1) + x0]
            + t[y0 * (sw + 1) + x0]
    };
    let thresholds: Vec<f64> = (0..sw * sh)
        .map(|i| {
            let (x, y) = (i % sw, i / sw);
            let (x0, y0, x1, y1) = (
                x.saturating_sub(r),
                y.saturating_sub(r),
                (x + r + 1).min(sw),
                (y + r + 1).min(sh),
            );
            let n = ((x1 - x0) * (y1 - y0)) as f64;
            let m = box_of(&sum, x0, y0, x1, y1) / n;
            let sd = (box_of(&sq, x0, y0, x1, y1) / n - m * m).max(0.0).sqrt();
            m * (1.0 + 0.2 * (sd / 128.0 - 1.0))
        })
        .collect();
    GrayImage::from_fn(w, h, |x, y| {
        let t = thresholds[(y as usize / 4).min(sh - 1) * sw + (x as usize / 4).min(sw - 1)];
        Luma([if g.get_pixel(x, y)[0] as f64 > t {
            255
        } else {
            0
        }])
    })
}

/// Divide out uneven lighting and shadows: the paper's own brightness,
/// estimated by erasing the ink (a local maximum) and smoothing, turns white.
fn even_out(img: &RgbImage) -> RgbImage {
    use image::imageops::{FilterType, blur, resize};
    let (w, h) = img.dimensions();
    let s = (256.0 / w.max(h) as f32).min(1.0);
    let (sw, sh) = (
        ((w as f32 * s).round() as u32).max(1),
        ((h as f32 * s).round() as u32).max(1),
    );
    let small = resize(img, sw, sh, FilterType::Triangle);
    let paper = RgbImage::from_fn(sw, sh, |x, y| {
        let mut m = [0u8; 3];
        for ny in y.saturating_sub(2)..(y + 3).min(sh) {
            for nx in x.saturating_sub(2)..(x + 3).min(sw) {
                let p = small.get_pixel(nx, ny);
                m = std::array::from_fn(|c| m[c].max(p[c]));
            }
        }
        Rgb(m)
    });
    let paper = resize(&blur(&paper, 3.0), w, h, FilterType::Triangle);
    // Within 12% of the paper is paper: grain and sensor noise go white too.
    RgbImage::from_fn(w, h, |x, y| {
        let (p, b) = (img.get_pixel(x, y), paper.get_pixel(x, y));
        Rgb(std::array::from_fn(|c| {
            (p[c] as u32 * 255 * 100 / (b[c] as u32 * 88).max(1)).min(255) as u8
        }))
    })
}

pub fn enhance(img: RgbImage, filter: Filter) -> DynamicImage {
    match filter {
        Filter::Original => DynamicImage::ImageRgb8(img),
        Filter::Color => {
            let mut img = even_out(&img);
            let tables: [[u8; 256]; 3] =
                std::array::from_fn(|c| stretch_table(img.pixels().map(|p| p[c]), 0.01, 0.95));
            for p in img.pixels_mut() {
                *p = Rgb(std::array::from_fn(|c| tables[c][p[c] as usize]));
            }
            DynamicImage::ImageRgb8(img)
        }
        Filter::Gray => {
            let mut g = luma(&even_out(&img));
            let table = stretch_table(g.as_raw().iter().copied(), 0.01, 0.95);
            for p in g.pixels_mut() {
                p[0] = table[p[0] as usize];
            }
            DynamicImage::ImageLuma8(g)
        }
        Filter::Bw => DynamicImage::ImageLuma8(black_and_white(&luma(&img))),
    }
}

/// Straighten, rotate and clean up each photo. Returns the encoded page and its extension.
pub fn scan(
    inputs: &[Vec<u8>],
    o: &ScanOptions,
) -> Result<Vec<(Vec<u8>, &'static str, ScannedPage)>> {
    if inputs.is_empty() {
        return Err(Error::option("scan needs at least one photo"));
    }
    if o.pages.len() > inputs.len() {
        return Err(Error::option(format!(
            "{} page settings for {} photos",
            o.pages.len(),
            inputs.len()
        )));
    }
    let quality = o.quality.unwrap_or(85);
    if !(1..=100).contains(&quality) {
        return Err(Error::option("quality must be between 1 and 100"));
    }
    let max_side = o.max_side.unwrap_or(MAX_SIDE).clamp(16, 10_000);
    inputs
        .iter()
        .enumerate()
        .map(|(i, bytes)| {
            let img = images::decode(bytes)?;
            let page = o.pages.get(i).cloned().unwrap_or_default();
            let (corners, detected) = match page.corners {
                Some(c) => (c, false),
                None => (detect(&img).corners, true),
            };
            let mut flat = warp(&img.to_rgb8(), &corners, max_side)?;
            flat = match page.rotate {
                None | Some(0) => flat,
                Some(90) => image::imageops::rotate90(&flat),
                Some(180) => image::imageops::rotate180(&flat),
                Some(270) => image::imageops::rotate270(&flat),
                Some(r) => {
                    return Err(Error::option(format!(
                        "rotation {r} must be 90, 180 or 270"
                    )));
                }
            };
            let out = enhance(flat, o.filter);
            let (fmt, ext) = match o.filter {
                Filter::Bw => (Format::Png, "png"),
                _ => (Format::Jpeg, "jpg"),
            };
            let page = ScannedPage {
                corners,
                detected,
                width: out.width(),
                height: out.height(),
            };
            Ok((images::encode(&out, fmt, quality, None)?, ext, page))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A white sheet, slightly tilted, on a dark desk, with a line of "text".
    fn photo() -> (RgbImage, Quad) {
        let q: Quad = [[70.0, 40.0], [330.0, 60.0], [310.0, 380.0], [50.0, 350.0]];
        let img = RgbImage::from_fn(400, 420, |x, y| {
            let p = [x as f32 + 0.5, y as f32 + 0.5];
            let inside = (0..4).all(|i| {
                let (a, b) = (q[i], q[(i + 1) % 4]);
                (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0]) >= 0.0
            });
            let ink = inside && (150..156).contains(&y) && (110..270).contains(&x);
            match (inside, ink) {
                (_, true) => Rgb([30, 30, 40]),
                (true, _) => Rgb([225, 220, 210]),
                _ => Rgb([60 + (x % 7) as u8, 50, 45]),
            }
        });
        (img, q)
    }

    #[test]
    fn finds_the_sheet() {
        let (img, q) = photo();
        let d = detect(&DynamicImage::ImageRgb8(img));
        assert!(d.found);
        for (got, want) in d.corners.iter().zip(q) {
            assert!(dist(*got, want) < 0.02 * 420.0, "{got:?} vs {want:?}");
        }
    }

    #[test]
    fn falls_back_to_an_inset_rectangle() {
        let d = detect(&DynamicImage::ImageRgb8(RgbImage::from_pixel(
            100,
            50,
            Rgb([128, 128, 128]),
        )));
        assert!(!d.found);
        assert_eq!(d.corners[0], [4.0, 2.0]);
        assert_eq!(d.corners[2], [96.0, 48.0]);
    }

    #[test]
    fn warp_straightens_the_sheet() {
        let (img, q) = photo();
        let flat = warp(&img, &q, 10_000).unwrap();
        let (w, h) = flat.dimensions();
        assert!(
            (255..=265).contains(&w) && (300..=325).contains(&h),
            "{w}x{h}"
        );
        // No desk left in any corner: they are all paper.
        for (x, y) in [(3, 3), (w - 4, 3), (w - 4, h - 4), (3, h - 4)] {
            assert!(
                flat.get_pixel(x, y)[0] > 180,
                "corner {x},{y} is {:?}",
                flat.get_pixel(x, y)
            );
        }
        let small = warp(&img, &q, 100).unwrap();
        assert_eq!(small.width().max(small.height()), 100);
    }

    #[test]
    fn black_and_white_has_two_colors() {
        let (img, q) = photo();
        let flat = warp(&img, &q, 10_000).unwrap();
        let DynamicImage::ImageLuma8(g) = enhance(flat, Filter::Bw) else {
            panic!()
        };
        assert!(g.pixels().all(|p| p[0] == 0 || p[0] == 255));
        assert!(g.pixels().any(|p| p[0] == 0), "the text line survives");
        assert!(g.pixels().filter(|p| p[0] == 255).count() > g.pixels().len() * 9 / 10);
    }

    #[test]
    fn color_turns_the_paper_white() {
        let (img, q) = photo();
        let flat = warp(&img, &q, 10_000).unwrap();
        let out = enhance(flat, Filter::Color).to_rgb8();
        assert!(out.get_pixel(20, 20).0.iter().all(|&c| c >= 250));
    }

    #[test]
    fn scan_encodes_pages() {
        let (img, _) = photo();
        let jpeg = images::encode(&DynamicImage::ImageRgb8(img), Format::Jpeg, 90, None).unwrap();
        let out = scan(
            &[jpeg.clone(), jpeg.clone()],
            &ScanOptions {
                pages: vec![PageOptions {
                    corners: None,
                    rotate: Some(90),
                }],
                filter: Filter::Bw,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].1, "png");
        assert!(out[0].2.detected);
        assert!(out[0].2.width > out[0].2.height, "rotated");
        assert!(out[1].2.width < out[1].2.height);
        assert!(out[0].0.len() < jpeg.len(), "black and white is small");
    }

    #[test]
    fn rejects_bad_settings() {
        let (img, _) = photo();
        let jpeg = images::encode(&DynamicImage::ImageRgb8(img), Format::Jpeg, 90, None).unwrap();
        assert!(scan(&[], &ScanOptions::default()).is_err());
        let outside = ScanOptions {
            pages: vec![PageOptions {
                corners: Some([[0.0, 0.0], [900.0, 0.0], [900.0, 900.0], [0.0, 900.0]]),
                rotate: None,
            }],
            ..Default::default()
        };
        assert!(scan(std::slice::from_ref(&jpeg), &outside).is_err());
        let twisted = ScanOptions {
            pages: vec![PageOptions {
                corners: Some([[0.0, 0.0], [100.0, 100.0], [100.0, 0.0], [0.0, 100.0]]),
                rotate: None,
            }],
            ..Default::default()
        };
        assert!(scan(std::slice::from_ref(&jpeg), &twisted).is_err());
        let extra = ScanOptions {
            pages: vec![PageOptions::default(); 2],
            ..Default::default()
        };
        assert!(scan(&[jpeg], &extra).is_err());
    }
}
