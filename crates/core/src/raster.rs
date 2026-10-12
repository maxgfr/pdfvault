//! Pixel helpers shared by PDF image recompression and the image tools.

use crate::error::{Error, Result};
use image::{DynamicImage, GrayImage, ImageBuffer, RgbImage, imageops::FilterType};
use std::collections::HashMap;

/// Deflate at the strongest level miniz offers.
pub fn deflate(data: &[u8]) -> Vec<u8> {
    miniz_oxide::deflate::compress_to_vec_zlib(data, 10)
}

pub fn inflate(data: &[u8], limit: usize) -> Result<Vec<u8>> {
    miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(data, limit)
        .map_err(|e| Error::InvalidPdf(format!("bad Flate stream: {e:?}")))
}

/// Decode an ASCII85 (Base-85) stream, `~>` terminated, whitespace ignored.
pub fn ascii85_decode(data: &[u8]) -> Result<Vec<u8>> {
    let mut out = Vec::with_capacity(data.len() * 4 / 5);
    let mut group = [0u32; 5];
    let mut n = 0;
    let body = data.strip_prefix(b"<~").unwrap_or(data);
    for &c in body {
        match c {
            b'~' => break,
            b'z' if n == 0 => out.extend_from_slice(&[0, 0, 0, 0]),
            b'!'..=b'u' => {
                group[n] = (c - b'!') as u32;
                n += 1;
                if n == 5 {
                    let v = group
                        .iter()
                        .try_fold(0u32, |acc, d| acc.checked_mul(85)?.checked_add(*d));
                    let v = v.ok_or_else(|| Error::InvalidPdf("ASCII85 group overflows".into()))?;
                    out.extend_from_slice(&v.to_be_bytes());
                    n = 0;
                }
            }
            c if c.is_ascii_whitespace() => {}
            _ => return Err(Error::InvalidPdf("bad ASCII85 character".into())),
        }
    }
    if n == 1 {
        return Err(Error::InvalidPdf("truncated ASCII85 group".into()));
    }
    if n > 1 {
        for g in group.iter_mut().skip(n) {
            *g = 84;
        }
        let v = group.iter().fold(0u64, |acc, d| acc * 85 + *d as u64) as u32;
        out.extend_from_slice(&v.to_be_bytes()[..n - 1]);
    }
    Ok(out)
}

/// Decode an ASCIIHex stream, `>` terminated, whitespace ignored.
pub fn ascii_hex_decode(data: &[u8]) -> Result<Vec<u8>> {
    let mut digits = Vec::with_capacity(data.len());
    for &c in data {
        match c {
            b'>' => break,
            c if c.is_ascii_hexdigit() => digits.push((c as char).to_digit(16).unwrap_or(0) as u8),
            c if c.is_ascii_whitespace() => {}
            _ => return Err(Error::InvalidPdf("bad ASCIIHex character".into())),
        }
    }
    if digits.len() % 2 == 1 {
        digits.push(0);
    }
    Ok(digits.chunks(2).map(|p| (p[0] << 4) | p[1]).collect())
}

/// Undo one ASCII text-encoding filter by name, or None if it is not one.
pub fn decode_ascii_filter(name: &[u8], data: &[u8]) -> Option<Result<Vec<u8>>> {
    match name {
        b"ASCII85Decode" | b"A85" => Some(ascii85_decode(data)),
        b"ASCIIHexDecode" | b"AHx" => Some(ascii_hex_decode(data)),
        _ => None,
    }
}

/// Apply PNG row filters (PDF /Predictor 15), choosing per row the filter with
/// the smallest sum of absolute values, the heuristic libpng uses.
pub fn png_predict(data: &[u8], row_bytes: usize, bpp: usize) -> Vec<u8> {
    let rows = data.len() / row_bytes.max(1);
    let mut out = Vec::with_capacity(data.len() + rows);
    let zero = vec![0u8; row_bytes];
    let mut candidate = vec![0u8; row_bytes];
    let mut best = vec![0u8; row_bytes];
    for r in 0..rows {
        let row = &data[r * row_bytes..(r + 1) * row_bytes];
        let prev = if r == 0 {
            &zero[..]
        } else {
            &data[(r - 1) * row_bytes..r * row_bytes]
        };
        let mut best_score = u64::MAX;
        let mut best_filter = 0u8;
        for filter in 0..5u8 {
            for i in 0..row_bytes {
                let a = if i >= bpp { row[i - bpp] } else { 0 };
                let b = prev[i];
                let c = if i >= bpp { prev[i - bpp] } else { 0 };
                candidate[i] = match filter {
                    0 => row[i],
                    1 => row[i].wrapping_sub(a),
                    2 => row[i].wrapping_sub(b),
                    3 => row[i].wrapping_sub(((a as u16 + b as u16) / 2) as u8),
                    _ => row[i].wrapping_sub(paeth(a, b, c)),
                };
            }
            let score: u64 = candidate
                .iter()
                .map(|&v| (v as i8).unsigned_abs() as u64)
                .sum();
            if score < best_score {
                best_score = score;
                best_filter = filter;
                best.copy_from_slice(&candidate);
            }
        }
        out.push(best_filter);
        out.extend_from_slice(&best);
    }
    out
}

