// The pages of the scan in progress, kept in memory only, like the workspace:
// leaving the scanner and coming back finds them, a reload empties it.

import { useSyncExternalStore } from "react";
import { runJob } from "../runner.ts";
import { classify, isImage, isPdf, workspace } from "../workspace.ts";

/** Corners in pixels of the upright photo: top-left, top-right, bottom-right, bottom-left. */
export type Quad = [number, number][];
export type Filter = "color" | "gray" | "bw" | "original";
export type Turn = 0 | 90 | 180 | 270;

export interface ScanPage {
  key: string;
  bytes: Uint8Array;
  /** Object URL of the photo. */
  photo: string;
  width?: number;
  height?: number;
  corners?: Quad;
  /** Whether detection found a sheet, rather than a default frame. */
  found?: boolean;
  rotate: Turn;
  /** Object URL of the straightened, filtered page. */
  preview?: string;
  error?: string;
}

interface Detection {
  corners: Quad;
  found: boolean;
  width: number;
  height: number;
}

/** Previews are small: they only have to look right. */
const PREVIEW_SIDE = 900;

// Two jobs at a time: ten imported photos must not start twenty workers on a phone.
let running = 0;
const waiting: (() => void)[] = [];
async function queued<T>(job: () => Promise<T>): Promise<T> {
  if (running >= 2) await new Promise<void>((go) => waiting.push(go));
  running++;
  try {
    return await job();
  } finally {
    running--;
    waiting.shift()?.();
  }
}

type Listener = () => void;

class ScanSession {
  #pages: ScanPage[] = [];
  #filter: Filter = "color";
  #listeners = new Set<Listener>();
  /** Bumped on every change to a page, so a slow preview never overwrites a newer one. */
  #generation = new Map<string, number>();

  subscribe(fn: Listener): () => void {
    this.#listeners.add(fn);
    return () => this.#listeners.delete(fn);
  }

  get pages(): ScanPage[] {
    return this.#pages;
  }

  get filter(): Filter {
    return this.#filter;
  }

  get(key: string): ScanPage | undefined {
    return this.#pages.find((p) => p.key === key);
  }

  #set(pages: ScanPage[]) {
    this.#pages = pages;
    for (const fn of this.#listeners) fn();
  }

