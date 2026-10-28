//! The Markdown reader behind `md2pdf`: the CommonMark blocks and inlines
//! documents actually use (ATX and setext headings, paragraphs, nested
//! lists, block quotes, fenced and indented code, thematic breaks, emphasis,
//! code spans, links, images, hard breaks, entities), plus GFM tables and
//! strikethrough. Raw HTML is dropped: comments and tags never reach the page.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Style {
    pub bold: bool,
    pub italic: bool,
    pub mono: bool,
    pub link: bool,
}

/// A run of inline text in one style; `"\n"` alone is a hard line break.
#[derive(Debug, Clone, PartialEq)]
pub struct Piece {
    pub text: String,
    pub style: Style,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    None,
    Left,
    Center,
    Right,
}

pub type Cell = Vec<Piece>;

#[derive(Debug, PartialEq)]
pub enum Node {
    Heading(usize, Vec<Piece>),
    Paragraph(Vec<Piece>),
    List {
        ordered: bool,
        start: u64,
        items: Vec<Vec<Node>>,
    },
    Quote(Vec<Node>),
    Code(String),
    Table {
        align: Vec<Align>,
        header: Option<Vec<Cell>>,
        rows: Vec<Vec<Cell>>,
    },
    Rule,
}

/// Read a whole document.
pub fn parse(md: &str) -> Vec<Node> {
    let text = md.replace("\r\n", "\n").replace('\r', "\n");
    let text = strip_comments(&text);
    let lines: Vec<String> = text.split('\n').map(expand_tabs).collect();
    blocks(&lines)
}

fn strip_comments(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(start) = rest.find("<!--") {
        out.push_str(&rest[..start]);
        match rest[start + 4..].find("-->") {
            Some(end) => rest = &rest[start + 4 + end + 3..],
            None => {
                rest = "";
                break;
            }
        }
    }
    out.push_str(rest);
    out
}

fn expand_tabs(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut col = 0;
    for c in line.chars() {
        if c == '\t' {
            let n = 4 - col % 4;
            out.extend(std::iter::repeat_n(' ', n));
            col += n;
        } else {
            out.push(c);
            col += 1;
        }
    }
    out
}

fn indent_of(line: &str) -> usize {
    line.len() - line.trim_start_matches(' ').len()
}

/// `line` without its first `n` spaces (fewer when it has fewer).
fn dedent(line: &str, n: usize) -> String {
    let k = indent_of(line).min(n);
    line[k..].to_string()
}

fn atx(line: &str) -> Option<(usize, &str)> {
    if indent_of(line) > 3 {
        return None;
    }
    let t = line.trim_start();
    let level = t.chars().take_while(|&c| c == '#').count();
    if !(1..=6).contains(&level) {
        return None;
    }
    let rest = &t[level..];
    if !rest.is_empty() && !rest.starts_with(' ') {
        return None;
    }
    // A closing run of #s is decoration when a space precedes it.
    let mut text = rest.trim();
    let stripped = text.trim_end_matches('#');
    if stripped.is_empty() || stripped.ends_with(' ') {
        text = stripped.trim_end();
    }
    Some((level, text))
}

fn is_rule(line: &str) -> bool {
    if indent_of(line) > 3 {
        return false;
    }
    let t: String = line.chars().filter(|c| !c.is_whitespace()).collect();
    let Some(first) = t.chars().next() else {
        return false;
    };
    matches!(first, '-' | '*' | '_') && t.len() >= 3 && t.chars().all(|c| c == first)
}

/// An opening code fence: its character, its length and its indent.
fn fence(line: &str) -> Option<(char, usize, usize)> {
    let ind = indent_of(line);
    if ind > 3 {
        return None;
    }
    let t = &line[ind..];
    let c = t.chars().next()?;
    if c != '`' && c != '~' {
        return None;
    }
    let n = t.chars().take_while(|&x| x == c).count();
    if n < 3 || (c == '`' && t[n..].contains('`')) {
        return None;
    }
    Some((c, n, ind))
}