fn paeth(a: u8, b: u8, c: u8) -> u8 {
    let p = a as i16 + b as i16 - c as i16;
    let (pa, pb, pc) = (
        (p - a as i16).abs(),
        (p - b as i16).abs(),
        (p - c as i16).abs(),
    );
    if pa <= pb && pa <= pc {
        a
    } else if pb <= pc {
        b
    } else {
        c
    }
}

/// Pixels decoded from a PDF image or an image file: 8-bit gray or RGB.
#[derive(Clone)]
pub enum Pixels {
    Gray(GrayImage),
    Rgb(RgbImage),
}

impl Pixels {
    pub fn width(&self) -> u32 {
        match self {
            Pixels::Gray(i) => i.width(),
            Pixels::Rgb(i) => i.width(),
        }
    }
    pub fn height(&self) -> u32 {
        match self {
            Pixels::Gray(i) => i.height(),
            Pixels::Rgb(i) => i.height(),
        }
    }
    pub fn to_gray(&self) -> Pixels {
        match self {
            Pixels::Gray(_) => self.clone(),
            Pixels::Rgb(i) => Pixels::Gray(DynamicImage::ImageRgb8(i.clone()).to_luma8()),
        }
    }
    pub fn resize(&self, w: u32, h: u32) -> Pixels {
        let f = FilterType::CatmullRom;
        match self {
            Pixels::Gray(i) => Pixels::Gray(image::imageops::resize(i, w, h, f)),
            Pixels::Rgb(i) => Pixels::Rgb(image::imageops::resize(i, w, h, f)),
        }
    }
    pub fn components(&self) -> usize {
        match self {
            Pixels::Gray(_) => 1,
            Pixels::Rgb(_) => 3,
        }
    }
    pub fn raw(&self) -> &[u8] {
        match self {
            Pixels::Gray(i) => i.as_raw(),
            Pixels::Rgb(i) => i.as_raw(),
        }
    }
    /// An RGB image whose channels are all equal is really gray.
    pub fn simplify(self) -> Pixels {
        if let Pixels::Rgb(i) = &self
            && i.pixels().all(|p| p[0] == p[1] && p[1] == p[2])
        {
            return Pixels::Gray(ImageBuffer::from_fn(i.width(), i.height(), |x, y| {
                image::Luma([i.get_pixel(x, y)[0]])
            }));
        }
        self
    }
}

pub fn encode_jpeg(px: &Pixels, quality: u8) -> Result<Vec<u8>> {
    let (w, h) = (px.width(), px.height());
    if w > u16::MAX as u32 || h > u16::MAX as u32 {
        return Err(Error::Image("image too large for JPEG".into()));
    }
    let mut out = Vec::new();
    let mut enc = jpeg_encoder::Encoder::new(&mut out, quality.clamp(1, 100));
    // Standard Huffman tables: the optimized ones jpeg-encoder writes are
    // misread by some decoders (hayro among them).
    let color = match px {
        Pixels::Gray(_) => jpeg_encoder::ColorType::Luma,
        Pixels::Rgb(_) => {
            enc.set_sampling_factor(jpeg_encoder::SamplingFactor::F_2_2);
            jpeg_encoder::ColorType::Rgb
        }
    };
    enc.encode(px.raw(), w as u16, h as u16, color)
        .map_err(|e| Error::Image(e.to_string()))?;
    Ok(out)
}

/// A lossless palette when the image has at most 256 colors.
pub struct Palette {
    /// RGB triples (or single gray bytes for a gray source).
    pub colors: Vec<u8>,
    pub bits: u8,
    /// Packed indices, rows padded to whole bytes.
    pub data: Vec<u8>,
}

