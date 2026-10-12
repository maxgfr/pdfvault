// Binaries the CLI ships with. `type: "file"` embeds them in the compiled
// executable and copies them next to the npm bundle; nothing is downloaded.
import coreWasm from "../../engine/wasm/core/core_bg.wasm" with { type: "file" };
import ocrWasm from "../../engine/wasm/ocr/ocr_bg.wasm" with { type: "file" };
import renderWasm from "../../engine/wasm/render/render_bg.wasm" with { type: "file" };
import detection from "../../../models/text-detection.rten" with { type: "file" };
import recognition from "../../../models/text-recognition.rten" with { type: "file" };
import { join } from "node:path";
import type { Loader } from "@pdfvault/engine";

// The npm bundle gets paths relative to itself, not to the working directory.
const at = (path: string) => (path.startsWith(".") ? join(import.meta.dir, path) : path);

// wasm-pack's own .d.ts types these imports as modules; with `type: "file"` they are paths.
const path = (p: unknown) => p as string;
const wasm = { core: path(coreWasm), render: path(renderWasm), ocr: path(ocrWasm) };
const models = { "text-detection.rten": path(detection), "text-recognition.rten": path(recognition) };

export const loader: Loader = {
  wasm: (name) => Bun.file(at(wasm[name])).arrayBuffer(),
  model: async (name) => new Uint8Array(await Bun.file(at(models[name])).arrayBuffer()),
};