fn closes_fence(line: &str, c: char, n: usize) -> bool {
    if indent_of(line) > 3 {
        return false;
    }
    let t = line.trim();
    t.chars().take_while(|&x| x == c).count() >= n && t.chars().all(|x| x == c)
}

struct Marker {
    indent: usize,
    ordered: bool,
    start: u64,
    /// The bullet character, or the delimiter after the number.
    kind: char,
    /// Column where the item's content starts.
    content: usize,
}

fn list_marker(line: &str) -> Option<Marker> {
    let indent = indent_of(line);
    if indent > 3 {
        return None;
    }
    let t = &line[indent..];
    let (ordered, start, kind, len) = match t.chars().next()? {
        c @ ('-' | '*' | '+') => (false, 1, c, 1),
        c if c.is_ascii_digit() => {
            let digits = t.chars().take_while(char::is_ascii_digit).count();
            if digits > 9 {
                return None;
            }
            let delim = t[digits..].chars().next()?;
            if delim != '.' && delim != ')' {
                return None;
            }
            (true, t[..digits].parse().ok()?, delim, digits + 1)
        }
        _ => return None,
    };
    let after = &t[len..];
    if !after.is_empty() && !after.starts_with(' ') {
        return None;
    }
    let spaces = after.chars().take_while(|&c| c == ' ').count();
    let gap = if after.trim().is_empty() || spaces > 4 {
        1
    } else {
        spaces
    };
    Some(Marker {
        indent,
        ordered,
        start,
        kind,
        content: indent + len + gap,
    })
}

fn is_html_line(t: &str) -> bool {
    let Some(rest) = t.strip_prefix('<') else {
        return false;
    };
    let rest = rest.strip_prefix('/').unwrap_or(rest);
    rest.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
        && rest
            .chars()
            .take_while(|c| !c.is_whitespace() && *c != '>' && *c != '/')
            .all(|c| c.is_ascii_alphanumeric() || c == '-')
        && !rest.contains(':')
}

/// Cells of a table row, split on pipes outside code spans and escapes.
fn split_cells(line: &str) -> Vec<String> {
    let mut t = line.trim();
    t = t.strip_prefix('|').unwrap_or(t);
    if t.ends_with('|') && !t.ends_with("\\|") {
        t = &t[..t.len() - 1];
    }
    let mut cells = vec![];
    let mut cur = String::new();
    let mut in_code = 0usize;
    let mut chars = t.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' if chars.peek() == Some(&'|') => {
                cur.push('|');
                chars.next();
            }
            '`' => {
                let mut n = 1;
                while chars.peek() == Some(&'`') {
                    chars.next();
                    n += 1;
                }
                in_code = if in_code == n {
                    0
                } else if in_code == 0 {
                    n
                } else {
                    in_code
                };
                cur.extend(std::iter::repeat_n('`', n));
            }
            '|' if in_code == 0 => cells.push(std::mem::take(&mut cur).trim().to_string()),
            c => cur.push(c),
        }
    }
    cells.push(cur.trim().to_string());
    cells
}

fn delimiter_row(line: &str) -> Option<Vec<Align>> {
    if !line.contains('-') {
        return None;
    }
    let cells = split_cells(line);
    let mut out = vec![];
    for c in cells {
        let left = c.starts_with(':');
        let right = c.ends_with(':') && c.len() > 1;
        let core = c.trim_start_matches(':').trim_end_matches(':');
        if core.is_empty() || !core.chars().all(|x| x == '-') {
            return None;
        }
        out.push(match (left, right) {
            (true, true) => Align::Center,
            (true, false) => Align::Left,
            (false, true) => Align::Right,
            _ => Align::None,
        });
    }
    Some(out)
}

fn table_at(lines: &[String], i: usize) -> Option<Vec<Align>> {
    let head = lines.get(i)?;
    if !head.contains('|') || indent_of(head) > 3 {
        return None;
    }
    let align = delimiter_row(lines.get(i + 1)?)?;
    (split_cells(head).len() == align.len()).then_some(align)
}

