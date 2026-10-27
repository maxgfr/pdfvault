// GLM-OCR in the job worker, on WebGPU. Loaded only for GLM jobs: the worker
// imports this chunk, and with it Transformers.js, on demand. Transformers.js
// reads the model only from the files the user downloaded (models.ts), runs
// ONNX Runtime from this site's ort/ folder, and is denied the network.
import { env, AutoProcessor, GlmOcrForConditionalGeneration, RawImage } from "@huggingface/transformers";
import type { GlmRecognizer, Transformers, glmRecognizer } from "@pdfvault/engine";
import type { glmFiles } from "./models.ts";

/** The worker hands over what it already holds, so this chunk is only Transformers.js. */
export function loadGlm(recognizer: typeof glmRecognizer, files: typeof glmFiles): Promise<GlmRecognizer> {
  env.allowRemoteModels = false;
  // Lookups must be enabled to consult the cache below; any miss is refused by `fetch`.
  env.allowLocalModels = true;
  env.useBrowserCache = false;
  env.useCustomCache = true;
  env.customCache = files;
  env.useWasmCache = false;
  env.fetch = async () => new Response(null, { status: 404 });
  const ort = env.backends.onnx.wasm!;
  const base = new URL("./ort/", self.location.href).href;
  ort.wasmPaths = { mjs: `${base}ort-wasm-simd-threaded.asyncify.mjs`, wasm: `${base}ort-wasm-simd-threaded.asyncify.wasm` };
  ort.proxy = false;
  return recognizer({ AutoProcessor, GlmOcrForConditionalGeneration, RawImage } as unknown as Transformers, "webgpu");
}
