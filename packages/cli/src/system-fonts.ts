// Fonts installed on this computer, found by PostScript name, so a changed
// line can be redrawn in the complete font its PDF only embeds a subset of.
// Only each file's table directory and `name` table are read.
import { readdir } from "node:fs/promises";
import { homedir, platform } from "node:os";
import { extname, join } from "node:path";

const FONT_EXT = new Set([".ttf", ".otf", ".ttc", ".otc"]);

/** Where fonts live; PDFVAULT_FONT_DIRS (separated like PATH) replaces the defaults. */
export function fontDirs(): string[] {
  const custom = process.env.PDFVAULT_FONT_DIRS;
  if (custom !== undefined) return custom.split(platform() === "win32" ? ";" : ":").filter(Boolean);
  const home = homedir();
  switch (platform()) {
    case "darwin":
      return ["/System/Library/Fonts", "/Library/Fonts", join(home, "Library/Fonts")];
    case "win32":
      return [join(process.env.WINDIR ?? "C:\\Windows", "Fonts"), join(process.env.LOCALAPPDATA ?? join(home, "AppData/Local"), "Microsoft/Windows/Fonts")];
    default:
      return ["/usr/share/fonts", "/usr/local/share/fonts", join(home, ".fonts"), join(home, ".local/share/fonts")];
  }
}

async function* walk(dir: string, depth = 0): AsyncGenerator<string> {
  if (depth > 6) return;
  let entries;
  try {
    entries = await readdir(dir, { withFileTypes: true });
  } catch {
    return;
  }
  for (const e of entries) {
    const p = join(dir, e.name);
    if (e.isDirectory()) yield* walk(p, depth + 1);
    else if (FONT_EXT.has(extname(e.name).toLowerCase())) yield p;
  }
}

async function read(path: string, start: number, length: number): Promise<DataView> {
  const buf = await Bun.file(path).slice(start, start + length).arrayBuffer();
  return new DataView(buf);
}

const tagAt = (v: DataView, o: number) => String.fromCharCode(v.getUint8(o), v.getUint8(o + 1), v.getUint8(o + 2), v.getUint8(o + 3));

function decodeName(v: DataView, platformId: number): string {
  const bytes = new Uint8Array(v.buffer, v.byteOffset, v.byteLength);
  if (platformId === 0 || platformId === 3) return new TextDecoder("utf-16be").decode(bytes);
  return String.fromCharCode(...bytes);
}

/** The PostScript name of the face whose table directory starts at `offset`. */
async function faceName(path: string, offset: number): Promise<string | null> {
  const head = await read(path, offset, 12);
  if (head.byteLength < 12) return null;
  const tables = head.getUint16(4);
  const dir = await read(path, offset + 12, tables * 16);
  for (let i = 0; i < tables && (i + 1) * 16 <= dir.byteLength; i++) {
    if (tagAt(dir, i * 16) !== "name") continue;
    const at = dir.getUint32(i * 16 + 8);
    const length = dir.getUint32(i * 16 + 12);
    const name = await read(path, at, Math.min(length, 1 << 20));
    const count = name.getUint16(2);
    const strings = name.getUint16(4);
    for (let r = 0; r < count; r++) {
      const rec = 6 + r * 12;
      if (rec + 12 > name.byteLength || name.getUint16(rec + 6) !== 6) continue;
      const plat = name.getUint16(rec);
      const len = name.getUint16(rec + 8);
      const off = strings + name.getUint16(rec + 10);
      if (off + len > name.byteLength) continue;
      return decodeName(new DataView(name.buffer, name.byteOffset + off, len), plat).trim();
    }
  }
  return null;
}

/** PostScript names of every face in a font file. */
export async function postscriptNames(path: string): Promise<string[]> {
  try {
    const head = await read(path, 0, 12);
    if (head.byteLength < 12) return [];
    if (tagAt(head, 0) === "ttcf") {
      const n = Math.min(head.getUint32(8), 256);
      const offsets = await read(path, 12, n * 4);
      const names = await Promise.all(Array.from({ length: n }, (_, i) => faceName(path, offsets.getUint32(i * 4))));
      return names.filter((x): x is string => Boolean(x));
    }
    const one = await faceName(path, 0);
    return one ? [one] : [];
  } catch {
    return [];
  }
}

export const nameKey = (s: string) => s.toLowerCase().replace(/[^a-z0-9]/g, "");

let index: Promise<Map<string, string>> | null = null;

/** PostScript name (normalized) → the file holding it, built once. */
function fontIndex(): Promise<Map<string, string>> {
  index ??= (async () => {
    const map = new Map<string, string>();
    for (const dir of fontDirs()) {
      for await (const path of walk(dir)) {
        for (const name of await postscriptNames(path)) if (!map.has(nameKey(name))) map.set(nameKey(name), path);
      }
    }
    return map;
  })();
  return index;
}

/** The installed font file with a face named `postscriptName`, if any. */
export async function systemFont(postscriptName: string): Promise<Uint8Array | null> {
  const path = (await fontIndex()).get(nameKey(postscriptName));
  return path ? new Uint8Array(await Bun.file(path).arrayBuffer()) : null;
}
