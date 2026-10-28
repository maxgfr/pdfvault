/// <reference lib="webworker" />
// One worker per job: the page stays responsive, and terminating the worker
// returns every byte of wasm memory to the browser.
import { Engine, EngineError, glmRecognizer, previewPages, toolById, withDefaults, type EditOps, type Loader, type ModelPackId, type ModuleName, type Options } from "@pdfvault/engine";
import { glmUnsupported } from "./glm-support.ts";
import { glmFiles, readModel, requireInstalled } from "./models.ts";

declare const self: DedicatedWorkerGlobalScope;

/** Core operations the page may call directly, outside the tool registry. */
export const DIRECT_OPS = ["kdf", "zip", "meta-read", "info", "form-fields", "text-runs", "scan-detect", "scan"] as const;

export interface JobRequest {
  /** A tool id, or `core:<op>` for one of DIRECT_OPS. */
  toolId: string;
  inputs: Uint8Array[];
  options: Record<string, unknown>;
  modules?: Partial<Record<ModuleName, WebAssembly.Module>>;
}

export type JobMessage =
  | { type: "progress"; done: number; total: number; label: string }
  | { type: "loading"; module: ModuleName }
  | { type: "done"; files: { suffix: string; ext: string; bytes: Uint8Array }[]; report: unknown }
  | { type: "error"; code: string; message: string; pack?: ModelPackId };

const post = (m: JobMessage, transfer: Transferable[] = []) => self.postMessage(m, transfer);

self.onmessage = async (event: MessageEvent<JobRequest>) => {
  const { toolId, inputs, options, modules = {} } = event.data;
  const loader: Loader = {
    async wasm(name) {
      // The page compiles each module once and hands it over; otherwise fetch it.
      const compiled = modules[name];
      if (compiled) return compiled;
      post({ type: "loading", module: name });
      return fetch(new URL(`./wasm/${name}.wasm`, self.location.href));
    },
    // Only what the user downloaded: a job never fetches a model by itself.
    model: readModel,
    async font(name) {
      const res = await fetch(new URL(`./fonts/${name}`, self.location.href));
      if (!res.ok) throw new EngineError("internal", `could not load ${name} (${res.status})`);
      return new Uint8Array(await res.arrayBuffer());
    },
    // The page only offers GLM-OCR where WebGPU works, so the download prompt comes first.
    async glm() {
      await requireInstalled("glm");
      const why = await glmUnsupported();
      if (why) throw new EngineError("unsupported", why);
      return (await import("./glm.ts")).loadGlm(glmRecognizer, glmFiles);
    },
  };
  try {
    if (toolId.startsWith("core:")) {
      const op = toolId.slice(5);
      if (!(DIRECT_OPS as readonly string[]).includes(op)) throw new EngineError("invalid_option", `unknown operation ${op}`);
      const out = await new Engine(loader).core(op, inputs, options);
      post({ type: "done", files: out.files, report: out.report }, out.files.map((f) => f.bytes.buffer as ArrayBuffer));
      return;
    }
    if (toolId === "preview:edit") {
      // The editor's exact preview: edits applied, touched pages rendered.
      const o = options as { ops: EditOps; pages: number[]; dpi: number; password?: string };
      const { pages, report } = await previewPages(new Engine(loader), inputs, o.ops, o.pages, o.dpi, o.password);
      const files = pages.map((p) => ({ ...p.file, suffix: `page-${p.page}` }));
      post({ type: "done", files, report }, files.map((f) => f.bytes.buffer as ArrayBuffer));
      return;
    }
    const tool = toolById.get(toolId);
    if (!tool) throw new EngineError("invalid_option", `unknown tool ${toolId}`);
    const engine = new Engine(loader);
    const result = await tool.run(engine, inputs, withDefaults(tool, options as Options), (done, total, label) =>
      post({ type: "progress", done, total, label }),
    );
    post(
      { type: "done", files: result.files, report: result.report },
      result.files.map((f) => f.bytes.buffer as ArrayBuffer),
    );
  } catch (e) {
    const err = e instanceof EngineError ? e : new EngineError("internal", e instanceof Error ? e.message : String(e));
    post({ type: "error", code: err.code, message: err.message, pack: err.pack });
  }
};
