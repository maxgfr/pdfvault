//! Markdown (as produced by [`crate::markdown`]) → EPUB 3, with a minimal,
//! deterministic ZIP writer.

use serde::Deserialize;

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct EpubOptions {
    pub title: String,
    pub author: String,
    /// BCP 47 language tag.
    pub language: String,
    /// Stable identifier; derived from the content when empty.
    pub identifier: String,
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Undo the backslash escapes the Markdown writer adds, then escape for XML.
fn inline(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(n) = chars.next() {
                out.push(n);
            }
        } else {
            out.push(c);
        }
    }
    esc(&out)
}

struct Chapter {
    title: String,
    body: String,
}

fn chapters(md: &str, fallback_title: &str) -> Vec<Chapter> {
    let mut out: Vec<Chapter> = vec![];
    let mut list: Option<&str> = None;
    let close_list = |list: &mut Option<&str>, body: &mut String| {
        if let Some(tag) = list.take() {
            body.push_str(&format!("</{tag}>\n"));
        }
    };
    for block in md.split("\n\n").map(str::trim).filter(|b| !b.is_empty()) {
        let heading = block.chars().take_while(|c| *c == '#').count();
        if heading == 1 || out.is_empty() {
            if let Some(c) = out.last_mut() {
                close_list(&mut list, &mut c.body);
            }
            let title = if heading == 1 {
                inline(block[1..].trim())
            } else {
                esc(fallback_title)
            };
            out.push(Chapter {
                title,
                body: String::new(),
            });
            if heading == 1 {
                let t = out.last().unwrap().title.clone();
                out.last_mut()
                    .unwrap()
                    .body
                    .push_str(&format!("<h1>{t}</h1>\n"));
                continue;
            }
        }
        let body = &mut out.last_mut().unwrap().body;
        let (tag, item) = if let Some(rest) = block.strip_prefix("- ") {
            (Some("ul"), rest)
        } else if let Some(pos) = block
            .find(". ")
            .filter(|p| *p < 4 && block[..*p].chars().all(|c| c.is_ascii_digit()))
        {
            (Some("ol"), &block[pos + 2..])
        } else {
            (None, block)
        };
        if tag != list {
            close_list(&mut list, body);
            if let Some(t) = tag {
                body.push_str(&format!("<{t}>\n"));
                list = Some(t);
            }
        }
        if tag.is_some() {
            body.push_str(&format!("<li>{}</li>\n", inline(item)));
        } else if (2..=6).contains(&heading) {
            body.push_str(&format!(
                "<h{heading}>{}</h{heading}>\n",
                inline(block[heading..].trim())
            ));
        } else if block == "---" {
            body.push_str("<hr/>\n");
        } else {
            body.push_str(&format!("<p>{}</p>\n", inline(block)));
        }
    }
    if let Some(c) = out.last_mut() {
        close_list(&mut list, &mut c.body);
    }
    if out.is_empty() {
        out.push(Chapter {
            title: esc(fallback_title),
            body: "<p></p>\n".into(),
        });
    }
    out
}

fn xhtml(title: &str, body: &str, lang: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<!DOCTYPE html>\n<html xmlns=\"http://www.w3.org/1999/xhtml\" xmlns:epub=\"http://www.idpf.org/2007/ops\" lang=\"{lang}\" xml:lang=\"{lang}\">\n<head><meta charset=\"utf-8\"/><title>{title}</title><link rel=\"stylesheet\" href=\"style.css\"/></head>\n<body>\n{body}</body>\n</html>\n"
    )
}

