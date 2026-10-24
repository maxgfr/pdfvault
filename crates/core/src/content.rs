//! A content-stream lexer that remembers where every operation sits, so a
//! stream can be edited by splicing bytes rather than re-encoded: whatever
//! is not touched (inline images, odd spacing, comments) stays byte for byte.

use crate::error::{Error, Result};
use lopdf::{Dictionary, Object, StringFormat};
use std::ops::Range;

/// Deepest array or dictionary nesting accepted inside a content stream.
const MAX_DEPTH: usize = 32;
/// Most operands one operation may take (TJ arrays count as one).
const MAX_OPERANDS: usize = 64;

#[derive(Debug, Clone)]
pub struct Op {
    pub name: Vec<u8>,
    pub args: Vec<Object>,
    /// From the first operand to the end of the operator (to `EI` for inline images).
    pub span: Range<usize>,
}

impl Op {
    pub fn is(&self, name: &str) -> bool {
        self.name == name.as_bytes()
    }

    pub fn num(&self, i: usize) -> Option<f32> {
        crate::pdf::num(self.args.get(i)?)
    }

    pub fn nums<const N: usize>(&self) -> Option<[f32; N]> {
        if self.args.len() < N {
            return None;
        }
        let mut out = [0.0; N];
        for (i, v) in out.iter_mut().enumerate() {
            *v = self.num(self.args.len() - N + i)?;
        }
        Some(out)
    }

    pub fn name_arg(&self, i: usize) -> Option<&[u8]> {
        self.args.get(i)?.as_name().ok()
    }
}

fn is_white(b: u8) -> bool {
    matches!(b, 0 | 9 | 10 | 12 | 13 | 32)
}

fn is_delim(b: u8) -> bool {
    matches!(
        b,
        b'(' | b')' | b'<' | b'>' | b'[' | b']' | b'{' | b'}' | b'/' | b'%'
    )
}

struct Lexer<'a> {
    s: &'a [u8],
    i: usize,
}

enum Token {
    Obj(Object),
    Keyword(Vec<u8>),
    ArrayEnd,
    DictEnd,
}

fn bad(msg: &str) -> Error {
    Error::Unsupported(format!("content stream: {msg}"))
}

