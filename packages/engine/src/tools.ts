// Every tool, declared once. The CLI derives its commands and flags from this
// list and the site derives its forms, so both run identical code paths.

import { parseEdits, redactionBoxes, type EditReport } from "./edit.ts";
import { isFontKey } from "./fonts.ts";
import type { Engine, OcrLine, OutFile, RunResult } from "./engine.ts";
import { EngineError } from "./errors.ts";

export type Category = "compress" | "organize" | "edit" | "security" | "convert" | "image";

/** What a tool consumes. Batch tools run once per input file. */
/** `mixed` takes PDFs and images in any order. */
export type InputKind = "pdf" | "pdfs" | "image" | "images" | "mixed" | "pdf+image" | "pdf+images" | "pdf+pdf";

interface FieldBase {
  name: string;
  label: string;
  help?: string;
  /** Shown only when another field has one of these values. */
  when?: { field: string; is: (string | number | boolean)[] };
  /** Kept out of the site's main form, under "More options". */
  advanced?: boolean;
  /** A shorter CLI spelling, e.g. `--gray` for `--grayscale`. */
  cliAlias?: string;
}

export type TextField = FieldBase & {
  kind: "text" | "pages" | "password" | "color" | "textarea";
  default?: string;
  placeholder?: string;
  required?: boolean;
};

export type Field =
  | (FieldBase & { kind: "select"; choices: { value: string; label: string }[]; default: string })
  | (FieldBase & { kind: "number"; min: number; max: number; step?: number; default?: number; unit?: string })
  | TextField
  | (FieldBase & { kind: "boolean"; default: boolean });

export type Options = Record<string, string | number | boolean | undefined>;

export interface Progress {
  (done: number, total: number, label: string): void;
}

export interface Tool {
  id: string;
  title: string;
  summary: string;
  category: Category;
  input: InputKind;
  fields: Field[];
  /** CLI spellings that map to this tool, e.g. ["img compress"]. */
  aliases?: string[];
  /** Loads the renderer (and maybe OCR): slower first run. */
  heavy?: "render" | "ocr";
  run(engine: Engine, inputs: Uint8Array[], options: Options, progress?: Progress): Promise<RunResult>;
}

// ------------------------------------------------------------------ helpers

const str = (v: unknown): string | undefined => (v === undefined || v === "" ? undefined : String(v));
const num = (v: unknown): number | undefined => (v === undefined || v === "" ? undefined : Number(v));
const bool = (v: unknown): boolean => v === true || v === "true";

function required(o: Options, name: string, label: string): string {
  const v = str(o[name]);
  if (v === undefined) throw new EngineError("invalid_option", `${label} is required`);
  return v;
}

const password: TextField = {
  kind: "password",
  name: "password",
  label: "Password of the input",
  help: "Only needed when the PDF is locked.",
  advanced: true,
};

const pagesField = (label = "Pages", help = "For example 1-3,7,10-. Empty means every page."): TextField => ({
  kind: "pages",
  name: "pages",
  label,
  help,
  placeholder: "all",
});

const pw = (o: Options) => ({ password: str(o.password) });

const isPdf = (b: Uint8Array) => new TextDecoder().decode(b.subarray(0, 1024)).includes("%PDF-");

/** How images are laid out on pages (Images to PDF, Combine). */
const imagePageFields: Field[] = [
  {
    kind: "select",
    name: "pageSize",
    label: "Page size",
    default: "a4",
    choices: [
      { value: "a4", label: "A4" },
      { value: "letter", label: "US Letter" },
      { value: "fit", label: "Fit each image" },
    ],
  },
  {
    kind: "select",
    name: "orientation",
    label: "Orientation",
    default: "auto",
    choices: [
      { value: "auto", label: "Follow each image" },
      { value: "portrait", label: "Portrait" },
      { value: "landscape", label: "Landscape" },
    ],
    when: { field: "pageSize", is: ["a4", "letter"] },
  },
  { kind: "number", name: "margin", label: "Margin", unit: "pt", min: 0, max: 200, default: 24, when: { field: "pageSize", is: ["a4", "letter"] } },
  { kind: "number", name: "dpi", label: "Image resolution", unit: "DPI", min: 10, max: 2400, default: 150, when: { field: "pageSize", is: ["fit"] } },
];

const imagePageOptions = (o: Options) => ({
  pageSize: str(o.pageSize) ?? "a4",
  orientation: str(o.orientation) ?? "auto",
  margin: num(o.margin) ?? 24,
  dpi: num(o.dpi) ?? 150,
});

const positions = [
  { value: "bottom-center", label: "Bottom center" },
  { value: "bottom-right", label: "Bottom right" },
  { value: "bottom-left", label: "Bottom left" },
  { value: "top-center", label: "Top center" },
  { value: "top-right", label: "Top right" },
  { value: "top-left", label: "Top left" },
];

const permissionFields = (defaults: boolean): Field[] => [
  { kind: "boolean", name: "allowPrint", label: "Allow printing", default: true },
  { kind: "boolean", name: "allowCopy", label: "Allow copying text and images", default: defaults },
  { kind: "boolean", name: "allowModify", label: "Allow editing", default: defaults },
  { kind: "boolean", name: "allowAnnotate", label: "Allow comments", default: defaults },
  { kind: "boolean", name: "allowFillForms", label: "Allow filling forms", default: true },
  { kind: "boolean", name: "allowAssemble", label: "Allow inserting, rotating and deleting pages", default: defaults },
];

const permissions = (o: Options) => ({
  print: bool(o.allowPrint),
  printHighQuality: bool(o.allowPrint),
  copy: bool(o.allowCopy),
  modify: bool(o.allowModify),
  annotate: bool(o.allowAnnotate),
  fillForms: bool(o.allowFillForms),
  assemble: bool(o.allowAssemble),
});

/** A page needs OCR when it has no extractable text. */
interface TextPage {
  page: number;
  spans: { text: string; x: number; y: number; w: number; size: number; bold: boolean }[];
  /** The page as GLM-OCR wrote it, passed through to the Markdown verbatim. */
  markdown?: string;
}