pub fn build(md: &str, o: &EpubOptions) -> Vec<u8> {
    let title = if o.title.trim().is_empty() {
        "Untitled".to_string()
    } else {
        o.title.trim().to_string()
    };
    let lang = if o.language.is_empty() {
        "en".to_string()
    } else {
        esc(&o.language)
    };
    let id = if o.identifier.is_empty() {
        format!("urn:pdfvault:{:08x}", crc32fast::hash(md.as_bytes()))
    } else {
        esc(&o.identifier)
    };
    let chs = chapters(md, &title);
    let mut files: Vec<(String, Vec<u8>)> = vec![];
    files.push(("mimetype".into(), b"application/epub+zip".to_vec()));
    files.push((
        "META-INF/container.xml".into(),
        b"<?xml version=\"1.0\"?>\n<container version=\"1.0\" xmlns=\"urn:oasis:names:tc:opendocument:xmlns:container\"><rootfiles><rootfile full-path=\"OEBPS/content.opf\" media-type=\"application/oebps-package+xml\"/></rootfiles></container>\n".to_vec(),
    ));
    let mut manifest = String::new();
    let mut spine = String::new();
    let mut nav = String::new();
    for (i, c) in chs.iter().enumerate() {
        let name = format!("chapter-{}.xhtml", i + 1);
        files.push((
            format!("OEBPS/{name}"),
            xhtml(&c.title, &c.body, &lang).into_bytes(),
        ));
        manifest.push_str(&format!(
            "<item id=\"c{}\" href=\"{name}\" media-type=\"application/xhtml+xml\"/>\n",
            i + 1
        ));
        spine.push_str(&format!("<itemref idref=\"c{}\"/>\n", i + 1));
        nav.push_str(&format!("<li><a href=\"{name}\">{}</a></li>\n", c.title));
    }
    files.push((
        "OEBPS/nav.xhtml".into(),
        xhtml(
            &esc(&title),
            &format!("<nav epub:type=\"toc\" id=\"toc\"><h1>Contents</h1><ol>\n{nav}</ol></nav>\n"),
            &lang,
        )
        .into_bytes(),
    ));
    files.push((
        "OEBPS/style.css".into(),
        b"body{font-family:serif;line-height:1.5;margin:0 5%}h1,h2,h3{font-family:sans-serif;line-height:1.2}p{margin:0 0 .8em}\n".to_vec(),
    ));
    let author = if o.author.trim().is_empty() {
        String::new()
    } else {
        format!("<dc:creator>{}</dc:creator>\n", esc(o.author.trim()))
    };
    let opf = format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<package xmlns=\"http://www.idpf.org/2007/opf\" version=\"3.0\" unique-identifier=\"uid\">\n<metadata xmlns:dc=\"http://purl.org/dc/elements/1.1/\">\n<dc:identifier id=\"uid\">{id}</dc:identifier>\n<dc:title>{}</dc:title>\n{author}<dc:language>{lang}</dc:language>\n<meta property=\"dcterms:modified\">2000-01-01T00:00:00Z</meta>\n</metadata>\n<manifest>\n<item id=\"nav\" href=\"nav.xhtml\" media-type=\"application/xhtml+xml\" properties=\"nav\"/>\n<item id=\"css\" href=\"style.css\" media-type=\"text/css\"/>\n{manifest}</manifest>\n<spine>\n{spine}</spine>\n</package>\n",
        esc(&title)
    );
    files.push(("OEBPS/content.opf".into(), opf.into_bytes()));
    zip(&files)
}

