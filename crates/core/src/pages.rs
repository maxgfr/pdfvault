//! Page-level tools. Most of them reduce to "build a document from this list
//! of (source, page) pairs", which [`assemble`] does once for all of them.

use crate::error::{Error, Result};
use crate::pdf::{self, n};
use lopdf::{Dictionary, Document, Object, ObjectId, Stream, dictionary};
use serde::Deserialize;
use std::collections::HashMap;

/// Build one document from pages picked across `sources`. The first source's
/// catalog extras (outlines, names, forms) are kept only when `keep_catalog`
/// is set, which is right when every page of a single source survives.
pub fn assemble(
    sources: &mut [Document],
    picks: &[(usize, usize)],
    keep_catalog: bool,
) -> Result<Vec<u8>> {
    if picks.is_empty() {
        return Err(Error::option("no pages selected"));
    }
    let mut out = Document::with_version("1.7");
    let mut max_id = 1;
    let mut page_maps: Vec<Vec<ObjectId>> = vec![];
    let mut first_catalog: Option<Dictionary> = None;
    for (i, src) in sources.iter_mut().enumerate() {
        for p in pdf::page_ids(src) {
            pdf::materialize(src, p);
        }
        src.renumber_objects_with(max_id);
        max_id = src.max_id + 1;
        page_maps.push(pdf::page_ids(src));
        if i == 0 && keep_catalog {
            first_catalog = src.catalog().ok().cloned();
        }
        for (id, obj) in std::mem::take(&mut src.objects) {
            out.objects.insert(id, obj);
        }
        if i == 0
            && let Ok(info) = src.trailer.get(b"Info")
        {
            out.trailer.set("Info", info.clone());
        }
    }
    out.max_id = max_id;
    let pages_id = out.new_object_id();
    let mut used: HashMap<ObjectId, usize> = HashMap::new();
    let mut kids = vec![];
    for &(s, p) in picks {
        let src_page = *page_maps.get(s).and_then(|m| m.get(p)).ok_or_else(|| {
            Error::option(format!("page {} of input {} does not exist", p + 1, s + 1))
        })?;
        let count = used.entry(src_page).or_insert(0);
        let id = if *count == 0 {
            src_page
        } else {
            // The same page twice needs two page objects; content is shared.
            let mut dict = out.get_dictionary(src_page)?.clone();
            dict.remove(b"Annots");
            out.add_object(dict)
        };
        *count += 1;
        let page = out.get_dictionary_mut(id)?;
        page.set("Parent", pages_id);
        page.remove(b"StructParents");
        kids.push(Object::Reference(id));
    }
    let count = kids.len() as i64;
    out.objects.insert(
        pages_id,
        dictionary! { "Type" => "Pages", "Kids" => kids, "Count" => count }.into(),
    );
    let mut catalog = dictionary! { "Type" => "Catalog", "Pages" => pages_id };
    if let Some(old) = first_catalog {
        for key in [
            &b"Outlines"[..],
            b"Names",
            b"AcroForm",
            b"PageLabels",
            b"ViewerPreferences",
            b"PageMode",
            b"Lang",
            b"MarkInfo",
            b"StructTreeRoot",
            b"Dests",
        ] {
            if let Ok(v) = old.get(key) {
                catalog.set(key, v.clone());
            }
        }
    }
    let root = out.add_object(catalog);
    out.trailer.set("Root", root);
    pdf::save(&mut out)
}

