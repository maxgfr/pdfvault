// The CLI's OCR models live in a cache folder, not in the executable: they
// are downloaded on first use (or with `pdfvault models pull`) and deleted
// with `pdfvault models rm`.
import { mkdir, readdir, rename, rm, stat } from "node:fs/promises";
import { homedir } from "node:os";
import { dirname, join } from "node:path";
import { MODEL_PACKS, packBytes, type ModelPackId } from "@pdfvault/engine";
import lockfile from "../glm-runtime/bun.lock" with { type: "text" };
import manifest from "../glm-runtime/package.json" with { type: "text" };

/** $PDFVAULT_MODELS, else the user's cache folder. */
export function modelDir(): string {
  const env = process.env;
  if (env.PDFVAULT_MODELS) return env.PDFVAULT_MODELS;
  if (process.platform === "win32" && env.LOCALAPPDATA) return join(env.LOCALAPPDATA, "pdfvault", "models");
  return join(env.XDG_CACHE_HOME || join(homedir(), ".cache"), "pdfvault", "models");
}

// GLM-OCR also needs Transformers.js and ONNX Runtime, which cannot be
// compiled into the executable (native addons). They are installed next to
// the model, from the pinned glm-runtime/bun.lock, and deleted with it.
// sharp and onnxruntime-web are stubbed out: pages arrive as raw pixels, and
// the Node build of Transformers.js carries its own copy of the web runtime.
const STUBS: Record<string, string> = {
  "stub/sharp/package.json": '{ "name": "sharp", "version": "0.0.0", "type": "module", "main": "index.js" }\n',
  "stub/sharp/index.js": 'export default function sharp() {\n  throw new Error("sharp is not installed: pdfvault hands GLM-OCR raw pixels");\n}\n',
  "stub/onnxruntime-web/package.json": '{ "name": "onnxruntime-web", "version": "0.0.0", "type": "module", "main": "index.js" }\n',
  "stub/onnxruntime-web/index.js": "export {};\n",
};
/** Written last: a runtime without it is an interrupted install. */
const DONE = ".installed";

export const runtimeDir = () => join(modelDir(), "glm-runtime");

/** The packs that need code besides their files: what it downloads from npm, and how to install it. */
const RUNTIMES: Partial<Record<ModelPackId, { bytes: number; dir: () => string; install: () => Promise<void> }>> = {
  // onnxruntime-node is most of it.
  glm: { bytes: 117_000_000, dir: runtimeDir, install: installRuntime },
};

export const hasRuntime = (id: ModelPackId) => RUNTIMES[id] !== undefined;

/** What getting a pack costs: its files, plus its runtime. */
export function downloadBytes(id: ModelPackId): number {
  return packBytes(id) + (RUNTIMES[id]?.bytes ?? 0);
}

async function du(path: string): Promise<number> {
  const s = await stat(path).catch(() => null);
  if (!s?.isDirectory()) return s?.size ?? 0;
  const sizes = await Promise.all((await readdir(path)).map((e) => du(join(path, e))));
  return sizes.reduce((n, b) => n + b, 0);
}

/**
 * Install the runtime with Bun from the pinned lockfile. A compiled pdfvault
 * is a Bun executable: BUN_BE_BUN=1 makes it run as `bun` itself.
 */
