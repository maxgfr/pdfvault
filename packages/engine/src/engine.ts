import initCore, * as core from "../wasm/core/core.js";
import initOcr, * as ocr from "../wasm/ocr/ocr.js";
import initRender, * as render from "../wasm/render/render.js";
import { EngineError, toEngineError } from "./errors.ts";

export type ModuleName = "core" | "render" | "ocr";
export type ModelName = "text-detection.rten" | "text-recognition.rten";
export type WasmSource = BufferSource | WebAssembly.Module | Response;

/** Where the engine gets its binaries: files under Bun, same-origin fetches in the browser. */
export interface Loader {
  wasm(name: ModuleName): Promise<WasmSource>;
  model(name: ModelName): Promise<Uint8Array>;
}

export interface OutFile {
  suffix: string;
  ext: string;
  bytes: Uint8Array;
}

export interface RunResult<R = unknown> {
  files: OutFile[];
  report: R;
}

export interface OcrWord {
  text: string;
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface OcrLine {
  text: string;
  words: OcrWord[];
}

interface RawResult {
  files: OutFile[];
  report: string;
}

/**
 * The single gateway to the wasm modules. Render and OCR load on first use,
 * so a compression job never pays for the rasterizer or the models.
 */
export class Engine {
  #loader: Loader;
  #ready: Partial<Record<ModuleName, Promise<void>>> = {};

  constructor(loader: Loader) {
    this.#loader = loader;
  }

  #load(name: ModuleName): Promise<void> {
    const init = { core: initCore, render: initRender, ocr: initOcr }[name];
    this.#ready[name] ??= (async () => {
      await init({ module_or_path: await this.#loader.wasm(name) });
      if (name === "ocr") {
        const [det, rec] = await Promise.all([
          this.#loader.model("text-detection.rten"),
          this.#loader.model("text-recognition.rten"),
        ]);
        try {
          ocr.init(det, rec);
        } catch (e) {
          throw toEngineError(e);
        }
      }
    })().catch((e: unknown) => {
      delete this.#ready[name];
      throw e instanceof EngineError ? e : new EngineError("internal", `could not load the ${name} engine: ${String(e)}`);
    });
    return this.#ready[name];
  }

  async #call<R>(mod: typeof core | typeof render, name: ModuleName, op: string, inputs: Uint8Array[], options: object): Promise<RunResult<R>> {
    await this.#load(name);
    let raw: RawResult;
    try {
      raw = mod.run(op, inputs, JSON.stringify(options)) as RawResult;
    } catch (e) {
      throw toEngineError(e);
    }
    return { files: raw.files, report: (raw.report ? JSON.parse(raw.report) : null) as R };
  }

  /** Run a pdfvault-core operation. */
  core<R = unknown>(op: string, inputs: Uint8Array[], options: object = {}): Promise<RunResult<R>> {
    return this.#call<R>(core, "core", op, inputs, options);
  }

  /** Run a pdfvault-render operation. */
  render<R = unknown>(op: string, inputs: Uint8Array[], options: object = {}): Promise<RunResult<R>> {
    return this.#call<R>(render, "render", op, inputs, options);
  }

  /** Recognize text in raw pixels (RGBA, RGB or gray). */
  async ocr(pixels: Uint8Array, width: number, height: number): Promise<OcrLine[]> {
    await this.#load("ocr");
    try {
      return JSON.parse(ocr.recognize(pixels, width, height)) as OcrLine[];
    } catch (e) {
      throw toEngineError(e);
    }
  }

  /** Whether a module is already loaded (the site shows a hint before heavy loads). */
  loaded(name: ModuleName): boolean {
    return this.#ready[name] !== undefined;
  }
}
