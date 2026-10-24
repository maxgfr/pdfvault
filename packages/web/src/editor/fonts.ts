// Fonts on the editor's side: the bundled Liberation fonts previewed exactly
// as the engine will write them, a font from this device (Local Font Access,
// where the browser has it) for scripts they lack, and the page's own
// background colour behind text being rewritten.

import { BUNDLED_FONTS, type FontKey } from "@pdfvault/engine";

export type Family = "sans" | "serif" | "mono";

const CSS_NAME: Record<Family, string> = { sans: "PV Sans", serif: "PV Serif", mono: "PV Mono" };
const FALLBACK: Record<Family, string> = {
  sans: "Arial, Helvetica, sans-serif",
  serif: "'Times New Roman', Times, serif",
  mono: "'Courier New', Courier, monospace",
};

/** The CSS font stack previewing a bundled family. */
export function previewFamily(family: Family): string {
  return `"${CSS_NAME[family]}", ${FALLBACK[family]}`;
}

const loaded = new Map<FontKey, Promise<void>>();

/** Load one bundled face for previews (once); the page re-renders on its own when it arrives. */
export function ensurePreviewFont(family: Family, bold: boolean, italic: boolean): void {
  const key = `${family}-${bold ? (italic ? "bolditalic" : "bold") : italic ? "italic" : "regular"}` as FontKey;
  if (loaded.has(key) || typeof FontFace === "undefined") return;
  const face = new FontFace(CSS_NAME[family], `url(fonts/${BUNDLED_FONTS[key]})`, {
    weight: bold ? "700" : "400",
    style: italic ? "italic" : "normal",
  });
  loaded.set(
    key,
    face
      .load()
      .then((f) => {
        document.fonts.add(f);
      })
      .catch(() => {
        // Offline before the font was cached: the fallback stack shows instead.
        loaded.delete(key);
      }),
  );
}

/** A font file the edit will embed for characters the bundled fonts lack. */
export interface ExtraFont {
  name: string;
  bytes: Uint8Array;
  /** Picks the face in a collection (.ttc). */
  postscriptName?: string;
}

const inRange = (chars: string, ranges: [number, number][]) =>
  [...chars].some((c) => {
    const cp = c.codePointAt(0)!;
    return ranges.some(([a, b]) => cp >= a && cp <= b);
  });

/** Font families, in order of preference, likely to draw `chars` on macOS, Windows, Linux or ChromeOS. */
export function familiesFor(chars: string): string[] {
  const out: string[] = [];
  const kana = inRange(chars, [[0x3040, 0x30ff]]);
  const hangul = inRange(chars, [
    [0xac00, 0xd7af],
    [0x1100, 0x11ff],
    [0x3130, 0x318f],
  ]);
  const han = inRange(chars, [
    [0x4e00, 0x9fff],
    [0x3400, 0x4dbf],
    [0x3000, 0x303f],
    [0xff00, 0xffef],
  ]);
  if (kana) out.push("Hiragino Sans", "Hiragino Kaku Gothic ProN", "Yu Gothic", "Meiryo", "MS Gothic", "Noto Sans JP", "Noto Sans CJK JP");
  if (hangul) out.push("Apple SD Gothic Neo", "Malgun Gothic", "Noto Sans KR", "Noto Sans CJK KR", "NanumGothic");
  if (han) out.push("PingFang SC", "Hiragino Sans GB", "Microsoft YaHei", "SimSun", "Noto Sans SC", "Noto Sans CJK SC", "Heiti SC", "Songti SC");
  if (
    inRange(chars, [
      [0x0600, 0x06ff],
      [0x0750, 0x077f],
      [0xfb50, 0xfdff],
      [0xfe70, 0xfeff],
    ])
  )
    out.push("Geeza Pro", "Arial", "Segoe UI", "Tahoma", "Noto Naskh Arabic", "Noto Sans Arabic", "Al Nile");
  if (inRange(chars, [[0x0900, 0x097f]])) out.push("Kohinoor Devanagari", "Devanagari Sangam MN", "Nirmala UI", "Mangal", "Noto Sans Devanagari");
  if (inRange(chars, [[0x0980, 0x09ff]])) out.push("Kohinoor Bangla", "Bangla Sangam MN", "Nirmala UI", "Vrinda", "Noto Sans Bengali");
  if (inRange(chars, [[0x0b80, 0x0bff]])) out.push("Tamil Sangam MN", "Nirmala UI", "Latha", "Noto Sans Tamil");
  if (inRange(chars, [[0x0e00, 0x0e7f]])) out.push("Thonburi", "Leelawadee UI", "Tahoma", "Noto Sans Thai");
  out.push("Arial Unicode MS", "Segoe UI", "Noto Sans", "DejaVu Sans");
  return [...new Set(out)];
}

