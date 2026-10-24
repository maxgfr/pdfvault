// The editor's document: pages in output order, items drawn on source pages,
// form values. Everything here is plain data; `toOps` turns it into the JSON
// the `edit` tool reads, so what you see is exactly what the engine writes.

import type { EditItem, EditOps, FontRef } from "@pdfvault/engine";
import { useCallback, useState } from "react";
import type { Family } from "./fonts.ts";

export type ToolName = "select" | "edittext" | "text" | "whiteout" | "rect" | "highlight" | "ink" | "note" | "image" | "redact";

/** How the text being rewritten looked, for the preview (never sent to the engine). */
export interface Look {
  family: Family;
  bold: boolean;
  italic: boolean;
  /** The page colour behind the original text. */
  bg: string;
  fontName: string;
  /** The family an installed copy of the font has, for previews. */
  familyName: string;
  /** The text as it was, to tell an untouched edit from a real one. */
  original: string;
  size: number;
  color: string;
  /** Per character of `original`, the glyph it came from. */
  sources0?: number[];
}

/** `run`: the paragraph an edit came from; `box`: an object's box before it moved (both editor-only). */
export type Item = EditItem & { id: string; look?: Look; run?: number; box?: [number, number, number, number] };

/** Sources for `text` edited from `original`: the characters kept at both
 *  ends keep their glyphs; what was typed in between takes the style before it. */
export function sourcesFor(text: string, original: string, sources0: number[]): number[] {
  const a = [...original];
  const b = [...text];
  let pre = 0;
  while (pre < a.length && pre < b.length && a[pre] === b[pre]) pre++;
  let suf = 0;
  while (suf < a.length - pre && suf < b.length - pre && a[a.length - 1 - suf] === b[b.length - 1 - suf]) suf++;
  return b.map((_, i) => (i < pre ? (sources0[i] ?? -1) : i >= b.length - suf ? (sources0[a.length - (b.length - i)] ?? -1) : -1));
}

/** An object edit that changes nothing (a click without a drag). */
export const idleObject = (it: Item) =>
  it.type === "object" && !it.delete && (it.dx ?? 0) === 0 && (it.dy ?? 0) === 0 && (it.sx ?? 1) === 1 && (it.sy ?? 1) === 1;

/** Edits the engine must apply for the page to look right: rewritten text and the page's own objects. */
export const changesPage = (it: Item) => it.type === "replace" || (it.type === "object" && !idleObject(it));

const strip = ({ id: _id, look: _look, run: _run, box: _box, ...it }: Item) => it as EditItem;

/** What a page's preview depends on. */
export function pageEdits(doc: Doc, src: number): EditItem[] {
  return doc.items.filter((it) => it.page === src && changesPage(it)).map(strip);
}

/** One output page: which source page it shows and how much it is turned. */
export interface PageRef {
  id: string;
  src: number;
  rotate: number;
}

export interface Blank {
  size: "same" | "a4" | "letter";
  like?: number;
}

/** A source page: its visual size in points and its rendered preview. */
export interface SrcPage {
  width: number;
  height: number;
  url?: string;
}

/** An image the edits can place (a signature, a logo); its index is its input number. */
export interface StampImage {
  name: string;
  bytes: Uint8Array;
  url: string;
  width: number;
  height: number;
}

export interface Doc {
  pages: PageRef[];
  blanks: Blank[];
  items: Item[];
  fields: Record<string, string | boolean>;
  flattenFields: boolean;
  redaction: "remove" | "flatten";
}

export const newId = () => crypto.randomUUID();

export function initialDoc(pageCount: number): Doc {
  return {
    pages: Array.from({ length: pageCount }, (_, i) => ({ id: newId(), src: i, rotate: 0 })),
    blanks: [],
    items: [],
    fields: {},
    flattenFields: false,
    redaction: "remove",
  };
}

const BLANK_SIZES = { a4: { width: 595.28, height: 841.89 }, letter: { width: 612, height: 792 } };

/** Source pages followed by the blank pages, as the engine numbers them. */
export function sources(pages: SrcPage[], doc: Doc): SrcPage[] {
  return [
    ...pages,
    ...doc.blanks.map((b) => {
      if (b.size !== "same") return BLANK_SIZES[b.size];
      const like = pages[b.like ?? pages.length - 1] ?? pages[0]!;
      return { width: like.width, height: like.height };
    }),
  ];
}

/** The family, weight and slant a text or rewritten line is drawn with. */
export function styleOf(it: Item): { family: Family; bold: boolean; italic: boolean } {
  if (it.type === "replace") {
    const look = it.look ?? { family: "sans", bold: false, italic: false };
    return { family: it.font && it.font !== "auto" ? it.font : look.family, bold: it.bold ?? look.bold, italic: it.italic ?? look.italic };
  }
  if (it.type === "text") return { family: it.font ?? "sans", bold: Boolean(it.bold), italic: Boolean(it.italic) };
  return { family: "sans", bold: false, italic: false };
}