impl Lexer<'_> {
    fn skip_space(&mut self) {
        while self.i < self.s.len() {
            let b = self.s[self.i];
            if is_white(b) {
                self.i += 1;
            } else if b == b'%' {
                while self.i < self.s.len() && !matches!(self.s[self.i], b'\r' | b'\n') {
                    self.i += 1;
                }
            } else {
                break;
            }
        }
    }

    fn token(&mut self, depth: usize) -> Result<Option<Token>> {
        self.skip_space();
        let Some(&b) = self.s.get(self.i) else {
            return Ok(None);
        };
        if depth > MAX_DEPTH {
            return Err(bad("nested too deeply"));
        }
        Ok(Some(match b {
            b'/' => {
                self.i += 1;
                Token::Obj(Object::Name(self.name()))
            }
            b'(' => Token::Obj(Object::String(self.literal()?, StringFormat::Literal)),
            b'<' if self.s.get(self.i + 1) == Some(&b'<') => {
                self.i += 2;
                Token::Obj(Object::Dictionary(self.dict(depth + 1)?))
            }
            b'<' => Token::Obj(Object::String(self.hex()?, StringFormat::Hexadecimal)),
            b'>' if self.s.get(self.i + 1) == Some(&b'>') => {
                self.i += 2;
                Token::DictEnd
            }
            b'[' => {
                self.i += 1;
                let mut items = vec![];
                loop {
                    match self.token(depth + 1)? {
                        Some(Token::Obj(o)) => items.push(o),
                        Some(Token::ArrayEnd) => break,
                        // Some producers put stray keywords in arrays; keep going.
                        Some(Token::Keyword(_)) => {}
                        Some(Token::DictEnd) | None => return Err(bad("unterminated array")),
                    }
                }
                Token::Obj(Object::Array(items))
            }
            b']' => {
                self.i += 1;
                Token::ArrayEnd
            }
            b'+' | b'-' | b'.' | b'0'..=b'9' => {
                let start = self.i;
                self.i += 1;
                while self
                    .s
                    .get(self.i)
                    .is_some_and(|c| c.is_ascii_digit() || matches!(c, b'.' | b'-' | b'+'))
                {
                    self.i += 1;
                }
                let text = std::str::from_utf8(&self.s[start..self.i]).unwrap_or("0");
                Token::Obj(number(text))
            }
            b')' | b'>' | b'{' | b'}' => {
                // Stray delimiters (PostScript braces in Type 4 functions do not
                // belong here): treat each as a keyword so the op is ignored.
                self.i += 1;
                Token::Keyword(vec![b])
            }
            _ => {
                let start = self.i;
                while self
                    .s
                    .get(self.i)
                    .is_some_and(|&c| !is_white(c) && !is_delim(c))
                {
                    self.i += 1;
                }
                let word = self.s[start..self.i].to_vec();
                match word.as_slice() {
                    b"true" => Token::Obj(Object::Boolean(true)),
                    b"false" => Token::Obj(Object::Boolean(false)),
                    b"null" => Token::Obj(Object::Null),
                    _ => Token::Keyword(word),
                }
            }
        }))
    }

    fn name(&mut self) -> Vec<u8> {
        let mut out = vec![];
        while let Some(&c) = self.s.get(self.i) {
            if is_white(c) || is_delim(c) {
                break;
            }
            if c == b'#'
                && let Some(h) = self.s.get(self.i + 1..self.i + 3)
                && let Ok(v) = u8::from_str_radix(std::str::from_utf8(h).unwrap_or("zz"), 16)
            {
                out.push(v);
                self.i += 3;
                continue;
            }
            out.push(c);
            self.i += 1;
        }
        out
    }

    fn literal(&mut self) -> Result<Vec<u8>> {
        self.i += 1;
        let mut out = vec![];
        let mut depth = 1;
        while let Some(&c) = self.s.get(self.i) {
            self.i += 1;
            match c {
                b'\\' => {
                    let Some(&e) = self.s.get(self.i) else { break };
                    self.i += 1;
                    match e {
                        b'n' => out.push(b'\n'),
                        b'r' => out.push(b'\r'),
                        b't' => out.push(b'\t'),
                        b'b' => out.push(8),
                        b'f' => out.push(12),
                        b'\r' => {
                            if self.s.get(self.i) == Some(&b'\n') {
                                self.i += 1;
                            }
                        }
                        b'\n' => {}
                        b'0'..=b'7' => {
                            let mut v = (e - b'0') as u32;
                            for _ in 0..2 {
                                match self.s.get(self.i) {
                                    Some(&d @ b'0'..=b'7') => {
                                        v = v * 8 + (d - b'0') as u32;
                                        self.i += 1;
                                    }
                                    _ => break,
                                }
                            }
                            out.push(v as u8);
                        }
                        other => out.push(other),
                    }
                }
                b'(' => {
                    depth += 1;
                    out.push(c);
                }
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        return Ok(out);
                    }
                    out.push(c);
                }
                _ => out.push(c),
            }
        }
        Err(bad("unterminated string"))
    }

    fn hex(&mut self) -> Result<Vec<u8>> {
        self.i += 1;
        let mut digits = vec![];
        while let Some(&c) = self.s.get(self.i) {
            self.i += 1;
            if c == b'>' {
                if digits.len() % 2 == 1 {
                    digits.push(b'0');
                }
                return Ok(digits
                    .chunks(2)
                    .map(|p| (hexval(p[0]) << 4) | hexval(p[1]))
                    .collect());
            }
            if c.is_ascii_hexdigit() {
                digits.push(c);
            } else if !is_white(c) {
                return Err(bad("bad hex string"));
            }
        }
        Err(bad("unterminated hex string"))
    }

    fn dict(&mut self, depth: usize) -> Result<Dictionary> {
        let mut d = Dictionary::new();
        loop {
            match self.token(depth)? {
                Some(Token::DictEnd) => return Ok(d),
                Some(Token::Obj(Object::Name(k))) => match self.token(depth)? {
                    Some(Token::Obj(v)) => d.set(k, v),
                    _ => return Err(bad("dictionary value missing")),
                },
                _ => return Err(bad("bad dictionary")),
            }
        }
    }

    /// Inline image data after `ID`: up to the `EI` that ends it.
    fn image_data(&mut self, dict: &Dictionary) -> Result<Range<usize>> {
        // One white-space byte separates ID from the data.
        if self.s.get(self.i).is_some_and(|&c| is_white(c)) {
            self.i += 1;
        }
        let start = self.i;
        if let Some(len) = inline_length(dict)
            && let Some(end) = start.checked_add(len)
            && end <= self.s.len()
        {
            let mut j = end;
            while self.s.get(j).is_some_and(|&c| is_white(c)) {
                j += 1;
            }
            if self.s.get(j..j + 2) == Some(b"EI") && ends_word(self.s, j + 2) {
                self.i = j + 2;
                return Ok(start..end);
            }
        }
        // Filtered data: the first `EI` between white space that is followed by
        // what looks like more content (or the end) closes the image.
        let mut j = start;
        while j + 2 <= self.s.len() {
            if self.s[j] == b'E'
                && self.s[j + 1] == b'I'
                && j > start
                && is_white(self.s[j - 1])
                && ends_word(self.s, j + 2)
                && looks_like_content(&self.s[j + 2..])
            {
                self.i = j + 2;
                return Ok(start..j - 1);
            }
            j += 1;
        }
        Err(bad("inline image without EI"))
    }
}