fn load_all(inputs: &[Vec<u8>], password: Option<&str>) -> Result<Vec<Document>> {
    inputs.iter().map(|b| pdf::load(b, password)).collect()
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PagesOptions {
    pub password: Option<String>,
    /// Page selection such as `1-3,7`.
    pub pages: Option<String>,
}

pub fn merge(inputs: &[Vec<u8>], password: Option<&str>) -> Result<Vec<u8>> {
    if inputs.len() < 2 {
        return Err(Error::option("merge needs at least two PDFs"));
    }
    let mut docs = load_all(inputs, password)?;
    let picks: Vec<(usize, usize)> = docs
        .iter()
        .enumerate()
        .flat_map(|(s, d)| (0..d.get_pages().len()).map(move |p| (s, p)))
        .collect();
    let titles = bookmark_starts(&docs);
    let mut out = pdf::load(&assemble(&mut docs, &picks, false)?, None)?;
    add_flat_outline(&mut out, &titles)?;
    pdf::save(&mut out)
}

/// One bookmark per merged file, named after its title or "Document n".
fn bookmark_starts(docs: &[Document]) -> Vec<(String, usize)> {
    let mut start = 0;
    docs.iter()
        .enumerate()
        .map(|(i, d)| {
            let title = d
                .trailer
                .get(b"Info")
                .ok()
                .and_then(|o| pdf::resolve(d, o).as_dict().ok())
                .and_then(|info| info.get(b"Title").ok())
                .and_then(crate::meta::decode_text)
                .filter(|t| !t.trim().is_empty())
                .unwrap_or_else(|| format!("Document {}", i + 1));
            let entry = (title, start);
            start += d.get_pages().len();
            entry
        })
        .collect()
}

fn add_flat_outline(doc: &mut Document, entries: &[(String, usize)]) -> Result<()> {
    let pages = pdf::page_ids(doc);
    let outlines_id = doc.new_object_id();
    let ids: Vec<ObjectId> = entries.iter().map(|_| doc.new_object_id()).collect();
    for (i, (title, page)) in entries.iter().enumerate() {
        let mut d = dictionary! {
            "Title" => crate::meta::encode_text(title),
            "Parent" => outlines_id,
            "Dest" => vec![pages[*page].into(), "Fit".into()],
        };
        if i > 0 {
            d.set("Prev", ids[i - 1]);
        }
        if i + 1 < ids.len() {
            d.set("Next", ids[i + 1]);
        }
        doc.objects.insert(ids[i], d.into());
    }
    doc.objects.insert(
        outlines_id,
        dictionary! { "Type" => "Outlines", "First" => ids[0], "Last" => *ids.last().unwrap(), "Count" => ids.len() as i64 }
            .into(),
    );
    let root = doc.trailer.get(b"Root")?.as_reference()?;
    doc.get_dictionary_mut(root)?.set("Outlines", outlines_id);
    Ok(())
}

/// Keep the pages in `spec` (1-based, ordered, duplicates allowed).
pub fn select(input: &[u8], spec: &str, password: Option<&str>) -> Result<Vec<u8>> {
    let doc = pdf::load(input, password)?;
    let count = doc.get_pages().len();
    let picks: Vec<(usize, usize)> = pdf::parse_ranges(spec, count)?
        .into_iter()
        .map(|p| (0, p))
        .collect();
    let keep_all = picks.len() == count && {
        let mut v: Vec<usize> = picks.iter().map(|p| p.1).collect();
        v.sort();
        v.dedup();
        v.len() == count
    };
    assemble(&mut [doc], &picks, keep_all)
}

pub fn delete(input: &[u8], spec: &str, password: Option<&str>) -> Result<Vec<u8>> {
    let doc = pdf::load(input, password)?;
    let count = doc.get_pages().len();
    let drop: std::collections::HashSet<usize> =
        pdf::parse_ranges(spec, count)?.into_iter().collect();
    let picks: Vec<(usize, usize)> = (0..count)
        .filter(|p| !drop.contains(p))
        .map(|p| (0, p))
        .collect();
    if picks.is_empty() {
        return Err(Error::option("cannot delete every page"));
    }
    assemble(&mut [doc], &picks, false)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", tag = "mode")]
pub enum SplitMode {
    /// Each range becomes one file: `1-3,4-6`.
    Ranges { ranges: String },
    /// Chunks of `every` pages.
    Every { every: usize },
    /// One file per top-level bookmark.
    Bookmarks,
}

/// Split into several documents, returned with a suggested name suffix each.
pub fn split(
    input: &[u8],
    mode: &SplitMode,
    password: Option<&str>,
) -> Result<Vec<(String, Vec<u8>)>> {
    let doc = pdf::load(input, password)?;
    let count = doc.get_pages().len();
    let groups: Vec<(String, Vec<usize>)> = match mode {
        SplitMode::Ranges { ranges } => ranges
            .split(',')
            .map(str::trim)
            .filter(|r| !r.is_empty())
            .map(|r| Ok((r.replace(' ', ""), pdf::parse_ranges(r, count)?)))
            .collect::<Result<_>>()?,
        SplitMode::Every { every } => {
            if *every == 0 {
                return Err(Error::option("every must be at least 1"));
            }
            (0..count)
                .step_by(*every)
                .map(|s| {
                    let e = (s + every).min(count);
                    (format!("{}-{}", s + 1, e), (s..e).collect())
                })
                .collect()
        }
        SplitMode::Bookmarks => {
            let starts = top_level_bookmarks(&doc);
            if starts.is_empty() {
                return Err(Error::option("this PDF has no bookmarks to split on"));
            }
            let mut groups = vec![];
            if starts[0].1 > 0 {
                groups.push(("front".to_string(), (0..starts[0].1).collect()));
            }
            for (i, (title, s)) in starts.iter().enumerate() {
                let e = starts.get(i + 1).map(|x| x.1).unwrap_or(count);
                if e > *s {
                    groups.push((slug(title), (*s..e).collect()));
                }
            }
            groups
        }
    };
    if groups.is_empty() {
        return Err(Error::option("nothing to split"));
    }
    groups
        .into_iter()
        .map(|(name, pages)| {
            let picks: Vec<(usize, usize)> = pages.into_iter().map(|p| (0, p)).collect();
            Ok((name, assemble(&mut [doc.clone()], &picks, false)?))
        })
        .collect()
}

fn slug(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if c.is_alphanumeric() {
            out.extend(c.to_lowercase());
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    let out = out
        .trim_end_matches('-')
        .chars()
        .take(60)
        .collect::<String>();
    if out.is_empty() {
        "section".into()
    } else {
        out
    }
}

/// (title, 0-based start page) for each top-level bookmark, in page order.
pub fn top_level_bookmarks(doc: &Document) -> Vec<(String, usize)> {
    let pages: HashMap<ObjectId, usize> = pdf::page_ids(doc)
        .into_iter()
        .enumerate()
        .map(|(i, id)| (id, i))
        .collect();
    let Ok(root) = doc.catalog() else {
        return vec![];
    };
    let Some(outlines) = root
        .get(b"Outlines")
        .ok()
        .and_then(|o| pdf::resolve(doc, o).as_dict().ok())
    else {
        return vec![];
    };
    let mut item = outlines.get(b"First").and_then(Object::as_reference).ok();
    let mut out = vec![];
    let mut guard = 0;
    while let Some(id) = item {
        guard += 1;
        if guard > 10_000 {
            break;
        }
        let Ok(d) = doc.get_dictionary(id) else { break };
        let title = d
            .get(b"Title")
            .ok()
            .and_then(crate::meta::decode_text)
            .unwrap_or_default();
        if let Some(page) = dest_page(doc, d).and_then(|p| pages.get(&p)) {
            out.push((title, *page));
        }
        item = d.get(b"Next").and_then(Object::as_reference).ok();
    }
    out.sort_by_key(|x| x.1);
    out.dedup_by_key(|x| x.1);
    out
}

fn dest_page(doc: &Document, item: &Dictionary) -> Option<ObjectId> {
    let dest = match item.get(b"Dest") {
        Ok(d) => pdf::resolve(doc, d).clone(),
        Err(_) => {
            let action = pdf::resolve(doc, item.get(b"A").ok()?).as_dict().ok()?;
            pdf::resolve(doc, action.get(b"D").ok()?).clone()
        }
    };
    let dest = match dest {
        Object::Name(_) | Object::String(..) => named_dest(doc, &dest)?,
        other => other,
    };
    let arr = match &dest {
        Object::Dictionary(d) => pdf::resolve(doc, d.get(b"D").ok()?)
            .as_array()
            .ok()?
            .clone(),
        Object::Array(a) => a.clone(),
        _ => return None,
    };
    arr.first()?.as_reference().ok()
}

fn named_dest(doc: &Document, name: &Object) -> Option<Object> {
    let key = match name {
        Object::Name(n) => n.clone(),
        Object::String(s, _) => s.clone(),
        _ => return None,
    };
    let cat = doc.catalog().ok()?;
    if let Ok(dests) = cat.get(b"Dests")
        && let Ok(d) = pdf::resolve(doc, dests).as_dict()
        && let Ok(v) = d.get(&key)
    {
        return Some(pdf::resolve(doc, v).clone());
    }
    let names = pdf::resolve(doc, cat.get(b"Names").ok()?).as_dict().ok()?;
    let tree = pdf::resolve(doc, names.get(b"Dests").ok()?)
        .as_dict()
        .ok()?;
    find_in_name_tree(doc, tree, &key, 0)
}

fn find_in_name_tree(
    doc: &Document,
    node: &Dictionary,
    key: &[u8],
    depth: usize,
) -> Option<Object> {
    if depth > 32 {
        return None;
    }
    if let Ok(Object::Array(names)) = node.get(b"Names").map(|o| pdf::resolve(doc, o)) {
        for pair in names.chunks(2) {
            if let [Object::String(k, _), v] = pair
                && k.as_slice() == key
            {
                return Some(pdf::resolve(doc, v).clone());
            }
        }
    }
    if let Ok(Object::Array(kids)) = node.get(b"Kids").map(|o| pdf::resolve(doc, o)) {
        for kid in kids {
            if let Ok(d) = pdf::resolve(doc, kid).as_dict()
                && let Some(v) = find_in_name_tree(doc, d, key, depth + 1)
            {
                return Some(v);
            }
        }
    }
    None
}

/// Rotate the selected pages by a multiple of 90 degrees, clockwise.
pub fn rotate(
    input: &[u8],
    angle: i64,
    spec: Option<&str>,
    password: Option<&str>,
) -> Result<Vec<u8>> {
    if angle % 90 != 0 {
        return Err(Error::option("rotation must be a multiple of 90"));
    }
    let mut doc = pdf::load(input, password)?;
    let ids = pdf::page_ids(&doc);
    for p in pdf::parse_ranges(spec.unwrap_or(""), ids.len())? {
        let id = ids[p];
        let r = pdf::page_rotation(&doc, id);
        pdf::materialize(&mut doc, id);
        doc.get_dictionary_mut(id)?
            .set("Rotate", ((r + angle) % 360 + 360) % 360);
    }
    pdf::save(&mut doc)
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct BlankOptions {
    /// Insert after these 1-based pages; 0 means before the first page.
    pub after: Vec<usize>,
    pub count: Option<usize>,
    /// "same" (neighbor's size), "a4" or "letter".
    pub size: Option<String>,
    pub password: Option<String>,
}

pub fn insert_blank(input: &[u8], o: &BlankOptions) -> Result<Vec<u8>> {
    let mut doc = pdf::load(input, o.password.as_deref())?;
    let ids = pdf::page_ids(&doc);
    let count = o.count.unwrap_or(1).clamp(1, 1000);
    if o.after.iter().any(|a| *a > ids.len()) {
        return Err(Error::option(format!(
            "insert position beyond the last page ({})",
            ids.len()
        )));
    }
    let after = if o.after.is_empty() {
        vec![ids.len()]
    } else {
        o.after.clone()
    };
    let mut order: Vec<Object> = vec![];
    let make_blank = |doc: &mut Document, near: ObjectId| -> Result<ObjectId> {
        let b = pdf::page_box(doc, near);
        let (w, h) = match o.size.as_deref().unwrap_or("same") {
            "a4" => (595.28, 841.89),
            "letter" => (612.0, 792.0),
            "same" => (b[2] - b[0], b[3] - b[1]),
            other => return Err(Error::option(format!("unknown size '{other}'"))),
        };
        let content = doc.add_object(Stream::new(Dictionary::new(), vec![]));
        Ok(doc.add_object(dictionary! {
            "Type" => "Page", "MediaBox" => vec![0.into(), 0.into(), w.into(), h.into()],
            "Contents" => content, "Resources" => Dictionary::new(),
        }))
    };
    for i in 0..=ids.len() {
        if i > 0 {
            order.push(ids[i - 1].into());
        }
        for _ in 0..after.iter().filter(|a| **a == i).count() {
            let near = ids[i.saturating_sub(1).min(ids.len() - 1)];
            for _ in 0..count {
                order.push(make_blank(&mut doc, near)?.into());
            }
        }
    }
    rebuild_page_tree(&mut doc, order)?;
    pdf::save(&mut doc)
}

/// Replace the page tree with a flat one holding `kids`, keeping the catalog.
pub fn rebuild_page_tree(doc: &mut Document, kids: Vec<Object>) -> Result<()> {
    for k in &kids {
        if let Object::Reference(id) = k {
            pdf::materialize(doc, *id);
        }
    }
    let pages_id = doc.new_object_id();
    for k in &kids {
        if let Object::Reference(id) = k {
            doc.get_dictionary_mut(*id)?.set("Parent", pages_id);
        }
    }
    let count = kids.len() as i64;
    doc.objects.insert(
        pages_id,
        dictionary! { "Type" => "Pages", "Kids" => kids, "Count" => count }.into(),
    );
    let root = doc.trailer.get(b"Root")?.as_reference()?;
    doc.get_dictionary_mut(root)?.set("Pages", pages_id);
    Ok(())
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct CropOptions {
    /// Margins to cut, in points: top, right, bottom, left.
    pub margins: [f32; 4],
    pub pages: Option<String>,
    pub password: Option<String>,
}

pub fn crop(input: &[u8], o: &CropOptions) -> Result<Vec<u8>> {
    let mut doc = pdf::load(input, o.password.as_deref())?;
    let ids = pdf::page_ids(&doc);
    let [t, r, b, l] = o.margins;
    if o.margins.iter().any(|m| *m < 0.0) {
        return Err(Error::option("margins cannot be negative"));
    }
    for p in pdf::parse_ranges(o.pages.as_deref().unwrap_or(""), ids.len())? {
        let id = ids[p];
        let bx = pdf::page_box(&doc, id);
        let nb = [bx[0] + l, bx[1] + b, bx[2] - r, bx[3] - t];
        if nb[2] - nb[0] < 1.0 || nb[3] - nb[1] < 1.0 {
            return Err(Error::option(format!(
                "margins leave nothing of page {}",
                p + 1
            )));
        }
        pdf::materialize(&mut doc, id);
        let arr: Vec<Object> = nb.iter().map(|v| (*v).into()).collect();
        let d = doc.get_dictionary_mut(id)?;
        d.set("CropBox", arr.clone());
        d.set("TrimBox", arr);
    }
    pdf::save(&mut doc)
}

/// Turn a page into a Form XObject in the same document (for N-up and stamps).
pub fn page_as_xobject(doc: &mut Document, page: ObjectId) -> Result<(ObjectId, [f32; 4])> {
    let bx = pdf::page_box(doc, page);
    let content = doc.get_page_content(page);
    let res =
        pdf::inherited(doc, page, b"Resources").unwrap_or(Object::Dictionary(Dictionary::new()));
    let mut s = Stream::new(
        dictionary! {
            "Type" => "XObject", "Subtype" => "Form",
            "BBox" => bx.iter().map(|v| Object::from(*v)).collect::<Vec<_>>(),
            "Resources" => res,
        },
        content,
    );
    let _ = s.compress();
    Ok((doc.add_object(s), bx))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct NupOptions {
    /// 2, 4, 6, 8, 9 or 16 pages per sheet.
    pub per_sheet: usize,
    /// "a4", "letter" or "same".
    pub sheet: String,
    pub margin: f32,
    pub gap: f32,
    pub border: bool,
    pub password: Option<String>,
}

impl Default for NupOptions {
    fn default() -> Self {
        NupOptions {
            per_sheet: 2,
            sheet: "a4".into(),
            margin: 18.0,
            gap: 12.0,
            border: false,
            password: None,
        }
    }
}

pub fn nup(input: &[u8], o: &NupOptions) -> Result<Vec<u8>> {
    let (cols, rows) = match o.per_sheet {
        2 => (2, 1),
        4 => (2, 2),
        6 => (3, 2),
        8 => (4, 2),
        9 => (3, 3),
        16 => (4, 4),
        n => {
            return Err(Error::option(format!(
                "{n} pages per sheet is not supported (2, 4, 6, 8, 9, 16)"
            )));
        }
    };
    let mut doc = pdf::load(input, o.password.as_deref())?;
    let ids = pdf::page_ids(&doc);
    let first = pdf::page_box(&doc, ids[0]);
    let (mut sw, mut sh) = match o.sheet.as_str() {
        "a4" => (595.28, 841.89),
        "letter" => (612.0, 792.0),
        "same" => (first[2] - first[0], first[3] - first[1]),
        other => return Err(Error::option(format!("unknown sheet '{other}'"))),
    };
    // Wide grids go on landscape sheets.
    if (cols > rows) != (sw > sh) {
        std::mem::swap(&mut sw, &mut sh);
    }
    let cell_w = (sw - 2.0 * o.margin - (cols - 1) as f32 * o.gap) / cols as f32;
    let cell_h = (sh - 2.0 * o.margin - (rows - 1) as f32 * o.gap) / rows as f32;
    if cell_w <= 1.0 || cell_h <= 1.0 {
        return Err(Error::option("margins and gaps leave no room for pages"));
    }
    let mut forms = vec![];
    for id in &ids {
        let rot = pdf::page_rotation(&doc, *id);
        forms.push((page_as_xobject(&mut doc, *id)?, rot));
    }
    let mut sheets: Vec<Object> = vec![];
    for chunk in forms.chunks(cols * rows) {
        let mut ops = String::new();
        let mut xo = Dictionary::new();
        for (i, ((form, bx), rot)) in chunk.iter().enumerate() {
            let (c, r) = (i % cols, i / cols);
            let (pw, ph) = (bx[2] - bx[0], bx[3] - bx[1]);
            let (vw, vh) = if rot % 180 == 90 { (ph, pw) } else { (pw, ph) };
            let scale = (cell_w / vw).min(cell_h / vh);
            let x0 = o.margin + c as f32 * (cell_w + o.gap) + (cell_w - vw * scale) / 2.0;
            let y0 = sh - o.margin - (r + 1) as f32 * cell_h - r as f32 * o.gap
                + (cell_h - vh * scale) / 2.0;
            // Map the form's BBox to the cell, honoring /Rotate (clockwise).
            let (a, b, cc, d, e, f) = match rot {
                90 => (
                    0.0,
                    -scale,
                    scale,
                    0.0,
                    x0 - bx[1] * scale,
                    y0 + bx[2] * scale,
                ),
                180 => (
                    -scale,
                    0.0,
                    0.0,
                    -scale,
                    x0 + bx[2] * scale,
                    y0 + bx[3] * scale,
                ),
                270 => (
                    0.0,
                    scale,
                    -scale,
                    0.0,
                    x0 + bx[3] * scale,
                    y0 - bx[0] * scale,
                ),
                _ => (
                    scale,
                    0.0,
                    0.0,
                    scale,
                    x0 - bx[0] * scale,
                    y0 - bx[1] * scale,
                ),
            };
            let name = format!("P{i}");
            xo.set(name.clone(), *form);
            ops.push_str(&format!(
                "q {} {} {} {} {} {} cm /{name} Do Q\n",
                n(a),
                n(b),
                n(cc),
                n(d),
                n(e),
                n(f)
            ));
            if o.border {
                ops.push_str(&format!(
                    "q 0.5 w 0.6 G {} {} {} {} re S Q\n",
                    n(x0),
                    n(y0),
                    n(vw * scale),
                    n(vh * scale)
                ));
            }
        }
        let content = doc.add_object(Stream::new(Dictionary::new(), ops.into_bytes()));
        let page = doc.add_object(dictionary! {
            "Type" => "Page", "MediaBox" => vec![0.into(), 0.into(), sw.into(), sh.into()],
            "Contents" => content, "Resources" => dictionary! { "XObject" => xo },
        });
        sheets.push(page.into());
    }
    rebuild_page_tree(&mut doc, sheets)?;
    if let Ok(cat) = doc.catalog_mut() {
        for k in [
            &b"Outlines"[..],
            b"AcroForm",
            b"PageLabels",
            b"StructTreeRoot",
        ] {
            cat.remove(k);
        }
    }
    pdf::save(&mut doc)
}

/// Pages of one document grouped by their 1-based number, used by tests.
pub fn page_count(input: &[u8]) -> Result<usize> {
    Ok(pdf::load(input, None)?.get_pages().len())
}

/// Text of each page title line, for tests that check page order.
#[cfg(test)]
pub fn page_titles(input: &[u8]) -> Vec<String> {
    let doc = pdf::load(input, None).unwrap();
    let pages = doc.get_pages();
    pages
        .keys()
        .map(|n| {
            let text = doc.extract_text(&[*n]).unwrap_or_default();
            text.split("The quick")
                .next()
                .unwrap_or("")
                .trim()
                .to_string()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures;
    use crate::testutil::qpdf_check;

    #[test]
    fn merge_two() {
        let out = merge(&[fixtures::text_pdf(2), fixtures::text_pdf(3)], None).unwrap();
        assert_eq!(
            page_titles(&out),
            ["Page 1", "Page 2", "Page 1", "Page 2", "Page 3"]
        );
        let doc = pdf::load(&out, None).unwrap();
        assert_eq!(
            top_level_bookmarks(&doc)
                .iter()
                .map(|b| b.1)
                .collect::<Vec<_>>(),
            vec![0, 2]
        );
        qpdf_check(&out);
    }

    #[test]
    fn merge_needs_two() {
        assert!(merge(&[fixtures::text_pdf(1)], None).is_err());
    }

    #[test]
    fn select_reorders_and_duplicates() {
        let out = select(&fixtures::text_pdf(4), "4,1-2,2", None).unwrap();
        assert_eq!(page_titles(&out), ["Page 4", "Page 1", "Page 2", "Page 2"]);
        qpdf_check(&out);
    }

    #[test]
    fn delete_pages() {
        let out = delete(&fixtures::text_pdf(5), "2,4", None).unwrap();
        assert_eq!(page_titles(&out), ["Page 1", "Page 3", "Page 5"]);
        assert!(delete(&fixtures::text_pdf(2), "1-2", None).is_err());
        qpdf_check(&out);
    }

    #[test]
    fn split_modes() {
        let input = fixtures::text_pdf(5);
        let parts = split(&input, &SplitMode::Every { every: 2 }, None).unwrap();
        assert_eq!(
            parts
                .iter()
                .map(|p| page_count(&p.1).unwrap())
                .collect::<Vec<_>>(),
            vec![2, 2, 1]
        );
        assert_eq!(parts[0].0, "1-2");
        let parts = split(
            &input,
            &SplitMode::Ranges {
                ranges: "1-3, 5".into(),
            },
            None,
        )
        .unwrap();
        assert_eq!(parts.len(), 2);
        assert_eq!(page_titles(&parts[1].1), ["Page 5"]);
        let parts = split(&fixtures::outlined_pdf(), &SplitMode::Bookmarks, None).unwrap();
        assert_eq!(
            parts.iter().map(|p| p.0.as_str()).collect::<Vec<_>>(),
            ["intro", "methods", "results"]
        );
        assert_eq!(
            parts
                .iter()
                .map(|p| page_count(&p.1).unwrap())
                .collect::<Vec<_>>(),
            vec![2, 1, 2]
        );
        for p in parts {
            qpdf_check(&p.1);
        }
        assert!(split(&input, &SplitMode::Bookmarks, None).is_err());
    }

    #[test]
    fn rotate_pages() {
        let out = rotate(&fixtures::text_pdf(3), 90, Some("2"), None).unwrap();
        let doc = pdf::load(&out, None).unwrap();
        let rots: Vec<i64> = pdf::page_ids(&doc)
            .iter()
            .map(|id| pdf::page_rotation(&doc, *id))
            .collect();
        assert_eq!(rots, vec![0, 90, 0]);
        let out = rotate(&out, -180, None, None).unwrap();
        let doc = pdf::load(&out, None).unwrap();
        let rots: Vec<i64> = pdf::page_ids(&doc)
            .iter()
            .map(|id| pdf::page_rotation(&doc, *id))
            .collect();
        assert_eq!(rots, vec![180, 270, 180]);
        assert!(rotate(&out, 45, None, None).is_err());
        qpdf_check(&out);
    }

    #[test]
    fn blank_pages() {
        let o = BlankOptions {
            after: vec![0, 2],
            count: Some(1),
            ..Default::default()
        };
        let out = insert_blank(&fixtures::text_pdf(2), &o).unwrap();
        assert_eq!(page_titles(&out), ["", "Page 1", "Page 2", ""]);
        qpdf_check(&out);
    }

    #[test]
    fn crop_sets_box() {
        let o = CropOptions {
            margins: [10.0, 20.0, 30.0, 40.0],
            ..Default::default()
        };
        let out = crop(&fixtures::text_pdf(1), &o).unwrap();
        let doc = pdf::load(&out, None).unwrap();
        assert_eq!(
            pdf::page_box(&doc, pdf::page_ids(&doc)[0]),
            [40.0, 30.0, 592.0, 782.0]
        );
        let bad = CropOptions {
            margins: [400.0, 0.0, 400.0, 0.0],
            ..Default::default()
        };
        assert!(crop(&fixtures::text_pdf(1), &bad).is_err());
        qpdf_check(&out);
    }

    #[test]
    fn nup_sheets() {
        let out = nup(
            &fixtures::text_pdf(5),
            &NupOptions {
                per_sheet: 4,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(page_count(&out).unwrap(), 2);
        let out2 = nup(&fixtures::text_pdf(3), &NupOptions::default()).unwrap();
        let doc = pdf::load(&out2, None).unwrap();
        let b = pdf::page_box(&doc, pdf::page_ids(&doc)[0]);
        assert!(b[2] > b[3], "2-up is landscape");
        assert!(
            nup(
                &fixtures::text_pdf(3),
                &NupOptions {
                    per_sheet: 5,
                    ..Default::default()
                }
            )
            .is_err()
        );
        qpdf_check(&out);
    }
}
