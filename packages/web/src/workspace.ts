// The files you are working on, kept in memory only: nothing is stored, and
// a reload empties it. Pages subscribe to it so files survive moving between
// the home page, the editor and back.

import { EngineError } from "@pdfvault/engine";
import { useSyncExternalStore } from "react";
import { coreReport, runJob } from "./runner.ts";

export type Kind = "pdf" | "image" | "unsupported";

export interface WorkFile {
  key: string;
  name: string;
  bytes: Uint8Array;
  kind: Kind;
  /** Why an unsupported file cannot be used, and what to do instead. */
  problem?: string;
  pages?: number;
  /** The PDF needs a password to open. */
  locked?: boolean;
  password?: string;
  /** Object URL of a preview: the image itself, or page 1 of a PDF. */
  thumb?: string;
  /** The file this one replaced, for "Restore original". */
  previous?: WorkFile;
  /** Produced here and not downloaded or saved yet. */
  unsaved?: boolean;
}

const has = (b: Uint8Array, at: number, ascii: string) => [...ascii].every((c, i) => b[at + i] === c.charCodeAt(0));

export const isPdf = (b: Uint8Array) => new TextDecoder().decode(b.subarray(0, 1024)).includes("%PDF-");

/** The image formats the engine decodes (images::Format::detect). */
export function isImage(b: Uint8Array): boolean {
  return (
    (b[0] === 0xff && b[1] === 0xd8 && b[2] === 0xff) ||
    has(b, 0, "\x89PNG") ||
    has(b, 0, "GIF8") ||
    has(b, 0, "BM") ||
    (has(b, 0, "RIFF") && has(b, 8, "WEBP"))
  );
}

/** HEIC (iPhone) or AVIF: photos in an ISO-BMFF box the engine cannot decode. */
function isoPhoto(b: Uint8Array): "heic" | "avif" | undefined {
  if (!has(b, 4, "ftyp")) return undefined;
  const brand = String.fromCharCode(...b.subarray(8, 12));
  if (/^(heic|heix|hevc|heim|heis|mif1|msf1)$/.test(brand)) return "heic";
  if (/^avi[fs]$/.test(brand)) return "avif";
  return undefined;
}

/** Redraw a photo as JPEG with the browser's own decoder; null where it cannot read it. */
export async function redrawAsJpeg(file: Blob): Promise<Uint8Array | null> {
  try {
    const bitmap = await createImageBitmap(file);
    const canvas = new OffscreenCanvas(bitmap.width, bitmap.height);
    canvas.getContext("2d")!.drawImage(bitmap, 0, 0);
    bitmap.close();
    return new Uint8Array(await (await canvas.convertToBlob({ type: "image/jpeg", quality: 0.92 })).arrayBuffer());
  } catch {
    return null;
  }
}

/** What kind of file this is, and for the ones we cannot read, why. */
export function classify(name: string, b: Uint8Array): { kind: Kind; problem?: string } {
  if (isPdf(b)) return { kind: "pdf" };
  if (isImage(b)) return { kind: "image" };
  const ext = name.toLowerCase().split(".").pop() ?? "";
  const photo = isoPhoto(b);
  if (photo === "heic") {
    return {
      kind: "unsupported",
      problem: "This browser cannot read HEIC photos. Open pdfvault in Safari, or export it as JPEG (on iPhone: Settings › Camera › Formats › Most Compatible).",
    };
  }
  if (photo === "avif") return { kind: "unsupported", problem: "This browser cannot read AVIF images. Save it as JPEG or PNG first." };
  if (has(b, 0, "PK\x03\x04")) {
    if (/^(docx?|xlsx?|pptx?|odt|ods|odp|pages|key|numbers)$/.test(ext)) {
      return { kind: "unsupported", problem: "Office documents cannot be converted in the browser. Export it to PDF from its app, then drop the PDF." };
    }
    return { kind: "unsupported", problem: "This is an archive. Unzip it and drop the files inside." };
  }
  if (/^(doc|xls|ppt|rtf)$/.test(ext)) {
    return { kind: "unsupported", problem: "Office documents cannot be converted in the browser. Export it to PDF from its app, then drop the PDF." };
  }
  if (/^(tiff?|svg|psd|raw|cr2|nef|dng)$/.test(ext)) {
    return { kind: "unsupported", problem: "This image format cannot be read. Convert it to JPEG or PNG first." };
  }
  return { kind: "unsupported", problem: "pdfvault reads PDFs and JPEG, PNG, WebP, GIF or BMP images." };
}