/// ZIP with `mimetype` stored first (as EPUB requires) and the rest deflated.
/// Timestamps are fixed so the same input always gives the same bytes.
pub fn zip(files: &[(String, Vec<u8>)]) -> Vec<u8> {
    let mut out = vec![];
    let mut central = vec![];
    let (time, date) = (0u16, 0x21u16); // 1980-01-01 00:00
    for (name, data) in files {
        let stored = name == "mimetype";
        let body = if stored {
            data.clone()
        } else {
            miniz_oxide::deflate::compress_to_vec(data, 9)
        };
        let method: u16 = if stored { 0 } else { 8 };
        let crc = crc32fast::hash(data);
        let offset = out.len() as u32;
        let header = |sig: u32, central: bool| {
            let mut h = vec![];
            h.extend(sig.to_le_bytes());
            if central {
                h.extend(20u16.to_le_bytes());
            }
            h.extend(20u16.to_le_bytes());
            h.extend(0u16.to_le_bytes());
            h.extend(method.to_le_bytes());
            h.extend(time.to_le_bytes());
            h.extend(date.to_le_bytes());
            h.extend(crc.to_le_bytes());
            h.extend((body.len() as u32).to_le_bytes());
            h.extend((data.len() as u32).to_le_bytes());
            h.extend((name.len() as u16).to_le_bytes());
            h.extend(0u16.to_le_bytes());
            if central {
                // comment length, disk number, internal attributes; then external attributes.
                h.extend([0u8; 6]);
                h.extend(0u32.to_le_bytes());
                h.extend(offset.to_le_bytes());
            }
            h.extend(name.as_bytes());
            h
        };
        out.extend(header(0x04034b50, false));
        out.extend(&body);
        central.extend(header(0x02014b50, true));
    }
    let cd_offset = out.len() as u32;
    out.extend(&central);
    out.extend(0x06054b50u32.to_le_bytes());
    out.extend([0u8; 4]);
    out.extend((files.len() as u16).to_le_bytes());
    out.extend((files.len() as u16).to_le_bytes());
    out.extend((central.len() as u32).to_le_bytes());
    out.extend(cd_offset.to_le_bytes());
    out.extend(0u16.to_le_bytes());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unzip(z: &[u8]) -> Vec<(String, Vec<u8>)> {
        let mut out = vec![];
        let mut i = 0;
        while i + 30 <= z.len() && z[i..i + 4] == 0x04034b50u32.to_le_bytes() {
            let method = u16::from_le_bytes([z[i + 8], z[i + 9]]);
            let csize = u32::from_le_bytes(z[i + 18..i + 22].try_into().unwrap()) as usize;
            let nlen = u16::from_le_bytes([z[i + 26], z[i + 27]]) as usize;
            let name = String::from_utf8(z[i + 30..i + 30 + nlen].to_vec()).unwrap();
            let body = &z[i + 30 + nlen..i + 30 + nlen + csize];
            let data = if method == 0 {
                body.to_vec()
            } else {
                miniz_oxide::inflate::decompress_to_vec(body).unwrap()
            };
            out.push((name, data));
            i += 30 + nlen + csize;
        }
        out
    }

    /// Read the archive the way unzip does: through the central directory.
    fn central_names(z: &[u8]) -> Vec<String> {
        let eocd = z
            .windows(4)
            .rposition(|w| w == 0x06054b50u32.to_le_bytes())
            .unwrap();
        let count = u16::from_le_bytes([z[eocd + 10], z[eocd + 11]]) as usize;
        let mut at = u32::from_le_bytes(z[eocd + 16..eocd + 20].try_into().unwrap()) as usize;
        let mut names = vec![];
        for _ in 0..count {
            assert_eq!(z[at..at + 4], 0x02014b50u32.to_le_bytes());
            let nlen = u16::from_le_bytes([z[at + 28], z[at + 29]]) as usize;
            let extra = u16::from_le_bytes([z[at + 30], z[at + 31]]) as usize;
            let comment = u16::from_le_bytes([z[at + 32], z[at + 33]]) as usize;
            let local = u32::from_le_bytes(z[at + 42..at + 46].try_into().unwrap()) as usize;
            assert_eq!(z[local..local + 4], 0x04034b50u32.to_le_bytes());
            names.push(String::from_utf8(z[at + 46..at + 46 + nlen].to_vec()).unwrap());
            at += 46 + nlen + extra + comment;
        }
        names
    }

    #[test]
    fn central_directory_is_valid() {
        let z = zip(&[
            ("mimetype".into(), b"x".to_vec()),
            ("a/b.txt".into(), b"hello".to_vec()),
        ]);
        assert_eq!(central_names(&z), ["mimetype", "a/b.txt"]);
    }

    #[test]
    fn epub_layout() {
        let md = "# One\n\nHello & welcome.\n\n- a\n\n- b\n\n# Two\n\n## Sub\n\nText \\*star\\*\n";
        let e = build(
            md,
            &EpubOptions {
                title: "Book".into(),
                author: "Ada".into(),
                ..Default::default()
            },
        );
        let files = unzip(&e);
        assert_eq!(
            files[0],
            ("mimetype".to_string(), b"application/epub+zip".to_vec())
        );
        let ch1 = String::from_utf8(
            files
                .iter()
                .find(|f| f.0 == "OEBPS/chapter-1.xhtml")
                .unwrap()
                .1
                .clone(),
        )
        .unwrap();
        assert!(
            ch1.contains("<h1>One</h1>")
                && ch1.contains("Hello &amp; welcome.")
                && ch1.contains("<ul>\n<li>a</li>\n<li>b</li>\n</ul>")
        );
        let ch2 = String::from_utf8(
            files
                .iter()
                .find(|f| f.0 == "OEBPS/chapter-2.xhtml")
                .unwrap()
                .1
                .clone(),
        )
        .unwrap();
        assert!(ch2.contains("<h2>Sub</h2>") && ch2.contains("Text *star*"));
        assert_eq!(
            build(md, &EpubOptions::default()),
            build(md, &EpubOptions::default())
        );
    }
}
