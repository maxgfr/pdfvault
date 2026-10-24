// The editor's document: what the site's editor builds and `pdfvault edit
// --ops` reads. One JSON shape drives the engine calls (core `edit-plan`,
// render `edit`, core `arrange`); see crates/core/src/edit.rs for the rules.

import { EngineError } from "./errors.ts";

/** A font file among the attachments; `name` picks a face in a collection (.ttc). */
export interface FontRef {
  file: number;
  name?: string;
}

/** Visual page space: points, origin at the bottom-left of the page as displayed. */
export type EditItem =
  | {
      type: "text";
      page: number;
      x: number;
      y: number;
      text: string;
      size?: number;
      color?: string;
      font?: "sans" | "serif" | "mono";
      bold?: boolean;
      italic?: boolean;
      fontFile?: FontRef;
    }
  | {
      /** Existing text rewritten: glyphs whose centre is in the box go, `text` is written in their place. */
      type: "replace";
      page: number;
      x: number;
      y: number;
      w: number;
      h: number;
      text: string;
      /** Instead of a box: the first line of the page containing this is rewritten, with it replaced by `text`. */
      find?: string;
      start?: number;
      baseline?: number;
      size?: number;
      color?: string;
      /** "auto": the original font when it has every character, else the closest bundled one. */
      font?: "auto" | "sans" | "serif" | "mono";
      bold?: boolean;
      italic?: boolean;
      fontFile?: FontRef;
      hscale?: number;
      /** A paragraph from `text-runs`: the glyphs it takes out, and per character the glyph whose style it keeps. */
      glyphs?: number[];
      sources?: number[];
      /** Reflow between `left` and `right`, `leading` apart. */
      left?: number;
      right?: number;
      leading?: number;
      align?: "left" | "right" | "center" | "justify";
    }
  | {
      /** One of the page's own images, drawings or shapes, as `text-runs` lists them. */
      type: "object";
      page: number;
      kind: "image" | "form" | "path";
      index: number;
      delete?: boolean;
      dx?: number;
      dy?: number;
      sx?: number;
      sy?: number;
    }
  | { type: "rect"; page: number; x: number; y: number; w: number; h: number; fill?: string; stroke?: string; lineWidth?: number; opacity?: number }
  | { type: "ink"; page: number; paths: number[][]; color?: string; lineWidth?: number; opacity?: number }
  | { type: "image"; page: number; x: number; y: number; w: number; h: number; image: number; opacity?: number }
  | { type: "note"; page: number; x: number; y: number; text: string; color?: string }
  | { type: "highlight"; page: number; x: number; y: number; w: number; h: number; color?: string }
  | { type: "redact"; page: number; x: number; y: number; w: number; h: number };

export interface EditOps {
  /** Pages appended after the input's, so items can sit on them. */
  blanks?: { size?: "same" | "a4" | "letter"; like?: number }[];
  fields?: Record<string, string | boolean>;
  flattenFields?: boolean;
  items?: EditItem[];
  /** "remove" (default): what is under a redaction box leaves the content, the rest stays
   *  selectable. "flatten": redacted pages become images. */
  redaction?: "remove" | "flatten";
  /** Fonts (attachments) tried for characters a text's own font lacks. */
  fallbackFonts?: FontRef[];
  /** Complete copies of the document's fonts, by PostScript name: a changed line whose
   *  embedded subset lacks a character is redrawn in the copy. `file` is an attachment. */
  substitutes?: { name: string; file: number }[];
  /** The output pages in order (0-based, blanks included); omitted = unchanged. */
  order?: { page: number; rotate?: number }[];
  /** Resolution pages turned into images are rendered at. */
  redactDpi?: number;
}

/** A form widget as `form-fields` lists it. */
export interface FormField {
  name: string;
  kind: "text" | "checkbox" | "radio" | "choice";
  value: string;
  options: string[];
  onState: string | null;
  page: number;
  rect: [number, number, number, number];
  multiline: boolean;
  readOnly: boolean;
  maxLen: number | null;
}

/** A line of existing text as `text-runs` lists it (visual page space). */
export interface TextRun {
  page: number;
  x: number;
  y: number;
  w: number;
  h: number;
  start: number;
  baseline: number;
  size: number;
  text: string;
  family: "sans" | "serif" | "mono";
  bold: boolean;
  italic: boolean;
  color: string;
  fontName: string;
  fontFamily: string;
  hscale: number;
  editable: boolean;
}

/** A paragraph as `text-runs` lists it (visual page space). */
export interface Paragraph {
  page: number;
  x: number;
  y: number;
  w: number;
  h: number;
  start: number;
  left: number;
  right: number;
  /** Where rewritten text wraps (the column's edge; a lone line's end is no limit). */
  wrap: number;
  baseline: number;
  leading: number;
  align: "left" | "right" | "center" | "justify";
  lines: number;
  text: string;
  sources: number[];
  glyphs: number[];
  size: number;
  family: "sans" | "serif" | "mono";
  bold: boolean;
  italic: boolean;
  color: string;
  fontName: string;
  fontFamily: string;
  hscale: number;
  editable: boolean;
}

/** An image, drawing or shape of a page, as `text-runs` lists it. */
export interface PageObject {
  page: number;
  kind: "image" | "form" | "path";
  index: number;
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface EditReport {
  pages: number;
  blanks: number;
  fields: number;
  drawn: number;
  annotations: number;
  replaced: number;
  /** Lines kept entirely in their font; partly (missing characters in another font); redrawn in an installed copy. */
  sameFont: number;
  mixed: number;
  substituted: number;
  covered: number;
  objects: number;
  /** Rewritten paragraphs now reaching lower than before (nothing below moves). */
  overflow: number;
  /** Pages with redactions (1-based), and those that had to become images. */
  redactedPages: number[];
  flattenedPages: number[];
}

/** First baseline of a text item, in font sizes below its top (matches the Rust side). */
export const TEXT_ASCENT = 0.8;
export const TEXT_LINE = 1.2;
export const NOTE_SIZE = 20;

export function parseEdits(json: string): EditOps {
  let v: unknown;
  try {
    v = JSON.parse(json);
  } catch (e) {
    throw new EngineError("invalid_option", `the edits are not valid JSON: ${e instanceof Error ? e.message : String(e)}`);
  }
  if (!v || typeof v !== "object" || Array.isArray(v)) throw new EngineError("invalid_option", "the edits must be a JSON object");
  return v as EditOps;
}

/** Redaction boxes grouped by page, as render `redact-check` takes them. */
export function redactionBoxes(ops: EditOps): { page: number; rects: [number, number, number, number][] }[] {
  const by = new Map<number, [number, number, number, number][]>();
  for (const it of ops.items ?? []) {
    if (it.type !== "redact") continue;
    by.set(it.page, [...(by.get(it.page) ?? []), [it.x, it.y, it.w, it.h]]);
  }
  return [...by].sort((a, b) => a[0] - b[0]).map(([page, rects]) => ({ page, rects }));
}

/** The characters a `missing_glyphs` error names. */
export function missingCharacters(e: unknown): string | null {
  if (!(e instanceof EngineError) || e.code !== "missing_glyphs") return null;
  const at = e.message.indexOf(": ");
  return at < 0 ? "" : e.message.slice(at + 2);
}