fn hexval(c: u8) -> u8 {
    match c {
        b'0'..=b'9' => c - b'0',
        b'a'..=b'f' => c - b'a' + 10,
        b'A'..=b'F' => c - b'A' + 10,
        _ => 0,
    }
}

fn number(text: &str) -> Object {
    if !text.contains('.')
        && let Ok(v) = text.parse::<i64>()
    {
        return Object::Integer(v);
    }
    // PDF allows "-.5", "5." and the occasional "--5" or "1-2" from sloppy writers.
    let cleaned: String = {
        let neg = text.starts_with('-');
        let rest: String = text
            .trim_start_matches(['+', '-'])
            .chars()
            .take_while(|c| c.is_ascii_digit() || *c == '.')
            .collect();
        if neg { format!("-{rest}") } else { rest }
    };
    Object::Real(cleaned.parse::<f32>().unwrap_or(0.0))
}

fn ends_word(s: &[u8], at: usize) -> bool {
    s.get(at).is_none_or(|&c| is_white(c) || is_delim(c))
}

/// After a candidate `EI`, real content is printable ASCII.
fn looks_like_content(rest: &[u8]) -> bool {
    rest.iter()
        .take(64)
        .all(|&c| is_white(c) || (0x20..0x7f).contains(&c))
}

/// Byte length of unfiltered inline image data, when the dictionary says it.
fn inline_length(d: &Dictionary) -> Option<usize> {
    let get = |short: &[u8], long: &[u8]| d.get(short).or_else(|_| d.get(long)).ok();
    if get(b"F", b"Filter").is_some_and(|f| !matches!(f, Object::Array(a) if a.is_empty())) {
        return None;
    }
    let w = get(b"W", b"Width")?.as_i64().ok()? as usize;
    let h = get(b"H", b"Height")?.as_i64().ok()? as usize;
    let mask = get(b"IM", b"ImageMask").and_then(|v| v.as_bool().ok()) == Some(true);
    let bpc = if mask {
        1
    } else {
        get(b"BPC", b"BitsPerComponent")?.as_i64().ok()? as usize
    };
    let comps = if mask {
        1
    } else {
        match get(b"CS", b"ColorSpace")? {
            Object::Name(n) => match n.as_slice() {
                b"G" | b"DeviceGray" | b"I" | b"Indexed" => 1,
                b"RGB" | b"DeviceRGB" => 3,
                b"CMYK" | b"DeviceCMYK" => 4,
                _ => return None,
            },
            Object::Array(a) if a.first().and_then(|o| o.as_name().ok()) == Some(b"I") => 1,
            Object::Array(a) if a.first().and_then(|o| o.as_name().ok()) == Some(b"Indexed") => 1,
            _ => return None,
        }
    };
    w.checked_mul(comps)?
        .checked_mul(bpc)?
        .div_ceil(8)
        .checked_mul(h)
}

/// Split a content stream into operations with their byte spans.
pub fn parse(s: &[u8]) -> Result<Vec<Op>> {
    let mut lx = Lexer { s, i: 0 };
    let mut ops = vec![];
    let mut args: Vec<Object> = vec![];
    let mut start = None;
    loop {
        lx.skip_space();
        let at = lx.i;
        let Some(tok) = lx.token(0)? else { break };
        match tok {
            Token::Obj(o) => {
                if args.len() >= MAX_OPERANDS {
                    return Err(bad("too many operands"));
                }
                start.get_or_insert(at);
                args.push(o);
            }
            Token::Keyword(word) if word == b"BI" => {
                let begin = start.take().unwrap_or(at);
                args.clear();
                let mut dict = Dictionary::new();
                loop {
                    match lx.token(1)? {
                        Some(Token::Keyword(k)) if k == b"ID" => break,
                        Some(Token::Obj(Object::Name(k))) => match lx.token(1)? {
                            Some(Token::Obj(v)) => dict.set(k, v),
                            _ => return Err(bad("bad inline image dictionary")),
                        },
                        _ => return Err(bad("bad inline image dictionary")),
                    }
                }
                let data = lx.image_data(&dict)?;
                ops.push(Op {
                    name: b"BI".to_vec(),
                    args: vec![
                        Object::Dictionary(dict),
                        Object::Integer(data.start as i64),
                        Object::Integer(data.end as i64),
                    ],
                    span: begin..lx.i,
                });
            }
            Token::Keyword(word) => {
                let begin = start.take().unwrap_or(at);
                ops.push(Op {
                    name: word,
                    args: std::mem::take(&mut args),
                    span: begin..lx.i,
                });
            }
            Token::ArrayEnd | Token::DictEnd => return Err(bad("unbalanced delimiter")),
        }
    }
    Ok(ops)
}