  #patch(key: string, patch: Partial<ScanPage>) {
    this.#set(this.#pages.map((p) => (p.key === key ? { ...p, ...patch } : p)));
  }

  setFilter(filter: Filter) {
    this.#filter = filter;
    for (const fn of this.#listeners) fn();
    for (const p of this.#pages) void this.#preview(p.key);
  }

  /** Add photos; each is then searched for a sheet in the background. */
  add(photos: Uint8Array[]): ScanPage[] {
    const added = photos.map((bytes) => ({
      key: crypto.randomUUID(),
      bytes,
      photo: URL.createObjectURL(new Blob([bytes as BlobPart])),
      rotate: 0 as Turn,
    }));
    this.#set([...this.#pages, ...added]);
    for (const p of added) void this.detect(p.key);
    return added;
  }

  /** Look for the sheet again, discarding the corners set by hand. */
  async detect(key: string) {
    const page = this.get(key);
    if (!page) return;
    try {
      const r = await queued(() => runJob("core:scan-detect", [page.bytes], {}, [], () => {}));
      const d = (r.report as { pages: Detection[] }).pages[0]!;
      if (!this.get(key)) return;
      this.#patch(key, { corners: d.corners, found: d.found, width: d.width, height: d.height, error: undefined });
      await this.#preview(key);
    } catch (e) {
      this.#patch(key, { error: e instanceof Error ? e.message : String(e) });
    }
  }

  setCorners(key: string, corners: Quad) {
    this.#patch(key, { corners });
    void this.#preview(key);
  }

  /** The whole photo, for pages shot straight on or already cropped. */
  wholePhoto(key: string) {
    const p = this.get(key);
    if (!p?.width || !p.height) return;
    this.setCorners(key, [
      [0, 0],
      [p.width, 0],
      [p.width, p.height],
      [0, p.height],
    ]);
  }

  turn(key: string) {
    const p = this.get(key);
    if (!p) return;
    this.#patch(key, { rotate: ((p.rotate + 90) % 360) as Turn });
    void this.#preview(key);
  }

  move(key: string, by: number) {
    const at = this.#pages.findIndex((p) => p.key === key);
    const to = at + by;
    if (at < 0 || to < 0 || to >= this.#pages.length) return;
    const next = [...this.#pages];
    const [item] = next.splice(at, 1);
    next.splice(to, 0, item!);
    this.#set(next);
  }

  remove(key: string) {
    const p = this.get(key);
    if (!p) return;
    this.#release(p);
    this.#generation.delete(key);
    this.#set(this.#pages.filter((x) => x.key !== key));
  }

  clear() {
    for (const p of this.#pages) this.#release(p);
    this.#generation.clear();
    this.#set([]);
  }

  #release(p: ScanPage) {
    URL.revokeObjectURL(p.photo);
    if (p.preview) URL.revokeObjectURL(p.preview);
  }

  /** What the engine needs to make the PDF exactly as previewed. */
  settings(): { corners?: Quad; rotate: Turn }[] {
    return this.#pages.map((p) => ({ corners: p.corners, rotate: p.rotate }));
  }

  async #preview(key: string) {
    const page = this.get(key);
    if (!page?.corners) return;
    const gen = (this.#generation.get(key) ?? 0) + 1;
    this.#generation.set(key, gen);
    try {
      const r = await queued(() =>
        runJob(
          "core:scan",
          [page.bytes],
          { pages: [{ corners: page.corners, rotate: page.rotate }], filter: this.#filter, maxSide: PREVIEW_SIDE, quality: 80 },
          [],
          () => {},
        ),
      );
      if (this.#generation.get(key) !== gen) return;
      const old = this.get(key)?.preview;
      if (old) URL.revokeObjectURL(old);
      this.#patch(key, { preview: URL.createObjectURL(new Blob([r.files[0]!.bytes as BlobPart])), error: undefined });
    } catch (e) {
      if (this.#generation.get(key) === gen) this.#patch(key, { error: e instanceof Error ? e.message : String(e) });
    }
  }
}

export const session = new ScanSession();

export function useScanPages(): ScanPage[] {
  return useSyncExternalStore(
    (fn) => session.subscribe(fn),
    () => session.pages,
  );
}

export function useScanFilter(): Filter {
  return useSyncExternalStore(
    (fn) => session.subscribe(fn),
    () => session.filter,
  );
}

/**
 * Photos the engine reads go in as they are. Others (HEIC on an iPhone, say)
 * are redrawn as JPEG when this browser can decode them.
 */
export async function photoBytes(file: Blob, name: string): Promise<Uint8Array> {
  const bytes = new Uint8Array(await file.arrayBuffer());
  if (isImage(bytes)) return bytes;
  try {
    const bitmap = await createImageBitmap(file);
    const canvas = new OffscreenCanvas(bitmap.width, bitmap.height);
    canvas.getContext("2d")!.drawImage(bitmap, 0, 0);
    bitmap.close();
    return new Uint8Array(await (await canvas.convertToBlob({ type: "image/jpeg", quality: 0.92 })).arrayBuffer());
  } catch {
    throw new Error(classify(name, bytes).problem ?? "This file is not a photo.");
  }
}

/** Where the service worker leaves files shared to the app (see sw.ts). */
export const INBOX = "pdfvault-inbox";

/**
 * Take the files another app shared to pdfvault: photos join the scan, PDFs
 * and anything else go to your files. The inbox is emptied right away.
 * Returns how many photos joined the scan.
 */
export async function takeShared(): Promise<number> {
  if (!("caches" in self)) return 0;
  const cache = await caches.open(INBOX);
  const photos: Uint8Array[] = [];
  const others: { name: string; bytes: Uint8Array }[] = [];
  for (const req of await cache.keys()) {
    const res = await cache.match(req);
    if (!res) continue;
    const name = decodeURIComponent(res.headers.get("x-name") ?? "shared");
    const blob = await res.blob();
    const bytes = new Uint8Array(await blob.arrayBuffer());
    if (isPdf(bytes)) others.push({ name, bytes });
    else {
      try {
        photos.push(await photoBytes(blob, name));
      } catch {
        others.push({ name, bytes });
      }
    }
  }
  await caches.delete(INBOX);
  if (others.length) workspace.add(others);
  if (photos.length) session.add(photos);
  return photos.length;
}