async function installRuntime(): Promise<void> {
  const dir = runtimeDir();
  await rm(dir, { recursive: true, force: true });
  for (const [name, text] of Object.entries({ "package.json": manifest as unknown as string, "bun.lock": lockfile as unknown as string, ...STUBS })) {
    await mkdir(dirname(join(dir, name)), { recursive: true });
    await Bun.write(join(dir, name), text);
  }
  let proc;
  try {
    proc = Bun.spawn([process.execPath, "install", "--frozen-lockfile", "--production", "--ignore-scripts", "--no-progress", "--no-summary"], {
      cwd: dir,
      // Bun's package cache goes inside the folder, so `models rm glm` deletes everything.
      env: { ...process.env, BUN_BE_BUN: "1", BUN_INSTALL_CACHE_DIR: join(dir, ".cache") },
      stdout: "ignore",
      stderr: "pipe",
    });
  } catch (e) {
    throw new Error(`could not start Bun to install the GLM-OCR runtime (${String(e)}); install Bun (https://bun.sh) and run pdfvault with it`);
  }
  const err = await new Response(proc.stderr).text();
  if ((await proc.exited) !== 0) throw new Error(`installing the GLM-OCR runtime failed:\n${err.trim()}`);
  await rm(join(dir, ".cache"), { recursive: true, force: true });
  // onnxruntime-node ships every platform's binaries; keep this one's.
  const bin = join(dir, "node_modules", "onnxruntime-node", "bin", "napi-v6");
  for (const os of await readdir(bin).catch(() => [])) {
    for (const arch of await readdir(join(bin, os)).catch(() => [])) {
      if (os !== process.platform || arch !== process.arch) await rm(join(bin, os, arch), { recursive: true, force: true });
    }
  }
  await Bun.write(join(dir, DONE), "");
}

/** Whether every file of the pack is there at its pinned size, and its runtime finished installing. Cheap: run before each job. */
export async function ready(id: ModelPackId): Promise<boolean> {
  const sizes = await Promise.all(MODEL_PACKS[id].files.map((f) => stat(join(modelDir(), f.name)).then((s) => s.size === f.bytes, () => false)));
  const runtime = RUNTIMES[id];
  return sizes.every(Boolean) && (!runtime || (await Bun.file(join(runtime.dir(), DONE)).exists()));
}

/** Whether the pack is complete, and the bytes it and its runtime take on disk (for `models list` and `rm`). */
export async function installed(id: ModelPackId): Promise<{ complete: boolean; bytes: number }> {
  const files = await Promise.all(MODEL_PACKS[id].files.map((f) => stat(join(modelDir(), f.name)).then((s) => (s.size === f.bytes ? s.size : 0), () => 0)));
  const runtime = RUNTIMES[id];
  const bytes = files.reduce((n, b) => n + b, 0) + (runtime ? await du(runtime.dir()) : 0);
  return { complete: await ready(id), bytes };
}

/**
 * Download the files the pack is missing, each checked against its SHA-256,
 * then its runtime. A file already there at its pinned size was checked when
 * it was downloaded (only checked files are renamed into place).
 */
export async function pull(id: ModelPackId, progress?: (done: number, total: number) => void, onRuntime?: () => void): Promise<void> {
  const dir = modelDir();
  const files = MODEL_PACKS[id].files;
  const total = packBytes(id);
  let done = 0;
  let shown = -1;
  // Once per percent, not once per network chunk.
  const report = () => {
    const percent = Math.floor((done / total) * 100);
    if (percent !== shown) progress?.((shown = percent) * total / 100, total);
  };
  for (const f of files) {
    const path = join(dir, f.name);
    await mkdir(dirname(path), { recursive: true });
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
  const runtime = RUNTIMES[id];
  if (runtime && !(await ready(id))) {
    onRuntime?.();
    await runtime.install();
  }
}

/** Delete the pack's files, the folders they sit in, and its runtime; returns the bytes freed. */
export async function remove(id: ModelPackId): Promise<number> {
  const { bytes } = await installed(id);
  // A pack in a folder of its own (GLM-OCR keeps its repository's layout) goes as one folder.
  const tops = new Set(MODEL_PACKS[id].files.map((f) => f.name.split("/")[0]!));
  for (const top of tops) await rm(join(modelDir(), top), { recursive: true, force: true });
  for (const f of MODEL_PACKS[id].files) await rm(join(modelDir(), `${f.name}.part`), { force: true });
  const runtime = RUNTIMES[id];
  if (runtime) await rm(runtime.dir(), { recursive: true, force: true });
  return bytes;
}
