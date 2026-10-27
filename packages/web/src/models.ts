// OCR models on this device. They are downloaded only after the user agrees
// (from this site, or from Hugging Face for GLM-OCR, which is too large to
// serve here), checked against their pinned SHA-256, and kept in a cache of
// their own that app updates leave alone (sw.ts). About lists and deletes
// them. Shared by the page (download, delete) and the worker (read).
import { EngineError, MODEL_PACKS, megabytes, packBytes, packOf, sha256Hex, type ModelFile, type ModelName, type ModelPackId } from "@pdfvault/engine";

export const MODEL_CACHE = "pdfvault-models";

/** Every file is cached under this site's models/<name>, wherever it was downloaded from. */
const url = (name: string) => new URL(`./models/${name}`, location.href).href;

function missing(id: ModelPackId): EngineError {
  return new EngineError("model_missing", `${MODEL_PACKS[id].title} is not on this device yet (${megabytes(packBytes(id))})`, id);
}

/** A model file from the cache; `model_missing` when it was never downloaded (or was deleted). */
export async function readModel(name: ModelName): Promise<Uint8Array> {
  const hit = await (await caches.open(MODEL_CACHE)).match(url(name));
  if (!hit) throw missing(packOf(name));
  return new Uint8Array(await hit.arrayBuffer());
}

/**
 * The downloaded GLM-OCR files as Transformers.js reads them (its
 * `customCache`): a request for a path ending in one of them gets it, any
 * other gets nothing. Nothing is ever written.
 */
export const glmFiles = {
  async match(request: string | Request): Promise<Response | undefined> {
    const key = typeof request === "string" ? request : request.url;
    const file = MODEL_PACKS.glm.files.find((f: ModelFile) => key.endsWith(`/${f.name}`));
    return file ? (await caches.open(MODEL_CACHE)).match(url(file.name)) : undefined;
  },
  async put() {},
};

export async function isInstalled(id: ModelPackId): Promise<boolean> {
  const cache = await caches.open(MODEL_CACHE);
  for (const f of MODEL_PACKS[id].files) if (!(await cache.match(url(f.name)))) return false;
  return true;
}

/** `model_missing` unless every file of the pack is on this device. */
export async function requireInstalled(id: ModelPackId): Promise<void> {
  if (!(await isInstalled(id))) throw missing(id);
}

/** Fetch the pack's missing files (from this site or upstream), verify and keep them. */
export async function downloadPack(id: ModelPackId, onProgress?: (fraction: number) => void): Promise<void> {
  // Ask the browser not to evict the models under storage pressure.
  await navigator.storage?.persist?.().catch(() => false);
  const cache = await caches.open(MODEL_CACHE);
  const pack = MODEL_PACKS[id];
  const total = packBytes(id);
  // Private windows, for one, keep little: say so before downloading hundreds of MB.
  const room = await navigator.storage?.estimate?.().catch(() => undefined);
  if (room?.quota !== undefined && room.quota - (room.usage ?? 0) < total) {
    throw new Error(`this browser only lets the site keep ${megabytes(room.quota - (room.usage ?? 0))} more, and the model needs ${megabytes(total)}; a private window has less room than a normal one`);
  }
  let done = 0;
  for (const f of pack.files as readonly ModelFile[]) {
    if (await cache.match(url(f.name))) {
      done += f.bytes;
      continue;
    }
    const res = await fetch(pack.webSource === "self" ? url(f.name) : f.url, { cache: "no-store" });
    if (!res.ok || !res.body) throw new Error(`could not download ${f.name} (${res.status})`);
    const chunks: Uint8Array[] = [];
    const reader = res.body.getReader();
    for (;;) {
      const { done: end, value } = await reader.read();
      if (end) break;
      chunks.push(value);
      done += value.length;
      onProgress?.(Math.min(done / total, 1));
    }
    // GLM-OCR's files run to hundreds of MB: one copy in memory for the checksum, the blob for the cache.
    const blob = new Blob(chunks as BlobPart[]);
    chunks.length = 0;
    if ((await sha256Hex(new Uint8Array(await blob.arrayBuffer()))) !== f.sha256) throw new Error(`${f.name} did not match its checksum; try again`);
    await cache.put(url(f.name), new Response(blob, { headers: { "content-type": "application/octet-stream" } })).catch((e: unknown) => {
      throw new Error(`could not keep ${f.name} on this device (${e instanceof Error ? e.message : String(e)}); is the disk full?`);
    });
  }
  onProgress?.(1);
}

export async function removePack(id: ModelPackId): Promise<void> {
  const cache = await caches.open(MODEL_CACHE);
  for (const f of MODEL_PACKS[id].files) await cache.delete(url(f.name));
}
