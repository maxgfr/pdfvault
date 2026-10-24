// The editor's document: pages in output order, items drawn on source pages,
// form values. Everything here is plain data; `toOps` turns it into the JSON
// the `edit` tool reads, so what you see is exactly what the engine writes.

import type { EditItem, EditOps } from "@pdfvault/engine";
import { useCallback, useState } from "react";

export type ToolName = "select" | "text" | "whiteout" | "rect" | "highlight" | "ink" | "note" | "image" | "redact";

export type Item = EditItem & { id: string };

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
}

export const newId = () => crypto.randomUUID();

export function initialDoc(pageCount: number): Doc {
  return {
    pages: Array.from({ length: pageCount }, (_, i) => ({ id: newId(), src: i, rotate: 0 })),
    blanks: [],
    items: [],
    fields: {},
    flattenFields: false,
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

/** Box of an item in visual page space: [x0, y0, x1, y1], y up. */
export function bounds(it: EditItem): [number, number, number, number] {
  switch (it.type) {
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
  return { ...it, x: it.x + dx, y: it.y + dy };
}

/** The `edit` tool's options for this document. */
export function toOps(doc: Doc, sourceCount: number): EditOps {
  const ops: EditOps = {};
  if (doc.blanks.length) ops.blanks = doc.blanks;
  if (doc.items.length) ops.items = doc.items.map(({ id: _id, ...it }) => it as EditItem);
  if (Object.keys(doc.fields).length) ops.fields = doc.fields;
  if (doc.flattenFields) ops.flattenFields = true;
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
