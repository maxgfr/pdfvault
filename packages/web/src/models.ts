// OCR models on this device. They are downloaded from this site only after
// the user agrees, checked against their pinned SHA-256, and kept in a cache
// of their own that app updates leave alone (sw.ts). About lists and deletes
// them. Shared by the page (download, delete) and the worker (read).
import { EngineError, MODEL_PACKS, megabytes, packBytes, packOf, sha256Hex, type ModelName, type ModelPackId } from "@pdfvault/engine";

export const MODEL_CACHE = "pdfvault-models";

const url = (name: string) => new URL(`./models/${name}`, location.href).href;

/** A model file from the cache; `model_missing` when it was never downloaded (or was deleted). */
export async function readModel(name: ModelName): Promise<Uint8Array> {
  const hit = await (await caches.open(MODEL_CACHE)).match(url(name));
  if (!hit) {
    const id = packOf(name);
    throw new EngineError("model_missing", `${MODEL_PACKS[id].title} is not on this device yet (${megabytes(packBytes(id))})`, id);
  }
  return new Uint8Array(await hit.arrayBuffer());
}

export async function isInstalled(id: ModelPackId): Promise<boolean> {
  const cache = await caches.open(MODEL_CACHE);
  for (const f of MODEL_PACKS[id].files) if (!(await cache.match(url(f.name)))) return false;
  return true;
}

/** Fetch the pack's missing files from this site, verify and keep them. */
export async function downloadPack(id: ModelPackId, onProgress?: (fraction: number) => void): Promise<void> {
  // Ask the browser not to evict the models under storage pressure.
  await navigator.storage?.persist?.().catch(() => false);
  const cache = await caches.open(MODEL_CACHE);
  const total = packBytes(id);
  let done = 0;
  for (const f of MODEL_PACKS[id].files) {
    if (await cache.match(url(f.name))) {
      done += f.bytes;
      continue;
    }
    const res = await fetch(url(f.name), { cache: "no-store" });
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
    const bytes = new Uint8Array(await new Blob(chunks as BlobPart[]).arrayBuffer());
    if ((await sha256Hex(bytes)) !== f.sha256) throw new Error(`${f.name} did not match its checksum; try again`);
    await cache.put(url(f.name), new Response(bytes, { headers: { "content-type": "application/octet-stream" } }));
  }
  onProgress?.(1);
}

export async function removePack(id: ModelPackId): Promise<void> {
  const cache = await caches.open(MODEL_CACHE);
  for (const f of MODEL_PACKS[id].files) await cache.delete(url(f.name));
}