/** Box of an item in visual page space: [x0, y0, x1, y1], y up. */
export function bounds(it: EditItem): [number, number, number, number] {
  switch (it.type) {
    case "object": {
      const b: [number, number, number, number] = (it as Item).box ?? [0, 0, 0, 0];
      if (it.delete) return b;
      const [sx, sy, dx, dy] = [it.sx ?? 1, it.sy ?? 1, it.dx ?? 0, it.dy ?? 0];
      return [b[0] + dx, b[1] + dy, b[0] + dx + (b[2] - b[0]) * sx, b[1] + dy + (b[3] - b[1]) * sy];
    }
    case "replace": {
      if (it.right !== undefined) {
        // A paragraph: its old box, and the lines the new text may need.
        const size = it.size ?? (it as Item).look?.size ?? it.h;
        const left = it.left ?? it.x;
        const perLine = Math.max(1, Math.floor(Math.max(10, it.right - left) / (size * 0.5)));
        const lines = Math.max(1, Math.ceil(it.text.length / perLine) + (it.text.match(/\n/g)?.length ?? 0));
        const base = it.baseline ?? it.y;
        const bottom = base - (it.leading ?? size * 1.2) * (lines - 1) - size * 0.3;
        return [Math.min(it.x, left, it.start ?? left), Math.min(it.y, bottom), Math.max(it.x + it.w, it.right), Math.max(it.y + it.h, base + size * 0.8)];
      }
      // The text taken out, and the new text, which may run longer.
      const size = it.size ?? it.h;
      const lines = it.text.split("\n");
      const start = it.start ?? it.x;
      const width = Math.max(...lines.map((l) => l.length * size * 0.55));
      const base = it.baseline ?? it.y;
      const bottom = base - size * (0.25 + 1.2 * (lines.length - 1));
      return [Math.min(it.x, start), Math.min(it.y, bottom), Math.max(it.x + it.w, start + width), Math.max(it.y + it.h, base + size * 0.8)];
    }
    case "text": {
      const size = it.size ?? 14;
      const lines = it.text.split("\n");
      const width = Math.max(size * 2, ...lines.map((l) => l.length * size * 0.55));
      const height = size * (0.8 + 1.2 * (lines.length - 1) + 0.3);
      return [it.x, it.y - height, it.x + width, it.y];
    }
    case "note":
      return [it.x, it.y - 20, it.x + 20, it.y];
    case "ink": {
      const xs = it.paths.flatMap((p) => p.filter((_, i) => i % 2 === 0));
      const ys = it.paths.flatMap((p) => p.filter((_, i) => i % 2 === 1));
      const pad = (it.lineWidth ?? 2) / 2;
      return [Math.min(...xs) - pad, Math.min(...ys) - pad, Math.max(...xs) + pad, Math.max(...ys) + pad];
    }
    default:
      return [Math.min(it.x, it.x + it.w), Math.min(it.y, it.y + it.h), Math.max(it.x, it.x + it.w), Math.max(it.y, it.y + it.h)];
  }
}

export function moveItem(it: Item, dx: number, dy: number): Item {
  if (it.type === "ink") return { ...it, paths: it.paths.map((p) => p.map((v, i) => v + (i % 2 === 0 ? dx : dy))) };
  if (it.type === "object") return { ...it, dx: (it.dx ?? 0) + dx, dy: (it.dy ?? 0) + dy };
  // The old text stays where it was; only its replacement moves.
  if (it.type === "replace")
    return {
      ...it,
      start: (it.start ?? it.x) + dx,
      baseline: (it.baseline ?? it.y) + dy,
      ...(it.right !== undefined ? { left: (it.left ?? it.x) + dx, right: it.right + dx } : {}),
    };
  return { ...it, x: it.x + dx, y: it.y + dy };
}

/** The `edit` tool's options for this document. */
export function toOps(doc: Doc, sourceCount: number, fallbackFonts: FontRef[] = []): EditOps {
  const ops: EditOps = {};
  if (doc.blanks.length) ops.blanks = doc.blanks;
  const items = doc.items.filter((it) => !idleObject(it));
  if (items.length) ops.items = items.map(strip);
  if (Object.keys(doc.fields).length) ops.fields = doc.fields;
  if (doc.flattenFields) ops.flattenFields = true;
  if (doc.redaction === "flatten" && doc.items.some((i) => i.type === "redact")) ops.redaction = "flatten";
  if (fallbackFonts.length) ops.fallbackFonts = fallbackFonts;
  const unchanged = doc.pages.length === sourceCount && doc.pages.every((p, i) => p.src === i && p.rotate % 360 === 0);
  if (!unchanged) ops.order = doc.pages.map((p) => (p.rotate % 360 ? { page: p.src, rotate: p.rotate % 360 } : { page: p.src }));
  return ops;
}

interface History {
  past: Doc[];
  present: Doc;
  future: Doc[];
}

const LIMIT = 100;

/** Undo and redo over whole-document snapshots. `live` changes skip the history (drags, typing) after a `checkpoint`. */
export function useHistory(initial: () => Doc) {
  const [h, setH] = useState<History>(() => ({ past: [], present: initial(), future: [] }));
  const commit = useCallback((fn: (d: Doc) => Doc) => setH((s) => ({ past: [...s.past.slice(-LIMIT), s.present], present: fn(s.present), future: [] })), []);
  const live = useCallback((fn: (d: Doc) => Doc) => setH((s) => ({ ...s, present: fn(s.present) })), []);
  const checkpoint = useCallback(() => setH((s) => ({ past: [...s.past.slice(-LIMIT), s.present], present: s.present, future: [] })), []);
  const undo = useCallback(
    () => setH((s) => (s.past.length ? { past: s.past.slice(0, -1), present: s.past.at(-1)!, future: [s.present, ...s.future] } : s)),
    [],
  );
  const redo = useCallback(
    () => setH((s) => (s.future.length ? { past: [...s.past, s.present], present: s.future[0]!, future: s.future.slice(1) } : s)),
    [],
  );
  const reset = useCallback((d: Doc) => setH({ past: [], present: d, future: [] }), []);
  return { doc: h.present, commit, live, checkpoint, undo, redo, reset, canUndo: h.past.length > 0, canRedo: h.future.length > 0 };
}