type Listener = () => void;

class Workspace {
  #files: WorkFile[] = [];
  #listeners = new Set<Listener>();
  /** The file the editor should open next. */
  editing: string | null = null;
  /** The file the viewer should open next. */
  viewing: string | null = null;

  subscribe(fn: Listener): () => void {
    this.#listeners.add(fn);
    return () => this.#listeners.delete(fn);
  }

  get files(): WorkFile[] {
    return this.#files;
  }

  get(key: string): WorkFile | undefined {
    return this.#files.find((f) => f.key === key);
  }

  #set(files: WorkFile[]) {
    this.#files = files;
    for (const fn of this.#listeners) fn();
  }

  #update(key: string, patch: Partial<WorkFile>) {
    this.#set(this.#files.map((f) => (f.key === key ? { ...f, ...patch } : f)));
  }

  /** Add files, in order; previews and page counts arrive in the background. */
  add(items: { name: string; bytes: Uint8Array; unsaved?: boolean; password?: string }[]): WorkFile[] {
    const added = items.map((i) => ({ ...this.#make(i.name, i.bytes, i.unsaved), password: i.password || undefined }));
    this.#set([...this.#files, ...added]);
    for (const f of added) this.#inspect(f);
    return added;
  }

  #make(name: string, bytes: Uint8Array, unsaved?: boolean): WorkFile {
    const { kind, problem } = classify(name, bytes);
    const f: WorkFile = { key: crypto.randomUUID(), name, bytes, kind, problem, unsaved };
    if (kind === "image") f.thumb = URL.createObjectURL(new Blob([bytes as BlobPart]));
    return f;
  }

  remove(key: string) {
    const f = this.get(key);
    if (f) release(f);
    this.#set(this.#files.filter((x) => x.key !== key));
  }

  clear() {
    this.#files.forEach(release);
    this.#set([]);
  }

  move(key: string, by: -1 | 1) {
    const at = this.#files.findIndex((f) => f.key === key);
    const to = at + by;
    if (at < 0 || to < 0 || to >= this.#files.length) return;
    const next = [...this.#files];
    const [item] = next.splice(at, 1);
    next.splice(to, 0, item!);
    this.#set(next);
  }

  /** Put a result in place of `key`, keeping the old file one step back. */
  replace(key: string, name: string, bytes: Uint8Array): WorkFile | undefined {
    const old = this.get(key);
    if (!old) return undefined;
    const next = { ...this.#make(name, bytes, true), key: old.key, previous: { ...old, previous: undefined } };
    if (old.previous) release(old.previous);
    this.#set(this.#files.map((f) => (f.key === key ? next : f)));
    this.#inspect(next);
    return next;
  }

  restore(key: string) {
    const cur = this.get(key);
    if (!cur?.previous) return;
    if (cur.thumb && cur.thumb !== cur.previous.thumb) URL.revokeObjectURL(cur.thumb);
    this.#set(this.#files.map((f) => (f.key === key ? { ...cur.previous!, key } : f)));
  }

  saved(key: string) {
    this.#update(key, { unsaved: false });
  }

  async unlock(key: string, password: string): Promise<boolean> {
    const f = this.get(key);
    if (!f) return false;
    const ok = await this.#readPdf({ ...f, password });
    if (ok) this.#update(key, { password, locked: false });
    return ok;
  }

  #queue: (() => Promise<void>)[] = [];
  #running = 0;

  /** Page count and a page-1 preview for PDFs, two files at a time. */
  #inspect(f: WorkFile) {
    if (f.kind !== "pdf") return;
    this.#queue.push(async () => {
      if (this.get(f.key)?.bytes === f.bytes) await this.#readPdf(f);
    });
    this.#pump();
  }

  #pump() {
    while (this.#running < 2 && this.#queue.length) {
      const job = this.#queue.shift()!;
      this.#running++;
      void job().finally(() => {
        this.#running--;
        this.#pump();
      });
    }
  }

  async #readPdf(f: WorkFile): Promise<boolean> {
    try {
      const info = await coreReport<{ pages: number; security: { needsPassword: boolean } }>("info", f.bytes, { password: f.password });
      if (info.pages === 0 && info.security.needsPassword) {
        this.#update(f.key, { locked: true });
        return false;
      }
      this.#update(f.key, { pages: info.pages, locked: false });
      const r = await runJob("pdf2img", [f.bytes], { format: "jpeg", dpi: 36, quality: 70, pages: "1", password: f.password }, ["render"], () => {});
      const page = r.files[0];
      const cur = this.get(f.key);
      if (page && cur && cur.bytes === f.bytes) {
        if (cur.thumb) URL.revokeObjectURL(cur.thumb);
        this.#update(f.key, { thumb: URL.createObjectURL(new Blob([page.bytes as BlobPart], { type: "image/jpeg" })) });
      }
      return true;
    } catch (e) {
      if (e instanceof EngineError && (e.code === "password_required" || e.code === "wrong_password")) {
        this.#update(f.key, { locked: true });
        return false;
      }
      if (e instanceof EngineError && e.code === "invalid_pdf") this.#update(f.key, { problem: `This PDF looks damaged: ${e.message}. Try Repair PDF.` });
      return true;
    }
  }
}

