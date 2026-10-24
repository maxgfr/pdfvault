// Binaries the CLI ships with. `type: "file"` embeds them in the compiled
// executable and copies them next to the npm bundle; nothing is downloaded.
import coreWasm from "../../engine/wasm/core/core_bg.wasm" with { type: "file" };
import ocrWasm from "../../engine/wasm/ocr/ocr_bg.wasm" with { type: "file" };
import renderWasm from "../../engine/wasm/render/render_bg.wasm" with { type: "file" };
import detection from "../../../models/text-detection.rten" with { type: "file" };
import recognition from "../../../models/text-recognition.rten" with { type: "file" };
import monoBold from "../../../models/fonts/LiberationMono-Bold.ttf" with { type: "file" };
import monoBoldItalic from "../../../models/fonts/LiberationMono-BoldItalic.ttf" with { type: "file" };
import monoItalic from "../../../models/fonts/LiberationMono-Italic.ttf" with { type: "file" };
import monoRegular from "../../../models/fonts/LiberationMono-Regular.ttf" with { type: "file" };
import sansBold from "../../../models/fonts/LiberationSans-Bold.ttf" with { type: "file" };
import sansBoldItalic from "../../../models/fonts/LiberationSans-BoldItalic.ttf" with { type: "file" };
import sansItalic from "../../../models/fonts/LiberationSans-Italic.ttf" with { type: "file" };
import sansRegular from "../../../models/fonts/LiberationSans-Regular.ttf" with { type: "file" };
import serifBold from "../../../models/fonts/LiberationSerif-Bold.ttf" with { type: "file" };
import serifBoldItalic from "../../../models/fonts/LiberationSerif-BoldItalic.ttf" with { type: "file" };
import serifItalic from "../../../models/fonts/LiberationSerif-Italic.ttf" with { type: "file" };
import serifRegular from "../../../models/fonts/LiberationSerif-Regular.ttf" with { type: "file" };
import { join } from "node:path";
import type { FontFile, Loader } from "@pdfvault/engine";
import { systemFont } from "./system-fonts.ts";

// The npm bundle gets paths relative to itself, not to the working directory.
const at = (path: string) => (path.startsWith(".") ? join(import.meta.dir, path) : path);

// wasm-pack's own .d.ts types these imports as modules; with `type: "file"` they are paths.
const path = (p: unknown) => p as string;
const wasm = { core: path(coreWasm), render: path(renderWasm), ocr: path(ocrWasm) };
const models = { "text-detection.rten": path(detection), "text-recognition.rten": path(recognition) };
const fonts: Record<FontFile, string> = {
  "LiberationSans-Regular.ttf": path(sansRegular),
  "LiberationSans-Bold.ttf": path(sansBold),
  "LiberationSans-Italic.ttf": path(sansItalic),
  "LiberationSans-BoldItalic.ttf": path(sansBoldItalic),
  "LiberationSerif-Regular.ttf": path(serifRegular),
  "LiberationSerif-Bold.ttf": path(serifBold),
  "LiberationSerif-Italic.ttf": path(serifItalic),
  "LiberationSerif-BoldItalic.ttf": path(serifBoldItalic),
  "LiberationMono-Regular.ttf": path(monoRegular),
  "LiberationMono-Bold.ttf": path(monoBold),
  "LiberationMono-Italic.ttf": path(monoItalic),
  "LiberationMono-BoldItalic.ttf": path(monoBoldItalic),
};

export const loader: Loader = {
  wasm: (name) => Bun.file(at(wasm[name])).arrayBuffer(),
  model: async (name) => new Uint8Array(await Bun.file(at(models[name])).arrayBuffer()),
  font: async (name) => new Uint8Array(await Bun.file(at(fonts[name])).arrayBuffer()),
  systemFont,
};
