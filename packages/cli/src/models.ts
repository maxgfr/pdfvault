// The CLI's OCR models live in a cache folder, not in the executable: they
// are downloaded on first use (or with `pdfvault models pull`) and deleted
// with `pdfvault models rm`.
import { mkdir, rename, rm, stat } from "node:fs/promises";
import { homedir } from "node:os";
import { join } from "node:path";
import { MODEL_PACKS, sha256Hex, type ModelPackId } from "@pdfvault/engine";

/** $PDFVAULT_MODELS, else the user's cache folder. */
export function modelDir(): string {
  const env = process.env;
  if (env.PDFVAULT_MODELS) return env.PDFVAULT_MODELS;
  if (process.platform === "win32" && env.LOCALAPPDATA) return join(env.LOCALAPPDATA, "pdfvault", "models");
  return join(env.XDG_CACHE_HOME || join(homedir(), ".cache"), "pdfvault", "models");
}

/** Bytes of the pack's files on disk, and whether all of them are there. */
export async function installed(id: ModelPackId): Promise<{ complete: boolean; bytes: number }> {
  let bytes = 0;
  let complete = true;
  for (const f of MODEL_PACKS[id].files) {
    const s = await stat(join(modelDir(), f.name)).catch(() => null);
    if (s?.size === f.bytes) bytes += s.size;
    else complete = false;
  }
  return { complete, bytes };
}

/** Download the files the pack is missing, each checked against its SHA-256. */
export async function pull(id: ModelPackId, progress?: (done: number, total: number) => void): Promise<void> {
  const dir = modelDir();
  await mkdir(dir, { recursive: true });
  const files = MODEL_PACKS[id].files;
  const total = files.reduce((n, f) => n + f.bytes, 0);
  let done = 0;
  for (const f of files) {
    const path = join(dir, f.name);
    if ((await stat(path).catch(() => null))?.size === f.bytes && (await sha256Hex(await Bun.file(path).bytes())) === f.sha256) {
      done += f.bytes;
      continue;
    }
    const res = await fetch(f.url);
    if (!res.ok || !res.body) throw new Error(`${f.name}: HTTP ${res.status}`);
    const chunks: Uint8Array[] = [];
    for await (const chunk of res.body) {
      chunks.push(chunk);
      done += chunk.length;
      progress?.(done, total);
    }
    const bytes = new Uint8Array(await new Blob(chunks as BlobPart[]).arrayBuffer());
    const actual = await sha256Hex(bytes);
    if (actual !== f.sha256) throw new Error(`${f.name}: checksum ${actual} does not match ${f.sha256}`);
    // Written aside, then renamed: an interrupted download never looks installed.
    await Bun.write(`${path}.part`, bytes);
    await rename(`${path}.part`, path);
  }
}

/** Delete the pack's files; returns the bytes freed. */
export async function remove(id: ModelPackId): Promise<number> {
  const { bytes } = await installed(id);
  for (const f of MODEL_PACKS[id].files) {
    await rm(join(modelDir(), f.name), { force: true });
    await rm(join(modelDir(), `${f.name}.part`), { force: true });
  }
  return bytes;
}