/// The raw data of an inline image op (`BI`), as [`parse`] recorded it.
pub fn inline_data<'a>(s: &'a [u8], op: &Op) -> Option<&'a [u8]> {
    let a = op.args.get(1)?.as_i64().ok()? as usize;
    let b = op.args.get(2)?.as_i64().ok()? as usize;
    s.get(a..b)
}

/// Replace byte ranges of `s`. Ranges must not overlap; order does not matter.
pub fn splice(s: &[u8], mut edits: Vec<(Range<usize>, Vec<u8>)>) -> Vec<u8> {
    edits.sort_by_key(|(r, _)| r.start);
    let mut out = Vec::with_capacity(s.len());
    let mut at = 0;
    for (r, bytes) in edits {
        if r.start < at {
            continue;
        }
        out.extend_from_slice(&s[at..r.start]);
        out.extend(bytes);
        at = r.end;
    }
    out.extend_from_slice(&s[at..]);
    out
}

/// Write `o` as content-stream syntax (strings always as hex).
pub fn write_obj(out: &mut Vec<u8>, o: &Object) {
    match o {
        Object::Null => out.extend(b"null"),
        Object::Boolean(b) => out.extend(if *b { &b"true"[..] } else { b"false" }),
        Object::Integer(i) => out.extend(i.to_string().as_bytes()),
        Object::Real(r) => out.extend(crate::pdf::n(*r).as_bytes()),
        Object::Name(n) => {
            out.push(b'/');
            for &c in n {
                if c <= b' ' || c >= 0x7f || is_delim(c) || c == b'#' {
                    out.extend(format!("#{c:02X}").as_bytes());
                } else {
                    out.push(c);
                }
            }
        }
        Object::String(s, _) => {
            out.push(b'<');
            for c in s {
                out.extend(format!("{c:02X}").as_bytes());
            }
            out.push(b'>');
        }
        Object::Array(a) => {
            out.push(b'[');
            for (i, v) in a.iter().enumerate() {
                if i > 0 {
                    out.push(b' ');
                }
                write_obj(out, v);
            }
            out.push(b']');
        }
        Object::Dictionary(d) => {
            out.extend(b"<<");
            for (k, v) in d.iter() {
                write_obj(out, &Object::Name(k.clone()));
                out.push(b' ');
                write_obj(out, v);
                out.push(b' ');
            }
            out.extend(b">>");
        }
        Object::Reference(_) | Object::Stream(_) => out.extend(b"null"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spans_strings_and_inline_images() {
        let s = b"q 1 0 0 1 5 5 cm BT /F1 12 Tf (a\\)b) Tj [<0041> -20 (c)] TJ ET\n\
                  BI /W 2 /H 1 /CS /G /BPC 8 ID \x00\xffEI Q % done\n/P <</MCID 3>> BDC EMC";
        let ops = parse(s).unwrap();
        let names: Vec<_> = ops
            .iter()
            .map(|o| String::from_utf8_lossy(&o.name).into_owned())
            .collect();
        assert_eq!(
            names,
            [
                "q", "cm", "BT", "Tf", "Tj", "TJ", "ET", "BI", "Q", "BDC", "EMC"
            ]
        );
        assert_eq!(&s[ops[4].span.clone()], b"(a\\)b) Tj");
        assert_eq!(ops[4].args[0].as_str().unwrap(), b"a)b");
        assert_eq!(inline_data(s, &ops[7]).unwrap(), b"\x00\xff");
        assert_eq!(&s[ops[8].span.clone()], b"Q");
        let tj = ops[5].args[0].as_array().unwrap();
        assert_eq!(tj[0].as_str().unwrap(), b"\x00A".as_slice());
        let out = splice(s, vec![(ops[4].span.clone(), b"<41> Tj".to_vec())]);
        assert!(out.starts_with(b"q 1 0 0 1 5 5 cm BT /F1 12 Tf <41> Tj [<0041>"));
    }

    #[test]
    fn filtered_inline_image_and_numbers() {
        let s = b"BI /W 4 /H 4 /F /AHx ID 00ff00ffEI>\nEI 0 g -.5 3. --2 Td";
        let ops = parse(s).unwrap();
        assert_eq!(ops[0].name, b"BI");
        assert_eq!(inline_data(s, &ops[0]).unwrap(), b"00ff00ffEI>");
        assert_eq!(ops[2].nums::<3>().unwrap(), [-0.5, 3.0, -2.0]);
    }

    #[test]
    fn rejects_broken_streams() {
        assert!(parse(b"(unterminated Tj").is_err());
        assert!(parse(b"[1 2 Tj").is_err());
        assert!(parse(b"BI /W 1 ID xyz").is_err());
    }
}