pub fn palette(px: &Pixels) -> Option<Palette> {
    let comps = px.components();
    let raw = px.raw();
    let mut map: HashMap<[u8; 3], u8> = HashMap::new();
    let mut colors = Vec::new();
    let mut indices = Vec::with_capacity(raw.len() / comps);
    for p in raw.chunks_exact(comps) {
        let key = if comps == 1 {
            [p[0], 0, 0]
        } else {
            [p[0], p[1], p[2]]
        };
        let idx = match map.get(&key) {
            Some(i) => *i,
            None => {
                if map.len() == 256 {
                    return None;
                }
                let i = map.len() as u8;
                map.insert(key, i);
                colors.extend_from_slice(&p[..comps]);
                i
            }
        };
        indices.push(idx);
    }
    let bits: u8 = match map.len() {
        0..=2 => 1,
        3..=4 => 2,
        5..=16 => 4,
        _ => 8,
    };
    let w = px.width() as usize;
    let row_bytes = (w * bits as usize).div_ceil(8);
    let mut data = vec![0u8; row_bytes * px.height() as usize];
    for (y, row) in indices.chunks_exact(w).enumerate() {
        for (x, &i) in row.iter().enumerate() {
            let bit = x * bits as usize;
            data[y * row_bytes + bit / 8] |= i << (8 - bits as usize - bit % 8);
        }
    }
    Some(Palette { colors, bits, data })
}

/// Map every pixel to the nearest color of `colors` (packed like [`Palette::colors`]).
pub fn snap_to_palette(px: &Pixels, colors: &[u8]) -> Pixels {
    let comps = px.components();
    let table: Vec<&[u8]> = colors.chunks_exact(comps).collect();
    let mut cache: HashMap<[u8; 3], usize> = HashMap::new();
    let mut out = px.raw().to_vec();
    for p in out.chunks_exact_mut(comps) {
        let key = if comps == 1 {
            [p[0], 0, 0]
        } else {
            [p[0], p[1], p[2]]
        };
        let best = *cache.entry(key).or_insert_with(|| {
            (0..table.len())
                .min_by_key(|&i| {
                    table[i]
                        .iter()
                        .zip(p.iter())
                        .map(|(a, b)| (*a as i32 - *b as i32).pow(2))
                        .sum::<i32>()
                })
                .unwrap_or(0)
        });
        p.copy_from_slice(table[best]);
    }
    match px {
        Pixels::Gray(i) => {
            Pixels::Gray(GrayImage::from_raw(i.width(), i.height(), out).expect("same size"))
        }
        Pixels::Rgb(i) => {
            Pixels::Rgb(RgbImage::from_raw(i.width(), i.height(), out).expect("same size"))
        }
    }
}

pub fn row_bytes(width: u32, components: usize, bits: u8) -> usize {
    (width as usize * components * bits as usize).div_ceil(8)
}

/// Unpack 1/2/4/8-bit samples to one byte per sample.
pub fn unpack(data: &[u8], width: usize, height: usize, comps: usize, bits: u8) -> Option<Vec<u8>> {
    if bits == 8 {
        let need = width * height * comps;
        return (data.len() >= need).then(|| data[..need].to_vec());
    }
    if !matches!(bits, 1 | 2 | 4) {
        return None;
    }
    let row = (width * comps * bits as usize).div_ceil(8);
    if data.len() < row * height {
        return None;
    }
    let mask = (1u16 << bits) - 1;
    let mut out = Vec::with_capacity(width * height * comps);
    for y in 0..height {
        for s in 0..width * comps {
            let bit = s * bits as usize;
            let byte = data[y * row + bit / 8] as u16;
            out.push(((byte >> (8 - bits as usize - bit % 8)) & mask) as u8);
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn predictor_roundtrip_filter_bytes() {
        let data: Vec<u8> = (0..60u8).collect();
        let p = png_predict(&data, 12, 3);
        assert_eq!(p.len(), 65);
        assert!(p.chunks(13).all(|r| r[0] <= 4));
    }

    #[test]
    fn palette_packs_bits() {
        let img = GrayImage::from_fn(10, 2, |x, _| {
            image::Luma([if x % 2 == 0 { 0 } else { 255 }])
        });
        let p = palette(&Pixels::Gray(img)).unwrap();
        assert_eq!(p.bits, 1);
        assert_eq!(p.data.len(), 4);
        assert_eq!(p.data[0], 0b0101_0101);
    }

    #[test]
    fn ascii_filters() {
        assert_eq!(
            ascii85_decode(b"87cURD]i,\"Ebo80~>").unwrap(),
            b"Hello World!"
        );
        assert_eq!(ascii85_decode(b"z~>").unwrap(), vec![0, 0, 0, 0]);
        assert_eq!(ascii85_decode(b"<~@:E^~>").unwrap(), b"abc");
        assert_eq!(ascii_hex_decode(b"48 65 6c6C 6>").unwrap(), b"Hel\x6c\x60");
        assert!(ascii85_decode(b"{").is_err());
    }

    #[test]
    fn unpack_bits() {
        assert_eq!(unpack(&[0b1010_0000], 3, 1, 1, 1).unwrap(), vec![1, 0, 1]);
        assert_eq!(unpack(&[0x12, 0x30], 3, 1, 1, 4).unwrap(), vec![1, 2, 3]);
    }
}
