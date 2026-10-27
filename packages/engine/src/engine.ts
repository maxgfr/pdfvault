import initCore, * as core from "../wasm/core/core.js";
import initOcr, * as ocr from "../wasm/ocr/ocr.js";
import initRender, * as render from "../wasm/render/render.js";
import { EngineError, toEngineError } from "./errors.ts";
import { BUNDLED_FONTS, type FontFile, type FontKey } from "./fonts.ts";
import type { GlmRecognizer } from "./glm.ts";
import type { ModelName } from "./models.ts";

export type ModuleName = "core" | "render" | "ocr";
export type WasmSource = BufferSource | WebAssembly.Module | Response;

/**
 * Where the engine gets its binaries: files under Bun, same-origin fetches in
 * the browser. `model` throws a `model_missing` EngineError for a model that
 * has not been downloaded.
 */
export interface Loader {
  wasm(name: ModuleName): Promise<WasmSource>;
  model(name: ModelName): Promise<Uint8Array>;
  font(name: FontFile): Promise<Uint8Array>;
  /** A font installed on this computer with a face of this PostScript name (the CLI only). */
  systemFont?(postscriptName: string): Promise<Uint8Array | null>;
  /**
   * GLM-OCR, where this platform can run it. Throws `model_missing` (pack
   * `glm`) when it is not downloaded, `unsupported` when it cannot run here.
   */
  glm?(): Promise<GlmRecognizer>;
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
  #fonts: Partial<Record<FontKey, Promise<Uint8Array>>> = {};
  #glm: Promise<GlmRecognizer> | undefined;

  constructor(loader: Loader) {
    this.#loader = loader;
  }

  #load(name: ModuleName): Promise<void> {
    const init = { core: initCore, render: initRender, ocr: initOcr }[name];
    this.#ready[name] ??= (async () => {
      await init({ module_or_path: await this.#loader.wasm(name) });
      if (name === "ocr") {
        const [det, rec, config] = await Promise.all([
          this.#loader.model("ppocrv6-small-det.onnx"),
          this.#loader.model("ppocrv6-small-rec.onnx"),
          this.#loader.model("ppocrv6-small-rec.yml"),
        ]);
        try {
          ocr.init(det, rec, new TextDecoder().decode(config));
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

  /** Read a page with GLM-OCR (loaded on first use): its text as Markdown, without positions. */
  async glm(rgba: Uint8Array, width: number, height: number): Promise<string> {
    const load = this.#loader.glm;
    if (!load) throw new EngineError("unsupported", "GLM-OCR does not run here; use the default text recognition");
    this.#glm ??= load().catch((e: unknown) => {
      this.#glm = undefined;
      throw e instanceof EngineError ? e : new EngineError("internal", `could not load GLM-OCR: ${String(e)}`);
    });
    const glm = await this.#glm;
    try {
      return await glm.recognize(rgba, width, height);
    } catch (e) {
      throw toEngineError(e);
    }
  }

  /** One of the bundled fonts, loaded once. */
  font(key: FontKey): Promise<Uint8Array> {
    this.#fonts[key] ??= this.#loader.font(BUNDLED_FONTS[key]).catch((e: unknown) => {
      delete this.#fonts[key];
      throw new EngineError("internal", `could not load the font ${BUNDLED_FONTS[key]}: ${String(e)}`);
    });
    return this.#fonts[key];
  }

  /** An installed font with a face of this PostScript name, when the loader can look. */
  async systemFont(postscriptName: string): Promise<Uint8Array | null> {
    try {
      return (await this.#loader.systemFont?.(postscriptName)) ?? null;
    } catch {
      return null;
    }
  }

  /** Whether a module is already loaded (the site shows a hint before heavy loads). */
  loaded(name: ModuleName): boolean {
    return this.#ready[name] !== undefined;
  }
}