/// Whether `line` starts a block that interrupts a paragraph.
fn interrupts(lines: &[String], i: usize) -> bool {
    let line = &lines[i];
    let t = line.trim();
    t.is_empty()
        || atx(line).is_some()
        || fence(line).is_some()
        || is_rule(line)
        || (indent_of(line) <= 3 && t.starts_with('>'))
        || list_marker(line).is_some_and(|m| !m.ordered || m.start == 1)
        || table_at(lines, i).is_some()
        || (indent_of(line) <= 3 && is_html_line(t))
}

fn blocks(lines: &[String]) -> Vec<Node> {
    let mut out = vec![];
    let mut i = 0;
    while i < lines.len() {
        let line = &lines[i];
        let t = line.trim();
        if t.is_empty() {
            i += 1;
            continue;
        }
        if indent_of(line) >= 4 {
            // Indented code: up to the first line indented less (blank lines kept).
            let mut code = vec![];
            while i < lines.len() && (indent_of(&lines[i]) >= 4 || lines[i].trim().is_empty()) {
                code.push(dedent(&lines[i], 4));
                i += 1;
            }
            while code.last().is_some_and(|l| l.trim().is_empty()) {
                code.pop();
            }
            out.push(Node::Code(code.join("\n")));
            continue;
        }
        if let Some((c, n, ind)) = fence(line) {
            let mut code = vec![];
            i += 1;
            while i < lines.len() && !closes_fence(&lines[i], c, n) {
                code.push(dedent(&lines[i], ind));
                i += 1;
            }
            i += 1;
            out.push(Node::Code(code.join("\n")));
            continue;
        }
        if let Some((level, text)) = atx(line) {
            out.push(Node::Heading(level, inlines(text)));
            i += 1;
            continue;
        }
        if is_rule(line) {
            out.push(Node::Rule);
            i += 1;
            continue;
        }
        if t.starts_with('>') {
            let mut inner = vec![];
            while i < lines.len() {
                let l = lines[i].trim_start();
                if let Some(rest) = l.strip_prefix('>') {
                    inner.push(rest.strip_prefix(' ').unwrap_or(rest).to_string());
                } else if !l.is_empty()
                    && !inner.last().is_none_or(|p| p.trim().is_empty())
                    && !interrupts(lines, i)
                {
                    inner.push(l.to_string()); // lazy continuation of a quoted paragraph
                } else {
                    break;
                }
                i += 1;
            }
            out.push(Node::Quote(blocks(&inner)));
            continue;
        }
        if let Some(first) = list_marker(line) {
            let mut items = vec![];
            while i < lines.len() {
                let Some(m) = list_marker(&lines[i]) else {
                    break;
                };
                if m.ordered != first.ordered || m.kind != first.kind || m.indent >= first.content {
                    break;
                }
                let mut item = vec![lines[i].get(m.content..).unwrap_or("").to_string()];
                i += 1;
                while i < lines.len() {
                    let l = &lines[i];
                    if l.trim().is_empty() {
                        // A blank line stays in the item when indented content follows.
                        let next = (i + 1..lines.len()).find(|&j| !lines[j].trim().is_empty());
                        match next {
                            Some(j) if indent_of(&lines[j]) >= m.content => {
                                item.push(String::new());
                                i += 1;
                            }
                            _ => break,
                        }
                    } else if indent_of(l) >= m.content {
                        item.push(dedent(l, m.content));
                        i += 1;
                    } else if list_marker(l).is_none()
                        && !interrupts(lines, i)
                        && item.last().is_some_and(|p| !p.trim().is_empty())
                    {
                        item.push(l.trim_start().to_string()); // lazy continuation
                        i += 1;
                    } else {
                        break;
                    }
                }
                items.push(blocks(&item));
                // Blank lines between items keep the list going.
                let mut j = i;
                while j < lines.len() && lines[j].trim().is_empty() {
                    j += 1;
                }
                match lines.get(j).and_then(|l| list_marker(l)) {
                    Some(next)
                        if next.ordered == first.ordered
                            && next.kind == first.kind
                            && next.indent < first.content =>
                    {
                        i = j
                    }
                    _ => break,
                }
            }
            out.push(Node::List {
                ordered: first.ordered,
                start: first.start,
                items,
            });
            continue;
        }
        if let Some(align) = table_at(lines, i) {
            let cells = |l: &str| -> Vec<Cell> {
                let mut c: Vec<Cell> = split_cells(l).iter().map(|s| inlines(s)).collect();
                c.resize(align.len(), vec![]);
                c
            };
            let header = cells(line);
            let mut rows = vec![];
            i += 2;
            while i < lines.len() && !lines[i].trim().is_empty() && lines[i].contains('|') {
                rows.push(cells(&lines[i]));
                i += 1;
            }
            out.push(Node::Table {
                align,
                header: Some(header),
                rows,
            });
            continue;
        }
        if is_html_line(t) {
            while i < lines.len() && !lines[i].trim().is_empty() {
                i += 1;
            }
            continue;
        }
        // Paragraph, or a setext heading when underlined with = or -.
        let mut para = vec![line.trim_start().to_string()];
        i += 1;
        let mut level = 0;
        while i < lines.len() {
            let l = &lines[i];
            let u = l.trim();
            if indent_of(l) <= 3
                && !u.is_empty()
                && (u.chars().all(|c| c == '=') || u.chars().all(|c| c == '-'))
            {
                level = if u.starts_with('=') { 1 } else { 2 };
                i += 1;
                break;
            }
            if interrupts(lines, i) {
                break;
            }
            para.push(l.trim_start().to_string());
            i += 1;
        }
        let text = para.join("\n");
        let text = text.trim_end();
        out.push(if level > 0 {
            Node::Heading(level, inlines(text))
        } else {
            Node::Paragraph(inlines(text))
        });
    }
    out
}

