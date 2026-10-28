// The CLI's OCR models live in a cache folder, not in the executable: they
// are downloaded on first use (or with `pdfvault models pull`) and deleted
// with `pdfvault models rm`.
import { mkdir, rename, rm, stat } from "node:fs/promises";
import { homedir } from "node:os";
import { join } from "node:path";
import { MODEL_PACKS, packBytes, type ModelPackId } from "@pdfvault/engine";

/** $PDFVAULT_MODELS, else the user's cache folder. */
export function modelDir(): string {
  const env = process.env;
  if (env.PDFVAULT_MODELS) return env.PDFVAULT_MODELS;
  if (process.platform === "win32" && env.LOCALAPPDATA) return join(env.LOCALAPPDATA, "pdfvault", "models");
  return join(env.XDG_CACHE_HOME || join(homedir(), ".cache"), "pdfvault", "models");
}

/** Bytes of the pack's files on disk, and whether all of them are there at their pinned size. */
export async function installed(id: ModelPackId): Promise<{ complete: boolean; bytes: number }> {
  const sizes = await Promise.all(MODEL_PACKS[id].files.map((f) => stat(join(modelDir(), f.name)).then((s) => (s.size === f.bytes ? s.size : 0), () => 0)));
  return { complete: sizes.every((s) => s > 0), bytes: sizes.reduce((n, s) => n + s, 0) };
}

/**
 * Download the files the pack is missing, each checked against its SHA-256.
 * A file already there at its pinned size was checked when it was downloaded
 * (only checked files are renamed into place).
 */
export async function pull(id: ModelPackId, progress?: (done: number, total: number) => void): Promise<void> {
  const dir = modelDir();
  await mkdir(dir, { recursive: true });
  const total = packBytes(id);
  let done = 0;
  let shown = -1;
  // Once per percent, not once per network chunk.
  const report = () => {
    const percent = Math.floor((done / total) * 100);
    if (percent !== shown) progress?.(((shown = percent) * total) / 100, total);
  };
  for (const f of MODEL_PACKS[id].files) {
    const path = join(dir, f.name);
    if ((await stat(path).catch(() => null))?.size === f.bytes) {
      done += f.bytes;
      continue;
    }
    const res = await fetch(f.url);
    if (!res.ok || !res.body) throw new Error(`${f.name}: HTTP ${res.status}`);
    // Streamed to disk aside, then renamed: an interrupted download never looks installed.
    const hash = new Bun.CryptoHasher("sha256");
    const part = Bun.file(`${path}.part`).writer();
    for await (const chunk of res.body) {
      hash.update(chunk);
      part.write(chunk);
      done += chunk.length;
      report();
    }
    await part.end();
    const actual = hash.digest("hex");
    if (actual !== f.sha256) {
      await rm(`${path}.part`, { force: true });
      throw new Error(`${f.name}: checksum ${actual} does not match ${f.sha256}`);
    }
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