interface LocalFontData {
  family: string;
  fullName: string;
  postscriptName: string;
  style: string;
  blob(): Promise<Blob>;
}

export const canQueryLocalFonts = () => typeof window !== "undefined" && "queryLocalFonts" in window;

/** Ask the browser for a font on this device likely to have `chars` (prompts for permission). */
export async function findLocalFont(chars: string): Promise<ExtraFont | null> {
  const query = (window as unknown as { queryLocalFonts?: () => Promise<LocalFontData[]> }).queryLocalFonts;
  if (!query) return null;
  const fonts = await query();
  for (const family of familiesFor(chars)) {
    const same = fonts.filter((f) => f.family === family);
    const pick = same.find((f) => /^(regular|normal|roman|book|w3)$/i.test(f.style)) ?? same[0];
    if (!pick) continue;
    const bytes = new Uint8Array(await (await pick.blob()).arrayBuffer());
    return { name: pick.fullName, bytes, postscriptName: pick.postscriptName };
  }
  return null;
}

/** The colour around a box of a rendered page (visual points, y up), as #rrggbb. */
export function sampleBackground(img: HTMLImageElement | null, W: number, H: number, box: [number, number, number, number]): string {
  if (!img || !img.naturalWidth) return "#ffffff";
  const sx = img.naturalWidth / W;
  const sy = img.naturalHeight / H;
  const pad = 2;
  const x0 = Math.max(0, Math.floor(box[0] * sx) - pad);
  const x1 = Math.min(img.naturalWidth - 1, Math.ceil(box[2] * sx) + pad);
  const y0 = Math.max(0, Math.floor((H - box[3]) * sy) - pad);
  const y1 = Math.min(img.naturalHeight - 1, Math.ceil((H - box[1]) * sy) + pad);
  const canvas = document.createElement("canvas");
  canvas.width = x1 - x0 + 1;
  canvas.height = y1 - y0 + 1;
  const ctx = canvas.getContext("2d", { willReadFrequently: true });
  if (!ctx || canvas.width < 2 || canvas.height < 2) return "#ffffff";
  ctx.drawImage(img, x0, y0, canvas.width, canvas.height, 0, 0, canvas.width, canvas.height);
  const data = ctx.getImageData(0, 0, canvas.width, canvas.height).data;
  // The border of the box: the page around the text, not the text itself.
  const counts = new Map<string, number>();
  const take = (x: number, y: number) => {
    const i = (y * canvas.width + x) * 4;
    // Quantize so JPEG noise votes for the same colour.
    const key = [data[i]!, data[i + 1]!, data[i + 2]!].map((v) => Math.min(255, Math.round(v / 8) * 8)).join(",");
    counts.set(key, (counts.get(key) ?? 0) + 1);
  };
  for (let x = 0; x < canvas.width; x++) {
    take(x, 0);
    take(x, canvas.height - 1);
  }
  for (let y = 0; y < canvas.height; y++) {
    take(0, y);
    take(canvas.width - 1, y);
  }
  const best = [...counts].sort((a, b) => b[1] - a[1])[0]?.[0] ?? "255,255,255";
  return `#${best
    .split(",")
    .map((v) => Number(v).toString(16).padStart(2, "0"))
    .join("")}`;
}