// ------------------------------------------------------------------ inlines

/// Inline content of one block.
pub fn inlines(s: &str) -> Vec<Piece> {
    let chars: Vec<char> = s.chars().collect();
    let mut out = vec![];
    inline(&chars, Style::default(), &mut out);
    // A hard break at the very end has nothing to break.
    while out.last().is_some_and(|p| p.text == "\n") {
        out.pop();
    }
    out
}

fn push(out: &mut Vec<Piece>, text: &str, style: Style) {
    if text.is_empty() {
        return;
    }
    match out.last_mut() {
        Some(p) if p.style == style && p.text != "\n" && text != "\n" => p.text.push_str(text),
        _ => out.push(Piece {
            text: text.to_string(),
            style,
        }),
    }
}

fn find_seq(chars: &[char], from: usize, seq: &[char]) -> Option<usize> {
    (from..chars.len().saturating_sub(seq.len() - 1)).find(|&k| chars[k..].starts_with(seq))
}

/// The `]` matching the `[` at `open`, skipping escapes and nested brackets.
fn close_bracket(chars: &[char], open: usize) -> Option<usize> {
    let mut depth = 0;
    let mut k = open;
    while k < chars.len() {
        match chars[k] {
            '\\' => k += 1,
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    return Some(k);
                }
            }
            _ => {}
        }
        k += 1;
    }
    None
}

/// `(destination "title")` after a link's text: the index past its `)`.
fn link_end(chars: &[char], at: usize) -> Option<usize> {
    if chars.get(at) != Some(&'(') {
        return None;
    }
    let mut k = at + 1;
    if chars.get(k) == Some(&'<') {
        while k < chars.len() && chars[k] != '>' {
            k += 1;
        }
    }
    let mut depth = 1;
    while k < chars.len() {
        match chars[k] {
            '\\' => k += 1,
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(k + 1);
                }
            }
            '\n' => return None,
            _ => {}
        }
        k += 1;
    }
    None
}