/** The OCR engine for the text outputs. GLM-OCR has no word positions, so OCR to a searchable PDF and Scan keep PP-OCRv6. */
const engineField: Field = {
  kind: "select",
  name: "engine",
  label: "Text recognition",
  default: "paddle",
  choices: [
    { value: "paddle", label: "PP-OCRv6 · fast" },
    { value: "glm", label: "GLM-OCR · formulas, harder pages, slow" },
  ],
  help: "GLM-OCR reads a whole page at once and writes formulas as LaTeX; tables come out as plain lines. It is a much larger model (a one-time download of about 750 MB) and takes about a minute per page. It gives no word positions, so OCR (searchable PDF) and Scan always use PP-OCRv6.",
  when: { field: "ocr", is: [true] },
};

/** Pixels GLM-OCR reads a page at: enough for body text, while its vision encoder stays tractable. */
const GLM_PIXELS = 1_000_000;

async function renderPage(engine: Engine, pdf: Uint8Array, page: number, dpi: number, password?: string) {
  const r = await engine.render<{ pages: { width: number; height: number }[] }>("render", [pdf], {
    format: "rgba",
    dpi,
    pages: String(page + 1),
    password,
  });
  const meta = r.report.pages[0];
  const file = r.files[0];
  if (!meta || !file) throw new EngineError("internal", `page ${page + 1} did not render`);
  return { pixels: file.bytes, width: meta.width, height: meta.height };
}

/** OCR lines → spans in points, the same shape the PDF text extractor returns. */
function linesToSpans(lines: OcrLine[], dpi: number): TextPage["spans"] {
  const k = 72 / dpi;
  return lines.map((l) => {
    const xs = l.words.map((w) => w.x);
    const x0 = Math.min(...xs);
    const x1 = Math.max(...l.words.map((w) => w.x + w.w));
    const y0 = Math.min(...l.words.map((w) => w.y));
    const h = Math.max(...l.words.map((w) => w.h));
    return { text: l.text, x: x0 * k, y: y0 * k, w: (x1 - x0) * k, size: Math.round(h * k * 0.8 * 2) / 2, bold: false };
  });
}

async function pagesText(engine: Engine, pdf: Uint8Array, o: Options, progress?: Progress): Promise<{ pages: TextPage[]; ocrPages: number[] }> {
  const text = await engine.render<{ pages: TextPage[] }>("text", [pdf], pw(o));
  const ocrPages: number[] = [];
  if (o.ocr === false || o.ocr === "false") return { pages: text.report.pages, ocrPages };
  const dpi = num(o.dpi) ?? 200;
  const empty = text.report.pages.filter((p) => p.spans.length === 0);
  const glm = str(o.engine) === "glm";
  const sizes = glm && empty.length ? (await engine.core<{ pageSizes: { width: number; height: number }[] }>("info", [pdf], pw(o))).report.pageSizes : [];
  for (const [i, p] of empty.entries()) {
    progress?.(i, empty.length, glm ? `Reading page ${p.page + 1} with GLM-OCR (about a minute per page)` : `Recognizing text on page ${p.page + 1}`);
    // GLM-OCR reads a page at a capped pixel count, whatever its size.
    const size = sizes[p.page];
    const at = size ? Math.min(dpi, Math.floor(72 * Math.sqrt(GLM_PIXELS / (size.width * size.height)))) : dpi;
    const img = await renderPage(engine, pdf, p.page, at, str(o.password));
    if (glm) p.markdown = await engine.glm(img.pixels, img.width, img.height);
    else p.spans = linesToSpans(await engine.ocr(img.pixels, img.width, img.height), at);
    ocrPages.push(p.page + 1);
  }
  return { pages: text.report.pages, ocrPages };
}

/** Recognize the pages without text (all of them with `force`) and add an invisible text layer. */
async function ocrPdf(engine: Engine, pdf: Uint8Array, o: Options, progress?: Progress): Promise<RunResult> {
  const dpi = num(o.dpi) ?? 200;
  const text = await engine.render<{ pages: TextPage[] }>("text", [pdf], { pages: str(o.pages), ...pw(o) });
  const todo = text.report.pages.filter((p) => bool(o.force) || p.spans.length === 0);
  const pages = [];
  for (const [i, p] of todo.entries()) {
    progress?.(i, todo.length, `Recognizing page ${p.page + 1}`);
    const img = await renderPage(engine, pdf, p.page, dpi, str(o.password));
    const lines = await engine.ocr(img.pixels, img.width, img.height);
    pages.push({ page: p.page, width: img.width, height: img.height, words: lines.flatMap((l) => l.words) });
  }
  progress?.(todo.length, todo.length, "Writing the text layer");
  const out = await engine.core("text-layer", [pdf], { pages, ...pw(o) });
  const words = pages.reduce((n, p) => n + p.words.length, 0);
  return { files: out.files, report: { pagesRecognized: pages.map((p) => p.page + 1), words, skipped: text.report.pages.length - todo.length } };
}

async function markdown(engine: Engine, pdf: Uint8Array, o: Options, plain: boolean, progress?: Progress) {
  const { pages, ocrPages } = await pagesText(engine, pdf, o, progress);
  const md = await engine.core("markdown", [], { pages, plain, pageBreaks: bool(o.pageBreaks) });
  return { file: md.files[0] as OutFile, ocrPages, pages: pages.length };
}

// ------------------------------------------------------------------ tools

