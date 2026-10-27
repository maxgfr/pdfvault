import type { ModuleName } from "@pdfvault/engine";
import { EngineError } from "@pdfvault/engine";
import { scriptURL } from "./trusted.ts";
import type { JobMessage, JobRequest } from "./worker.ts";

export interface JobResult {
  files: { suffix: string; ext: string; bytes: Uint8Array }[];
  report: unknown;
}

const TIMEOUT_MS = 20 * 60 * 1000;
const compiled = new Map<ModuleName, Promise<WebAssembly.Module>>();

/** Compile each wasm module once per page load; workers reuse the result. */
function compile(name: ModuleName): Promise<WebAssembly.Module> {
  let p = compiled.get(name);
  if (!p) {
    p = WebAssembly.compileStreaming(fetch(new URL(`./wasm/${name}.wasm`, location.href)));
    p.catch(() => compiled.delete(name));
    compiled.set(name, p);
  }
  return p;
}

export function warmUp(name: ModuleName) {
  void compile(name).catch(() => {});
}

export async function runJob(
  toolId: string,
  inputs: Uint8Array[],
  options: JobRequest["options"],
  heavy: ModuleName[],
  onProgress: (label: string, fraction: number | null) => void,
  signal?: AbortSignal,
): Promise<JobResult> {
  const modules: JobRequest["modules"] = {};
  for (const name of ["core", ...heavy] as ModuleName[]) {
    onProgress(name === "core" ? "Starting the engine" : `Loading the ${name === "ocr" ? "text recognition" : "page renderer"}`, null);
    modules[name] = await compile(name);
  }
  const worker = new Worker(scriptURL("./worker.js"), { type: "module", name: `pdfvault-${toolId}` });
  return new Promise<JobResult>((resolve, reject) => {
    const finish = () => {
      clearTimeout(timer);
      worker.terminate();
      signal?.removeEventListener("abort", abort);
    };
    const abort = () => {
      finish();
      reject(new EngineError("internal", "cancelled"));
    };
    const timer = setTimeout(() => {
      finish();
      reject(new EngineError("limit", "this took more than 20 minutes and was stopped"));
    }, TIMEOUT_MS);
    signal?.addEventListener("abort", abort);
    worker.onmessage = (e: MessageEvent<JobMessage>) => {
      const m = e.data;
      if (m.type === "progress") onProgress(m.label, m.total ? m.done / m.total : null);
      else if (m.type === "loading") onProgress(`Loading ${m.module}`, null);
      else if (m.type === "done") {
        finish();
        resolve({ files: m.files, report: m.report });
      } else {
        finish();
        reject(new EngineError(m.code as EngineError["code"], m.message, m.pack));
      }
    };
    worker.onerror = (e) => {
      finish();
      reject(new EngineError("internal", e.message || "the worker crashed (out of memory?)"));
    };
    onProgress("Working", null);
    // Copies, not transfers: the page keeps its files for another run.
    worker.postMessage({ toolId, inputs: inputs.map((i) => i.slice()), options, modules } satisfies JobRequest);
  });
}

/** Argon2id in a worker: the derived bytes, to import and wipe right away. */
export async function runKdf(passphrase: string, salt: Uint8Array, params: { memoryKib: number; iterations: number }): Promise<Uint8Array> {
  const result = await runJob("core:kdf", [salt], { passphrase, ...params }, [], () => {});
  const key = result.files[0]?.bytes;
  if (!key || key.length !== 32) throw new EngineError("internal", "key derivation failed");
  return key;
}

/** Pack several outputs into one ZIP (Rust's deterministic writer). */
export async function zipFiles(files: { name: string; bytes: Uint8Array }[]): Promise<Uint8Array> {
  const r = await runJob("core:zip", files.map((f) => f.bytes), { names: files.map((f) => f.name) }, [], () => {});
  return r.files[0]!.bytes;
}

/** Report-only core calls (document info before a run, metadata prefill, form fields). */
export async function coreReport<T>(op: "info" | "meta-read" | "form-fields" | "text-runs", input: Uint8Array, options: Record<string, string | undefined> = {}): Promise<T> {
  return (await runJob(`core:${op}`, [input], options, [], () => {})).report as T;
}