fn entity(chars: &[char], at: usize) -> Option<(String, usize)> {
    let end = (at + 1..chars.len().min(at + 12)).find(|&k| chars[k] == ';')?;
    let name: String = chars[at + 1..end].iter().collect();
    let c = match name.as_str() {
        "amp" => '&',
        "lt" => '<',
        "gt" => '>',
        "quot" => '"',
        "apos" => '\'',
        "nbsp" => '\u{a0}',
        "eacute" => 'é',
        "egrave" => 'è',
        "agrave" => 'à',
        "ccedil" => 'ç',
        "euro" => '€',
        "laquo" => '«',
        "raquo" => '»',
        "hellip" => '…',
        "mdash" => '—',
        "ndash" => '–',
        "copy" => '©',
        _ => {
            let num = name.strip_prefix('#')?;
            let v = match num.strip_prefix(['x', 'X']) {
                Some(hex) => u32::from_str_radix(hex, 16).ok()?,
                None => num.parse().ok()?,
            };
            char::from_u32(v).filter(|c| *c != '\0')?
        }
    };
    Some((c.to_string(), end + 1))
}

fn inline(chars: &[char], style: Style, out: &mut Vec<Piece>) {
    let mut i = 0;
    let mut text = String::new();
    macro_rules! flush {
        () => {
            push(out, &std::mem::take(&mut text), style)
        };
    }
    while i < chars.len() {
        let c = chars[i];
        match c {
            '\\' if i + 1 < chars.len() && chars[i + 1] == '\n' => {
                flush!();
                push(out, "\n", style);
                i += 2;
            }
            '\\' if i + 1 < chars.len() && chars[i + 1].is_ascii_punctuation() => {
                text.push(chars[i + 1]);
                i += 2;
            }
            '\n' => {
                if text.ends_with("  ") {
                    text.truncate(text.trim_end_matches(' ').len());
                    flush!();
                    push(out, "\n", style);
                } else {
                    text.truncate(text.trim_end_matches(' ').len());
                    text.push(' ');
                }
                i += 1;
                while i < chars.len() && chars[i] == ' ' {
                    i += 1;
                }
            }
            '`' => {
                let n = chars[i..].iter().take_while(|&&x| x == '`').count();
                let fence = vec!['`'; n];
                let mut k = i + n;
                let close = loop {
                    match find_seq(chars, k, &fence) {
                        Some(p) if chars.get(p + n) == Some(&'`') => {
                            k = p + n + chars[p + n..].iter().take_while(|&&x| x == '`').count();
                        }
                        other => break other,
                    }
                };
                match close {
                    Some(p) => {
                        flush!();
                        let mut code: String = chars[i + n..p]
                            .iter()
                            .map(|&x| if x == '\n' { ' ' } else { x })
                            .collect();
                        if code.len() > 2
                            && code.starts_with(' ')
                            && code.ends_with(' ')
                            && !code.trim().is_empty()
                        {
                            code = code[1..code.len() - 1].to_string();
                        }
                        push(
                            out,
                            &code,
                            Style {
                                mono: true,
                                ..style
                            },
                        );
                        i = p + n;
                    }
                    None => {
                        text.extend(&chars[i..i + n]);
                        i += n;
                    }
                }
            }
            '!' if chars.get(i + 1) == Some(&'[') => match close_bracket(chars, i + 1)
                .and_then(|b| link_end(chars, b + 1).map(|e| (b, e)))
            {
                Some((b, end)) => {
                    flush!();
                    let alt = Style {
                        italic: true,
                        ..style
                    };
                    push(out, "[image : ", alt);
                    inline(&chars[i + 2..b], alt, out);
                    push(out, "]", alt);
                    i = end;
                }
                None => {
                    text.push('!');
                    i += 1;
                }
            },
            '[' => match close_bracket(chars, i) {
                Some(b) if link_end(chars, b + 1).is_some() => {
                    flush!();
                    inline(
                        &chars[i + 1..b],
                        Style {
                            link: true,
                            ..style
                        },
                        out,
                    );
                    i = link_end(chars, b + 1).unwrap();
                }
                Some(b)
                    if chars.get(b + 1) == Some(&'[') && close_bracket(chars, b + 1).is_some() =>
                {
                    // Reference link [text][ref]: the text, as a link.
                    flush!();
                    inline(
                        &chars[i + 1..b],
                        Style {
                            link: true,
                            ..style
                        },
                        out,
                    );
                    i = close_bracket(chars, b + 1).unwrap() + 1;
                }
                _ => {
                    text.push('[');
                    i += 1;
                }
            },
            '<' => {
                let end = (i + 1..chars.len())
                    .find(|&k| chars[k] == '>' || chars[k] == '<' || chars[k] == '\n');
                let inner: String = end
                    .filter(|&e| chars[e] == '>')
                    .map(|e| chars[i + 1..e].iter().collect())
                    .unwrap_or_default();
                let e = end.unwrap_or(i);
                if !inner.is_empty()
                    && !inner.contains(' ')
                    && (inner.contains("://")
                        || inner.starts_with("mailto:")
                        || (inner.contains('@') && !inner.starts_with('/')))
                {
                    flush!();
                    push(
                        out,
                        inner.strip_prefix("mailto:").unwrap_or(&inner),
                        Style {
                            link: true,
                            ..style
                        },
                    );
                    i = e + 1;
                } else if !inner.is_empty() && is_html_line(&format!("<{inner}>")) {
                    if inner
                        .trim_end_matches('/')
                        .trim()
                        .eq_ignore_ascii_case("br")
                    {
                        flush!();
                        push(out, "\n", style);
                    }
                    i = e + 1;
                } else {
                    text.push('<');
                    i += 1;
                }
            }
            '&' => match entity(chars, i) {
                Some((s, end)) => {
                    text.push_str(&s);
                    i = end;
                }
                None => {
                    text.push('&');
                    i += 1;
                }
            },
            '~' if chars.get(i + 1) == Some(&'~') => match find_seq(chars, i + 2, &['~', '~']) {
                Some(p) if p > i + 2 => {
                    flush!();
                    inline(&chars[i + 2..p], style, out);
                    i = p + 2;
                }
                _ => {
                    text.push_str("~~");
                    i += 2;
                }
            },
            '*' | '_' => {
                let n = chars[i..].iter().take_while(|&&x| x == c).count();
                let prev = i.checked_sub(1).map(|k| chars[k]);
                let next = chars.get(i + n).copied();
                let intraword = c == '_'
                    && prev.is_some_and(char::is_alphanumeric)
                    && next.is_some_and(char::is_alphanumeric);
                let opens = next.is_some_and(|x| !x.is_whitespace()) && !intraword;
                let k = n.min(3);
                let run = vec![c; k];
                // The closer: the same run, not after a space, not inside a longer run.
                let close = opens
                    .then(|| {
                        let mut from = i + k;
                        loop {
                            let p = find_seq(chars, from, &run)?;
                            let before = chars[p - 1];
                            let after = chars.get(p + k).copied();
                            let longer = after == Some(c);
                            let word_after = c == '_' && after.is_some_and(char::is_alphanumeric);
                            if p > i + k && !before.is_whitespace() && !longer && !word_after {
                                return Some(p);
                            }
                            from = p + 1;
                            if from >= chars.len() {
                                return None;
                            }
                        }
                    })
                    .flatten();
                match close {
                    Some(p) => {
                        flush!();
                        let s = Style {
                            bold: style.bold || k >= 2,
                            italic: style.italic || k != 2,
                            ..style
                        };
                        inline(&chars[i + k..p], s, out);
                        i = p + k;
                    }
                    None => {
                        text.extend(&chars[i..i + n]);
                        i += n;
                    }
                }
            }
            c => {
                text.push(c);
                i += 1;
            }
        }
    }
    flush!();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain(p: &[Piece]) -> String {
        p.iter().map(|x| x.text.as_str()).collect()
    }

    fn styled(p: &[Piece], f: impl Fn(&Style) -> bool) -> String {
        p.iter()
            .filter(|x| f(&x.style))
            .map(|x| x.text.as_str())
            .collect()
    }

    #[test]
    fn emphasis_code_links_and_escapes() {
        let p = inlines(
            "Le **prêt de *154 312 €*** est `CIF` et [la note](<a b.md>), 2 \\* 3, a_b_c, *pas fermé",
        );
        assert_eq!(
            plain(&p),
            "Le prêt de 154 312 € est CIF et la note, 2 * 3, a_b_c, *pas fermé"
        );
        assert_eq!(styled(&p, |s| s.bold), "prêt de 154 312 €");
        assert_eq!(styled(&p, |s| s.italic), "154 312 €");
        assert_eq!(styled(&p, |s| s.mono), "CIF");
        assert_eq!(styled(&p, |s| s.link), "la note");
        let q = inlines(
            "ligne 1  \nligne 2\\\nligne 3\nsuite <br> fin &amp; &#233; <span>x</span> <https://a.fr/b> ![alt](i.png) ~~barré~~",
        );
        assert_eq!(
            plain(&q),
            "ligne 1\nligne 2\nligne 3 suite \n fin & é x https://a.fr/b [image : alt] barré"
        );
    }

    #[test]
    fn blocks_lists_quotes_tables_and_code() {
        let md = "<!-- généré -->\n# Titre #\n\nUn paragraphe\nsur deux lignes.\n\nSous-titre\n---\n\n\
                  - un\n- deux\n  1. a\n  2. b\n- trois\n\n> cité\n> encore\n\n\
                  | A | B |\n|:--|--:|\n| 1 | x \\| y |\n| 2 |\n\n```rust\nlet x = 1;\n\n  y\n```\n\n    indenté\n\n***\n\n<div>html</div>\n\nfin";
        let n = parse(md);
        assert!(matches!(&n[0], Node::Heading(1, p) if plain(p) == "Titre"));
        assert!(
            matches!(&n[1], Node::Paragraph(p) if plain(p) == "Un paragraphe sur deux lignes.")
        );
        assert!(matches!(&n[2], Node::Heading(2, p) if plain(p) == "Sous-titre"));
        let Node::List {
            ordered: false,
            items,
            ..
        } = &n[3]
        else {
            panic!("{:?}", n[3])
        };
        assert_eq!(items.len(), 3);
        assert!(
            matches!(&items[1][1], Node::List { ordered: true, items, .. } if items.len() == 2)
        );
        assert!(
            matches!(&n[4], Node::Quote(q) if matches!(&q[0], Node::Paragraph(p) if plain(p) == "cité encore"))
        );
        let Node::Table {
            align,
            header,
            rows,
        } = &n[5]
        else {
            panic!("{:?}", n[5])
        };
        assert_eq!(align, &[Align::Left, Align::Right]);
        assert_eq!(plain(&header.as_ref().unwrap()[1]), "B");
        assert_eq!(plain(&rows[0][1]), "x | y");
        assert!(rows[1][1].is_empty(), "short rows are padded");
        assert_eq!(n[6], Node::Code("let x = 1;\n\n  y".into()));
        assert_eq!(n[7], Node::Code("indenté".into()));
        assert_eq!(n[8], Node::Rule);
        assert!(
            matches!(&n[9], Node::Paragraph(p) if plain(p) == "fin"),
            "HTML blocks are dropped: {:?}",
            n.get(9)
        );
        assert_eq!(n.len(), 10);
    }

    #[test]
    fn loose_lists_and_lazy_lines() {
        let n = parse(
            "1. premier\nsuite paresseuse\n\n2. second\n\n   avec un paragraphe\n\nhors liste",
        );
        let Node::List {
            ordered: true,
            start: 1,
            items,
        } = &n[0]
        else {
            panic!("{n:?}")
        };
        assert!(
            matches!(&items[0][0], Node::Paragraph(p) if plain(p) == "premier suite paresseuse")
        );
        assert_eq!(items[1].len(), 2);
        assert!(matches!(&n[1], Node::Paragraph(p) if plain(p) == "hors liste"));
        // A number that does not start at 1 does not interrupt a paragraph.
        assert_eq!(parse("En 2011\n2. pas une liste").len(), 1);
    }
}
