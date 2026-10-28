// OCR models on this device. They are downloaded from this site only after
// the user agrees, checked against their pinned SHA-256, and kept in a cache
// of their own that app updates leave alone (sw.ts). About lists and deletes
// them. Shared by the page (download, delete) and the worker (read).
import { MODEL_PACKS, megabytes, modelMissing, packBytes, packOf, sha256Hex, type ModelFile, type ModelName, type ModelPackId } from "@pdfvault/engine";

export const MODEL_CACHE = "pdfvault-models";

const url = (name: string) => new URL(`./models/${name}`, location.href).href;

/** A model file from the cache; `model_missing` when it was never downloaded (or was deleted). */
export async function readModel(name: ModelName): Promise<Uint8Array> {
  const hit = await (await caches.open(MODEL_CACHE)).match(url(name));
  if (!hit) throw modelMissing(packOf(name));
  return new Uint8Array(await hit.arrayBuffer());
}

export async function isInstalled(id: ModelPackId): Promise<boolean> {
  const cache = await caches.open(MODEL_CACHE);
  const hits = await Promise.all(MODEL_PACKS[id].files.map((f: ModelFile) => cache.match(url(f.name))));
  return hits.every(Boolean);
}

/** Fetch the pack's missing files from this site, verify and keep them. */
export async function downloadPack(id: ModelPackId, onProgress?: (fraction: number) => void): Promise<void> {
  // Ask the browser not to evict the models under storage pressure.
  await navigator.storage?.persist?.().catch(() => false);
  const cache = await caches.open(MODEL_CACHE);
  const total = packBytes(id);
  // Private windows, for one, keep little: say so before downloading.
  const room = await navigator.storage?.estimate?.().catch(() => undefined);
  if (room?.quota !== undefined && room.quota - (room.usage ?? 0) < total) {
    throw new Error(`this browser only lets the site keep ${megabytes(room.quota - (room.usage ?? 0))} more, and the model needs ${megabytes(total)}; a private window has less room than a normal one`);
  }
  let done = 0;
  let shown = -1;
  // Once per percent, not once per network chunk.
  const report = () => {
    const percent = Math.floor((done / total) * 100);
    if (percent !== shown) onProgress?.((shown = percent) / 100);
  };
  for (const f of MODEL_PACKS[id].files as readonly ModelFile[]) {
    if (await cache.match(url(f.name))) {
      done += f.bytes;
      continue;
    }
    const res = await fetch(url(f.name), { cache: "no-store" });
    if (!res.ok || !res.body) throw new Error(`could not download ${f.name} (${res.status})`);
    // Straight into one buffer of the pinned size, the only copy held for the checksum.
    const bytes = new Uint8Array(f.bytes);
    let at = 0;
    const reader = res.body.getReader();
    for (;;) {
      const { done: end, value } = await reader.read();
      if (end) break;
      if (at + value.length > f.bytes) throw new Error(`${f.name} is larger than expected; try again`);
      bytes.set(value, at);
      at += value.length;
      done += value.length;
      report();
    }
    if (at !== f.bytes || (await sha256Hex(bytes)) !== f.sha256) throw new Error(`${f.name} did not match its checksum; try again`);
    await cache.put(url(f.name), new Response(bytes, { headers: { "content-type": "application/octet-stream" } })).catch((e: unknown) => {
      throw new Error(`could not keep ${f.name} on this device (${e instanceof Error ? e.message : String(e)}); is the disk full?`);
    });
  }
  onProgress?.(1);
}

export async function removePack(id: ModelPackId): Promise<void> {
  const cache = await caches.open(MODEL_CACHE);
  await Promise.all(MODEL_PACKS[id].files.map((f: ModelFile) => cache.delete(url(f.name))));
}