function release(f: WorkFile) {
  if (f.thumb) URL.revokeObjectURL(f.thumb);
  if (f.previous?.thumb && f.previous.thumb !== f.thumb) URL.revokeObjectURL(f.previous.thumb);
}

export const workspace = new Workspace();

export function useWorkspace(): WorkFile[] {
  return useSyncExternalStore(
    (fn) => workspace.subscribe(fn),
    () => workspace.files,
  );
}

/** Read dropped items, walking into folders (one level of browser API, any depth). */
export async function filesFromDrop(dt: DataTransfer): Promise<File[]> {
  const entries = [...dt.items].map((i) => i.webkitGetAsEntry?.()).filter((e): e is FileSystemEntry => Boolean(e));
  if (!entries.length || entries.every((e) => e.isFile)) return [...dt.files];
  const out: File[] = [];
  const walk = async (entry: FileSystemEntry, depth: number): Promise<void> => {
    if (entry.isFile) {
      out.push(await new Promise<File>((res, rej) => (entry as FileSystemFileEntry).file(res, rej)));
    } else if (entry.isDirectory && depth < 8) {
      const reader = (entry as FileSystemDirectoryEntry).createReader();
      // readEntries returns at most 100 entries per call.
      for (;;) {
        const batch = await new Promise<FileSystemEntry[]>((res, rej) => reader.readEntries(res, rej));
        if (!batch.length) break;
        for (const e of batch.sort((a, b) => a.name.localeCompare(b.name))) {
          if (!e.name.startsWith(".")) await walk(e, depth + 1);
        }
      }
    }
  };
  for (const e of entries) await walk(e, 0);
  return out;
}

export async function readFiles(list: Iterable<File>): Promise<{ name: string; bytes: Uint8Array }[]> {
  return Promise.all(
    [...list].map(async (f) => {
      const name = f.name || "pasted";
      const bytes = new Uint8Array(await f.arrayBuffer());
      // HEIC (Safari) and AVIF become JPEG where this browser decodes them.
      const jpeg = isoPhoto(bytes) ? await redrawAsJpeg(f) : null;
      return jpeg ? { name: name.replace(/(\.[^.]*)?$/, ".jpg"), bytes: jpeg } : { name, bytes };
    }),
  );
}