export const tools: Tool[] = [
  {
    id: "compress",
    title: "Compress PDF",
    summary: "Shrink a PDF: recompress streams, downsample and re-encode images, drop dead weight.",
    category: "compress",
    input: "pdf",
    fields: [
      {
        kind: "select",
        name: "preset",
        label: "Level",
        help: "Lossless keeps every pixel. Extreme turns pages into images: text is no longer selectable.",
        default: "balanced",
        choices: [
          { value: "lossless", label: "Lossless" },
          { value: "balanced", label: "Balanced · 150 DPI" },
          { value: "strong", label: "Strong · 110 DPI" },
          { value: "extreme", label: "Extreme · pages as images" },
        ],
      },
      { kind: "number", name: "dpi", label: "Image resolution", unit: "DPI", min: 10, max: 1200, advanced: true, help: "Overrides the level's resolution." },
      { kind: "number", name: "quality", label: "JPEG quality", min: 1, max: 100, advanced: true, help: "Overrides the level's quality." },
      { kind: "boolean", name: "grayscale", label: "Convert images to grayscale", default: false, advanced: true, cliAlias: "gray" },
      { kind: "boolean", name: "keepMetadata", label: "Keep document metadata", default: false, advanced: true },
      { kind: "boolean", name: "removeAttachments", label: "Remove attached files", default: false, advanced: true },
      password,
    ],
    run(engine, [pdf], o) {
      const options = {
        preset: str(o.preset) ?? "balanced",
        dpi: num(o.dpi),
        quality: num(o.quality),
        grayscale: bool(o.grayscale),
        stripMetadata: !bool(o.keepMetadata),
        removeAttachments: bool(o.removeAttachments),
        ...pw(o),
      };
      return options.preset === "extreme" ? engine.render("extreme", [pdf!], options) : engine.core("compress", [pdf!], options);
    },
  },
  {
    id: "merge",
    title: "Merge PDFs",
    summary: "Combine several PDFs into one, in order, with a bookmark per file.",
    category: "organize",
    input: "pdfs",
    fields: [password],
    run: (engine, inputs, o) => engine.core("merge", inputs, pw(o)),
  },
  {
    id: "combine",
    title: "Combine into one PDF",
    summary: "PDFs and photos together, in the order given: each image becomes a page.",
    category: "organize",
    input: "mixed",
    aliases: ["join"],
    fields: [...imagePageFields, password],
    async run(engine, inputs, o) {
      // Runs of consecutive images become one PDF each, then everything is merged.
      const parts: Uint8Array[] = [];
      let images: Uint8Array[] = [];
      const flush = async () => {
        if (images.length) parts.push((await engine.core("img2pdf", images, imagePageOptions(o))).files[0]!.bytes);
        images = [];
      };
      for (const b of inputs) {
        if (isPdf(b)) {
          await flush();
          parts.push(b);
        } else images.push(b);
      }
      await flush();
      const out = parts.length === 1 ? parts[0]! : (await engine.core("merge", parts, pw(o))).files[0]!.bytes;
      const pages = (await engine.core<{ pages: number }>("info", [out])).report.pages;
      return { files: [{ suffix: "combined", ext: "pdf", bytes: out }], report: { pages, inputs: inputs.length } };
    },
  },
  {
    id: "split",
    title: "Split PDF",
    summary: "Cut a PDF into several files by ranges, every N pages, or by bookmark.",
    category: "organize",
    input: "pdf",
    fields: [
      {
        kind: "select",
        name: "mode",
        label: "Split",
        default: "every",
        choices: [
          { value: "every", label: "Every N pages" },
          { value: "ranges", label: "By page ranges" },
          { value: "bookmarks", label: "At each top-level bookmark" },
        ],
      },
      { kind: "number", name: "every", label: "Pages per file", min: 1, max: 10000, default: 1, when: { field: "mode", is: ["every"] } },
      { kind: "text", name: "ranges", label: "Ranges", placeholder: "1-3,4-6,7-", help: "Each comma-separated range becomes one file.", when: { field: "mode", is: ["ranges"] } },
      password,
    ],
    run(engine, [pdf], o) {
      const mode = str(o.mode) ?? "every";
      const options = mode === "ranges" ? { mode, ranges: required(o, "ranges", "Ranges") } : mode === "every" ? { mode, every: num(o.every) ?? 1 } : { mode };
      return engine.core("split", [pdf!], { ...options, ...pw(o) });
    },
  },
  {
    id: "extract",
    title: "Extract pages",
    summary: "Keep only the pages you pick, in the order you list them.",
    category: "organize",
    input: "pdf",
    aliases: ["pages extract"],
    fields: [{ ...pagesField("Pages to keep", "For example 1-3,7. Order is kept."), required: true }, password],
    run: (engine, [pdf], o) => engine.core("select", [pdf!], { pages: required(o, "pages", "Pages"), ...pw(o) }),
  },
  {
    id: "delete",
    title: "Delete pages",
    summary: "Remove pages from a PDF.",
    category: "organize",
    input: "pdf",
    aliases: ["pages delete"],
    fields: [{ ...pagesField("Pages to delete", "For example 2,5-7."), required: true }, password],
    run: (engine, [pdf], o) => engine.core("delete", [pdf!], { pages: required(o, "pages", "Pages"), ...pw(o) }),
  },
  {
    id: "reorder",
    title: "Reorder pages",
    summary: "Put pages in a new order; list a page twice to duplicate it.",
    category: "organize",
    input: "pdf",
    aliases: ["pages reorder"],
    fields: [{ ...pagesField("New order", "For example 3,1,2,4-. Reverse with last-1."), name: "order", required: true }, password],
    run: (engine, [pdf], o) => engine.core("select", [pdf!], { pages: required(o, "order", "New order"), ...pw(o) }),
  },
  {
    id: "rotate",
    title: "Rotate pages",
    summary: "Turn pages by 90, 180 or 270 degrees.",
    category: "organize",
    input: "pdf",
    fields: [
      {
        kind: "select",
        name: "angle",
        label: "Rotation",
        default: "90",
        choices: [
          { value: "90", label: "90° clockwise" },
          { value: "180", label: "180°" },
          { value: "270", label: "90° counter-clockwise" },
        ],
      },
      pagesField(),
      password,
    ],
    run: (engine, [pdf], o) => engine.core("rotate", [pdf!], { angle: num(o.angle) ?? 90, pages: str(o.pages), ...pw(o) }),
  },
  {
    id: "blank",
    title: "Insert blank pages",
    summary: "Add empty pages after the pages you choose.",
    category: "organize",
    input: "pdf",
    aliases: ["pages blank"],
    fields: [
      { kind: "text", name: "after", label: "After pages", placeholder: "end", help: "Comma-separated page numbers; 0 inserts before the first page. Empty means at the end." },
      { kind: "number", name: "count", label: "Blank pages each time", min: 1, max: 100, default: 1 },
      {
        kind: "select",
        name: "size",
        label: "Page size",
        default: "same",
        choices: [
          { value: "same", label: "Same as the neighboring page" },
          { value: "a4", label: "A4" },
          { value: "letter", label: "US Letter" },
        ],
      },
      password,
    ],
    run(engine, [pdf], o) {
      const after = (str(o.after) ?? "")
        .split(",")
        .map((s) => s.trim())
        .filter(Boolean)
        .map((s) => {
          const n = Number(s);
          if (!Number.isInteger(n) || n < 0) throw new EngineError("invalid_option", `"${s}" is not a page number`);
          return n;
        });
      return engine.core("blank", [pdf!], { after, count: num(o.count) ?? 1, size: str(o.size) ?? "same", ...pw(o) });
    },
  },
  {
    id: "crop",
    title: "Crop pages",
    summary: "Trim margins off pages.",
    category: "edit",
    input: "pdf",
    fields: [
      { kind: "number", name: "top", label: "Top", unit: "pt", min: 0, max: 5000, default: 0 },
      { kind: "number", name: "right", label: "Right", unit: "pt", min: 0, max: 5000, default: 0 },
      { kind: "number", name: "bottom", label: "Bottom", unit: "pt", min: 0, max: 5000, default: 0 },
      { kind: "number", name: "left", label: "Left", unit: "pt", min: 0, max: 5000, default: 0 },
      pagesField(),
      password,
    ],
    run: (engine, [pdf], o) =>
      engine.core("crop", [pdf!], {
        margins: [num(o.top) ?? 0, num(o.right) ?? 0, num(o.bottom) ?? 0, num(o.left) ?? 0],
        pages: str(o.pages),
        ...pw(o),
      }),
  },
  {
    id: "nup",
    title: "Pages per sheet",
    summary: "Print-style N-up: several pages on each sheet.",
    category: "organize",
    input: "pdf",
    aliases: ["n-up"],
    fields: [
      {
        kind: "select",
        name: "perSheet",
        label: "Pages per sheet",
        default: "2",
        choices: ["2", "4", "6", "8", "9", "16"].map((v) => ({ value: v, label: v })),
      },
      {
        kind: "select",
        name: "sheet",
        label: "Sheet size",
        default: "a4",
        choices: [
          { value: "a4", label: "A4" },
          { value: "letter", label: "US Letter" },
          { value: "same", label: "Same as the first page" },
        ],
      },
      { kind: "boolean", name: "border", label: "Draw a border around each page", default: false },
      { kind: "number", name: "margin", label: "Sheet margin", unit: "pt", min: 0, max: 200, default: 18, advanced: true },
      { kind: "number", name: "gap", label: "Gap between pages", unit: "pt", min: 0, max: 200, default: 12, advanced: true },
      password,
    ],
    run: (engine, [pdf], o) =>
      engine.core("nup", [pdf!], {
        perSheet: num(o.perSheet) ?? 2,
        sheet: str(o.sheet) ?? "a4",
        border: bool(o.border),
        margin: num(o.margin) ?? 18,
        gap: num(o.gap) ?? 12,
        ...pw(o),
      }),
  },
  {
    id: "img2pdf",
    title: "Images to PDF",
    summary: "Turn JPEG, PNG, WebP, GIF or BMP images into one PDF. JPEGs go in untouched.",
    category: "convert",
    input: "images",
    fields: imagePageFields,
    run: (engine, inputs, o) => engine.core("img2pdf", inputs, imagePageOptions(o)),
  },
  {
    id: "scan",
    title: "Scan documents",
    summary: "Photos of paper → a clean PDF: finds the sheet, straightens it, whitens the page. Optional OCR.",
    category: "convert",
    input: "images",
    aliases: ["scanner", "camera"],
    fields: [
      {
        kind: "select",
        name: "filter",
        label: "Look",
        default: "color",
        choices: [
          { value: "color", label: "Color · white paper" },
          { value: "gray", label: "Grayscale" },
          { value: "bw", label: "Black and white · smallest" },
          { value: "original", label: "Original photo" },
        ],
      },
      // A scanned sheet fills its page.
      ...imagePageFields.map((f): Field => (f.kind === "number" && f.name === "margin" ? { ...f, default: 0 } : f)),
      { kind: "boolean", name: "ocr", label: "Make the text searchable (OCR)", default: false },
      {
        kind: "textarea",
        name: "corners",
        label: "Page corners (JSON)",
        advanced: true,
        placeholder: '[{ "corners": [[40, 30], [980, 50], [960, 1400], [20, 1380]], "rotate": 90 }]',
        help: "One entry per photo: the sheet's corners in pixels (top-left, top-right, bottom-right, bottom-left) and a clockwise rotation. Photos without an entry are detected. On the command line, @file.json reads a file.",
      },
    ],
    async run(engine, inputs, o, progress) {
      let pages: unknown = [];
      if (str(o.corners)) {
        try {
          pages = JSON.parse(str(o.corners)!);
        } catch {
          throw new EngineError("invalid_option", "page corners must be JSON");
        }
      }
      const ocr = bool(o.ocr);
      const steps = ocr ? 3 : 2;
      progress?.(0, steps, "Straightening the pages");
      const scanned = await engine.core("scan", inputs, { pages, filter: str(o.filter) ?? "color" });
      progress?.(1, steps, "Making the PDF");
      const made = await engine.core("img2pdf", scanned.files.map((f) => f.bytes), imagePageOptions(o));
      const pdf = { ...made.files[0]!, suffix: "scan" };
      if (!ocr) return { files: [pdf], report: scanned.report };
      const searchable = await ocrPdf(engine, pdf.bytes, {}, (done, total, label) => progress?.(2 + done / Math.max(total, 1), steps, label));
      return { files: [{ ...searchable.files[0]!, suffix: "scan" }], report: { ...(scanned.report as object), ...(searchable.report as object) } };
    },
  },
  {
    id: "pdf2img",
    title: "PDF to images",
    summary: "Render pages as PNG, JPEG or WebP images.",
    category: "convert",
    input: "pdf",
    heavy: "render",
    fields: [
      {
        kind: "select",
        name: "format",
        label: "Format",
        default: "png",
        choices: [
          { value: "png", label: "PNG" },
          { value: "jpeg", label: "JPEG" },
          { value: "webp", label: "WebP (lossless)" },
        ],
      },
      { kind: "number", name: "dpi", label: "Resolution", unit: "DPI", min: 10, max: 1200, default: 150 },
      { kind: "number", name: "quality", label: "JPEG quality", min: 1, max: 100, default: 85, when: { field: "format", is: ["jpeg"] } },
      { kind: "boolean", name: "grayscale", label: "Grayscale", default: false, advanced: true },
      pagesField(),
      password,
    ],
    run: (engine, [pdf], o) =>
      engine.render("render", [pdf!], {
        format: str(o.format) ?? "png",
        dpi: num(o.dpi) ?? 150,
        quality: num(o.quality) ?? 85,
        grayscale: bool(o.grayscale),
        pages: str(o.pages),
        ...pw(o),
      }),
  },
  {
    id: "watermark",
    title: "Watermark",
    summary: "Stamp text across pages: diagonal, centered, tiled, top or bottom.",
    category: "edit",
    input: "pdf",
    fields: [
      { kind: "text", name: "text", label: "Text", required: true, placeholder: "CONFIDENTIAL" },
      {
        kind: "select",
        name: "position",
        label: "Placement",
        default: "diagonal",
        choices: [
          { value: "diagonal", label: "Diagonal" },
          { value: "center", label: "Center" },
          { value: "tile", label: "Tiled" },
          { value: "top", label: "Top" },
          { value: "bottom", label: "Bottom" },
        ],
      },
      { kind: "number", name: "fontSize", label: "Font size", unit: "pt", min: 1, max: 500, default: 64 },
      { kind: "color", name: "color", label: "Color", default: "#c0392b" },
      { kind: "number", name: "opacity", label: "Opacity", min: 0, max: 1, step: 0.05, default: 0.25 },
      { kind: "number", name: "rotation", label: "Rotation", unit: "°", min: -180, max: 180, default: 0, when: { field: "position", is: ["center", "top", "bottom"] }, advanced: true },
      pagesField(),
      password,
    ],
    run: (engine, [pdf], o) =>
      engine.core("watermark", [pdf!], {
        text: required(o, "text", "Text"),
        position: str(o.position) ?? "diagonal",
        fontSize: num(o.fontSize) ?? 64,
        color: str(o.color) ?? "#c0392b",
        opacity: num(o.opacity) ?? 0.25,
        rotation: num(o.rotation) ?? 0,
        pages: str(o.pages),
        ...pw(o),
      }),
  },
  {
    id: "number",
    title: "Page numbers",
    summary: "Number pages, with a format such as “Page {n} of {total}”.",
    category: "edit",
    input: "pdf",
    fields: [
      { kind: "text", name: "format", label: "Format", default: "{n}", help: "{n} is the page number, {total} the page count." },
      { kind: "select", name: "position", label: "Position", default: "bottom-center", choices: positions },
      { kind: "number", name: "start", label: "First number", min: -100000, max: 100000, default: 1 },
      { kind: "boolean", name: "skipFirst", label: "Leave the first page unnumbered", default: false },
      { kind: "number", name: "fontSize", label: "Font size", unit: "pt", min: 1, max: 200, default: 10, advanced: true },
      { kind: "color", name: "color", label: "Color", default: "#333333", advanced: true },
      { kind: "number", name: "margin", label: "Distance from the edge", unit: "pt", min: 0, max: 300, default: 28, advanced: true },
      pagesField(),
      password,
    ],
    run: (engine, [pdf], o) =>
      engine.core("number", [pdf!], {
        format: str(o.format) ?? "{n}",
        position: str(o.position) ?? "bottom-center",
        start: num(o.start) ?? 1,
        skipFirst: bool(o.skipFirst),
        fontSize: num(o.fontSize) ?? 10,
        color: str(o.color) ?? "#333333",
        margin: num(o.margin) ?? 28,
        pages: str(o.pages),
        ...pw(o),
      }),
  },
  {
    id: "header-footer",
    title: "Header & footer",
    summary: "Add text at the top and bottom of pages, left, center or right.",
    category: "edit",
    input: "pdf",
    fields: [
      { kind: "text", name: "headerLeft", label: "Header left" },
      { kind: "text", name: "headerCenter", label: "Header center" },
      { kind: "text", name: "headerRight", label: "Header right" },
      { kind: "text", name: "footerLeft", label: "Footer left" },
      { kind: "text", name: "footerCenter", label: "Footer center", placeholder: "{n} / {total}" },
      { kind: "text", name: "footerRight", label: "Footer right" },
      { kind: "text", name: "date", label: "Value of {date}", help: "Today's date when empty.", advanced: true },
      { kind: "boolean", name: "skipFirst", label: "Skip the first page", default: false },
      { kind: "number", name: "fontSize", label: "Font size", unit: "pt", min: 1, max: 200, default: 10, advanced: true },
      { kind: "color", name: "color", label: "Color", default: "#333333", advanced: true },
      { kind: "number", name: "margin", label: "Distance from the edge", unit: "pt", min: 0, max: 300, default: 28, advanced: true },
      pagesField(),
      password,
    ],
    run: (engine, [pdf], o) =>
      engine.core("header-footer", [pdf!], {
        headerLeft: str(o.headerLeft) ?? "",
        headerCenter: str(o.headerCenter) ?? "",
        headerRight: str(o.headerRight) ?? "",
        footerLeft: str(o.footerLeft) ?? "",
        footerCenter: str(o.footerCenter) ?? "",
        footerRight: str(o.footerRight) ?? "",
        date: str(o.date) ?? new Date().toISOString().slice(0, 10),
        skipFirst: bool(o.skipFirst),
        fontSize: num(o.fontSize) ?? 10,
        color: str(o.color) ?? "#333333",
        margin: num(o.margin) ?? 28,
        pages: str(o.pages),
        ...pw(o),
      }),
  },
  {
    id: "sign",
    title: "Sign (visual)",
    summary: "Place an image of your signature on pages. A visual mark, not a cryptographic signature.",
    category: "edit",
    input: "pdf+image",
    aliases: ["stamp"],
    fields: [
      { kind: "number", name: "x", label: "From the left", unit: "pt", min: 0, max: 20000, default: 360 },
      { kind: "number", name: "y", label: "From the bottom", unit: "pt", min: 0, max: 20000, default: 72 },
      { kind: "number", name: "width", label: "Width", unit: "pt", min: 1, max: 20000, default: 160 },
      { kind: "number", name: "opacity", label: "Opacity", min: 0, max: 1, step: 0.05, default: 1, advanced: true },
      { ...pagesField("Pages", "Default: the last page."), default: "last" },
      password,
    ],
    run: (engine, [pdf, img], o) =>
      engine.core("stamp", [pdf!, img!], {
        x: num(o.x) ?? 360,
        y: num(o.y) ?? 72,
        width: num(o.width) ?? 160,
        opacity: num(o.opacity) ?? 1,
        pages: str(o.pages) ?? "last",
        ...pw(o),
      }),
  },
  {
    id: "edit",
    title: "Edit PDF",
    summary: "Change existing text, add text, shapes, drawings, images and signatures; fill forms, comment, redact, reorder and rotate pages.",
    category: "edit",
    input: "pdf+images",
    aliases: ["annotate", "fill", "redact"],
    fields: [
      {
        kind: "textarea",
        name: "ops",
        label: "Edits (JSON)",
        required: true,
        placeholder: '{ "items": [{ "type": "text", "page": 0, "x": 72, "y": 720, "text": "Hello" }] }',
        help: "Items, form values, blank pages and page order; images and font files given after the PDF are attachment 0, 1… On the command line, @file.json reads a file.",
      },
      {
        kind: "boolean",
        name: "systemFonts",
        label: "Use the fonts installed on this computer to keep the document's fonts",
        default: true,
        advanced: true,
      },
      password,
    ],
    async run(engine, [pdf, ...attachments], o, progress) {
      const ops = parseEdits(required(o, "ops", "Edits"));
      const { order, redactDpi, ...edits } = ops;
      const boxes = redactionBoxes(ops);
      const steps = 2 + (boxes.length ? 1 : 0) + (order ? 1 : 0);
      progress?.(0, steps, "Preparing the edits");
      // Only the bundled fonts the text needs are loaded and passed along.
      const plan = await engine.core<{ fonts: string[]; wanted: { name: string }[] }>("edit-plan", [pdf!], { ...edits, ...pw(o) });
      const keys = plan.report.fonts.filter(isFontKey);
      const fonts = await Promise.all(keys.map((k) => engine.font(k)));
      const bundled = Object.fromEntries(keys.map((k, i) => [k, attachments.length + i]));
      // A document font whose subset lacks characters: its complete, installed copy keeps the line in it.
      const wanted = bool(o.systemFonts ?? true) ? plan.report.wanted : [];
      const installed = (await Promise.all(wanted.map(async (w) => ({ name: w.name, bytes: await engine.systemFont(w.name) })))).filter(
        (f): f is { name: string; bytes: Uint8Array } => f.bytes !== null,
      );
      const first = attachments.length + fonts.length;
      const substitutes = [...(edits.substitutes ?? []), ...installed.map((f, i) => ({ name: f.name, file: first + i }))];
      progress?.(1, steps, "Applying the edits");
      type CoreReport = Omit<EditReport, "redactedPages" | "flattenedPages"> & { redactedPages: number[]; flatten: number[] };
      const edited = await engine.core<CoreReport>("edit", [pdf!, ...attachments, ...fonts, ...installed.map((f) => f.bytes)], {
        ...edits,
        bundled,
        substitutes,
        ...pw(o),
      });
      let bytes = edited.files[0]!.bytes;
      let flattenedPages: number[] = [];
      if (boxes.length) {
        // A second, independent reading of every redacted page; what it doubts becomes an image.
        progress?.(2, steps, "Checking the redactions");
        const checked = await engine.render<{ flattenedPages: number[] }>("redact-check", [bytes], {
          pages: boxes,
          flatten: edited.report.flatten,
          dpi: redactDpi ?? 150,
        });
        bytes = checked.files[0]!.bytes;
        flattenedPages = checked.report.flattenedPages;
      }
      if (order) {
        progress?.(steps - 1, steps, "Arranging the pages");
        bytes = (await engine.core("arrange", [bytes], { order: order.map((p) => ({ page: p.page, rotate: p.rotate ?? 0 })) })).files[0]!.bytes;
      }
      const { flatten: _, ...rest } = edited.report;
      const report: EditReport = {
        ...rest,
        pages: order?.length ?? edited.report.pages,
        redactedPages: boxes.map((b) => b.page + 1),
        flattenedPages,
      };
      return { files: [{ suffix: "edited", ext: "pdf", bytes }], report };
    },
  },
  {
    id: "meta",
    title: "Edit metadata",
    summary: "Read, change or wipe the title, author and other document properties.",
    category: "edit",
    input: "pdf",
    fields: [
      { kind: "text", name: "title", label: "Title" },
      { kind: "text", name: "author", label: "Author" },
      { kind: "text", name: "subject", label: "Subject" },
      { kind: "text", name: "keywords", label: "Keywords" },
      { kind: "text", name: "creator", label: "Creator", advanced: true },
      { kind: "text", name: "producer", label: "Producer", advanced: true },
      { kind: "boolean", name: "clear", label: "Remove all existing metadata first (including XMP)", default: false },
      password,
    ],
    async run(engine, [pdf], o) {
      const keys = ["title", "author", "subject", "keywords", "creator", "producer"] as const;
      const set: Record<string, string> = {};
      for (const k of keys) {
        const v = o[k];
        if (v !== undefined) set[k[0]!.toUpperCase() + k.slice(1)] = String(v);
      }
      if (Object.keys(set).length === 0 && !bool(o.clear)) return engine.core("meta-read", [pdf!], pw(o));
      return engine.core("meta", [pdf!], { set, clear: bool(o.clear), ...pw(o) });
    },
  },
  {
    id: "encrypt",
    title: "Protect with password",
    summary: "Encrypt with AES-256 so the PDF needs a password to open.",
    category: "security",
    input: "pdf",
    aliases: ["protect"],
    fields: [
      { kind: "password", name: "userPassword", label: "Password to open", required: true },
      { kind: "password", name: "ownerPassword", label: "Owner password", help: "Lets you change permissions later. Random when empty.", advanced: true },
      ...permissionFields(true).map((f) => ({ ...f, advanced: true })),
      password,
    ],
    run: (engine, [pdf], o) =>
      engine.core("encrypt", [pdf!], {
        userPassword: required(o, "userPassword", "Password to open"),
        ownerPassword: str(o.ownerPassword),
        permissions: permissions(o),
        ...pw(o),
      }),
  },
  {
    id: "permissions",
    title: "Restrict permissions",
    summary: "Opens without a password, but printing, copying or editing are restricted.",
    category: "security",
    input: "pdf",
    fields: [{ kind: "password", name: "ownerPassword", label: "Owner password", required: true }, ...permissionFields(false), password],
    run: (engine, [pdf], o) =>
      engine.core("encrypt", [pdf!], {
        userPassword: "",
        ownerPassword: required(o, "ownerPassword", "Owner password"),
        permissions: permissions(o),
        ...pw(o),
      }),
  },
  {
    id: "decrypt",
    title: "Remove password",
    summary: "Unlock a protected PDF you have the password for.",
    category: "security",
    input: "pdf",
    aliases: ["unlock"],
    fields: [{ kind: "password", name: "password", label: "Password", required: true }],
    run: (engine, [pdf], o) => engine.core("decrypt", [pdf!], { password: required(o, "password", "Password") }),
  },
  {
    id: "flatten",
    title: "Flatten forms",
    summary: "Burn form fields into the page so they can no longer be edited.",
    category: "edit",
    input: "pdf",
    fields: [password],
    run: (engine, [pdf], o) => engine.core("flatten", [pdf!], pw(o)),
  },
  {
    id: "repair",
    title: "Repair PDF",
    summary: "Rebuild a damaged file's structure from whatever can be recovered.",
    category: "edit",
    input: "pdf",
    fields: [password],
    run: (engine, [pdf], o) => engine.core("repair", [pdf!], pw(o)),
  },
  {
    id: "compare",
    title: "Compare PDFs",
    summary: "Find visual differences page by page; changes are highlighted in red.",
    category: "edit",
    input: "pdf+pdf",
    heavy: "render",
    aliases: ["diff"],
    fields: [{ kind: "number", name: "dpi", label: "Resolution", unit: "DPI", min: 20, max: 300, default: 72 }, password],
    run: (engine, inputs, o) => engine.render("compare", inputs, { dpi: num(o.dpi) ?? 72, ...pw(o) }),
  },
  {
    id: "ocr",
    title: "OCR (searchable PDF)",
    summary: "Recognize text on scanned pages and add an invisible, selectable text layer. 50 languages.",
    category: "convert",
    input: "pdf",
    heavy: "ocr",
    fields: [
      { kind: "number", name: "dpi", label: "Recognition resolution", unit: "DPI", min: 100, max: 400, default: 200, advanced: true },
      { kind: "boolean", name: "force", label: "Also OCR pages that already have text", default: false },
      pagesField(),
      password,
    ],
    run: (engine, [pdf], o, progress) => ocrPdf(engine, pdf!, o, progress),
  },
  {
    id: "to-md",
    title: "PDF to Markdown",
    summary: "Extract text as Markdown; headings come from font sizes. Scanned pages go through OCR.",
    category: "convert",
    input: "pdf",
    heavy: "render",
    fields: [
      { kind: "boolean", name: "ocr", label: "OCR pages without text", default: true },
      engineField,
      { kind: "boolean", name: "pageBreaks", label: "Mark page breaks", default: false },
      password,
    ],
    async run(engine, [pdf], o, progress) {
      const r = await markdown(engine, pdf!, o, false, progress);
      return { files: [r.file], report: { pages: r.pages, ocrPages: r.ocrPages } };
    },
  },
  {
    id: "to-text",
    title: "PDF to text",
    summary: "Plain text, in reading order. Scanned pages go through OCR.",
    category: "convert",
    input: "pdf",
    heavy: "render",
    fields: [
      { kind: "boolean", name: "ocr", label: "OCR pages without text", default: true },
      engineField,
      { kind: "boolean", name: "pageBreaks", label: "Form feed between pages", default: false },
      password,
    ],
    async run(engine, [pdf], o, progress) {
      const r = await markdown(engine, pdf!, o, true, progress);
      return { files: [{ ...r.file, suffix: "text", ext: "txt" }], report: { pages: r.pages, ocrPages: r.ocrPages } };
    },
  },
  {
    id: "to-epub",
    title: "PDF to EPUB",
    summary: "A reflowable e-book, one chapter per top-level heading.",
    category: "convert",
    input: "pdf",
    heavy: "render",
    fields: [
      { kind: "text", name: "title", label: "Book title", help: "The PDF's title when empty." },
      { kind: "text", name: "author", label: "Author", help: "The PDF's author when empty." },
      { kind: "text", name: "language", label: "Language", default: "en", advanced: true },
      { kind: "boolean", name: "ocr", label: "OCR pages without text", default: true },
      engineField,
      password,
    ],
    async run(engine, [pdf], o, progress) {
      const r = await markdown(engine, pdf!, o, false, progress);
      const meta = await engine.core<{ info: Record<string, string> }>("meta-read", [pdf!], pw(o));
      const book = await engine.core("epub", [r.file.bytes], {
        title: str(o.title) ?? meta.report.info.Title ?? "Untitled",
        author: str(o.author) ?? meta.report.info.Author ?? "",
        language: str(o.language) ?? "en",
      });
      return { files: book.files, report: { pages: r.pages, ocrPages: r.ocrPages } };
    },
  },
  {
    id: "info",
    title: "PDF info",
    summary: "Pages, sizes, fonts, images, forms, protection and metadata at a glance.",
    category: "edit",
    input: "pdf",
    fields: [password],
    run: (engine, [pdf], o) => engine.core("info", [pdf!], pw(o)),
  },
  // ------------------------------------------------------------ images
  {
    id: "img-compress",
    title: "Compress images",
    summary: "Smaller JPEG, PNG or WebP files; never bigger than the original.",
    category: "image",
    input: "image",
    aliases: ["img compress"],
    fields: [
      { kind: "number", name: "quality", label: "JPEG quality", min: 1, max: 100, default: 75 },
      { kind: "number", name: "colors", label: "PNG colors", min: 2, max: 256, help: "Reduce PNGs to a palette (lossy). Empty keeps every color.", advanced: true },
      {
        kind: "select",
        name: "format",
        label: "Output format",
        default: "keep",
        advanced: true,
        choices: [
          { value: "keep", label: "Same as input" },
          { value: "jpeg", label: "JPEG" },
          { value: "png", label: "PNG" },
          { value: "webp", label: "WebP (lossless)" },
        ],
      },
    ],
    run: (engine, [img], o) =>
      engine.core("image-compress", [img!], {
        quality: num(o.quality) ?? 75,
        colors: num(o.colors),
        format: str(o.format) === "keep" ? undefined : str(o.format),
      }),
  },
  {
    id: "img-resize",
    title: "Resize images",
    summary: "Fit within a box, set an exact size, or scale by a percentage.",
    category: "image",
    input: "image",
    aliases: ["img resize"],
    fields: [
      { kind: "number", name: "maxWidth", label: "Max width", unit: "px", min: 1, max: 50000 },
      { kind: "number", name: "maxHeight", label: "Max height", unit: "px", min: 1, max: 50000 },
      { kind: "number", name: "width", label: "Exact width", unit: "px", min: 1, max: 50000, advanced: true },
      { kind: "number", name: "height", label: "Exact height", unit: "px", min: 1, max: 50000, advanced: true },
      { kind: "number", name: "percent", label: "Scale", unit: "%", min: 1, max: 1000, advanced: true },
      { kind: "number", name: "quality", label: "JPEG quality", min: 1, max: 100, default: 90, advanced: true },
    ],
    run(engine, [img], o) {
      const any = ["maxWidth", "maxHeight", "width", "height", "percent"].some((k) => num(o[k]) !== undefined);
      if (!any) throw new EngineError("invalid_option", "give a max size, an exact size or a percentage");
      return engine.core("image", [img!], {
        maxWidth: num(o.maxWidth),
        maxHeight: num(o.maxHeight),
        width: num(o.width),
        height: num(o.height),
        percent: num(o.percent),
        quality: num(o.quality) ?? 90,
      });
    },
  },
  {
    id: "img-convert",
    title: "Convert images",
    summary: "Between JPEG, PNG and WebP. GIF and BMP read too.",
    category: "image",
    input: "image",
    aliases: ["img convert"],
    fields: [
      {
        kind: "select",
        name: "format",
        label: "To",
        default: "png",
        choices: [
          { value: "png", label: "PNG" },
          { value: "jpeg", label: "JPEG" },
          { value: "webp", label: "WebP (lossless)" },
        ],
      },
      { kind: "number", name: "quality", label: "JPEG quality", min: 1, max: 100, default: 90, when: { field: "format", is: ["jpeg"] } },
    ],
    run: (engine, [img], o) => engine.core("image", [img!], { format: str(o.format) ?? "png", quality: num(o.quality) ?? 90 }),
  },
  {
    id: "img-strip",
    title: "Remove image metadata",
    summary: "Drop EXIF (GPS, camera), XMP and comments without re-encoding.",
    category: "image",
    input: "image",
    aliases: ["img strip"],
    fields: [],
    run: (engine, [img]) => engine.core("image-strip", [img!], {}),
  },
  {
    id: "img-edit",
    title: "Crop & rotate images",
    summary: "Crop to a rectangle, rotate by 90° steps, flip.",
    category: "image",
    input: "image",
    aliases: ["img edit", "img crop", "img rotate"],
    fields: [
      { kind: "number", name: "cropX", label: "Crop from left", unit: "px", min: 0, max: 50000 },
      { kind: "number", name: "cropY", label: "Crop from top", unit: "px", min: 0, max: 50000 },
      { kind: "number", name: "cropWidth", label: "Crop width", unit: "px", min: 1, max: 50000 },
      { kind: "number", name: "cropHeight", label: "Crop height", unit: "px", min: 1, max: 50000 },
      {
        kind: "select",
        name: "rotate",
        label: "Rotate",
        default: "0",
        choices: [
          { value: "0", label: "No rotation" },
          { value: "90", label: "90° clockwise" },
          { value: "180", label: "180°" },
          { value: "270", label: "90° counter-clockwise" },
        ],
      },
      {
        kind: "select",
        name: "flip",
        label: "Flip",
        default: "none",
        choices: [
          { value: "none", label: "No flip" },
          { value: "horizontal", label: "Horizontal" },
          { value: "vertical", label: "Vertical" },
        ],
      },
    ],
    run(engine, [img], o) {
      const c = [num(o.cropX), num(o.cropY), num(o.cropWidth), num(o.cropHeight)];
      const some = c.some((v) => v !== undefined);
      if (some && c.some((v) => v === undefined)) throw new EngineError("invalid_option", "a crop needs x, y, width and height");
      return engine.core("image", [img!], {
        crop: some ? c : undefined,
        rotate: num(o.rotate) || undefined,
        flip: str(o.flip) === "none" ? undefined : str(o.flip),
      });
    },
  },
];

export const toolById = new Map(tools.map((t) => [t.id, t]));

/** Fill in each field's default for options the caller left out. */
export function withDefaults(tool: Tool, options: Options): Options {
  const out: Options = { ...options };
  for (const f of tool.fields) {
    if (out[f.name] === undefined && "default" in f && f.default !== undefined) out[f.name] = f.default;
  }
  return out;
}

/** `report.pdf` + suffix "compressed" → `report-compressed.pdf`. */
export function outputName(inputName: string, file: OutFile): string {
  const base = inputName.replace(/\.[^./\\]+$/, "") || "output";
  return `${base}-${file.suffix}.${file.ext}`;
}

export const categories: { id: Category; title: string }[] = [
  { id: "compress", title: "Compress" },
  { id: "organize", title: "Organize" },
  { id: "edit", title: "Edit" },
  { id: "security", title: "Security" },
  { id: "convert", title: "Convert" },
  { id: "image", title: "Images" },
];
