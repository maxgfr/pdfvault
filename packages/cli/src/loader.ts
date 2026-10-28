// Binaries the CLI ships with. `type: "file"` embeds them in the compiled
// executable and copies them next to the npm bundle. The OCR models are not
// among them: they are read from the model folder (models.ts).
import coreWasm from "../../engine/wasm/core/core_bg.wasm" with { type: "file" };
import ocrWasm from "../../engine/wasm/ocr/ocr_bg.wasm" with { type: "file" };
import renderWasm from "../../engine/wasm/render/render_bg.wasm" with { type: "file" };
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
import { MODEL_PACKS, modelMissing, packOf, type FontFile, type Loader } from "@pdfvault/engine";
import { modelDir } from "./models.ts";
import { systemFont } from "./system-fonts.ts";

// The npm bundle gets paths relative to itself, not to the working directory.
const at = (path: string) => (path.startsWith(".") ? join(import.meta.dir, path) : path);

// wasm-pack's own .d.ts types these imports as modules; with `type: "file"` they are paths.
const path = (p: unknown) => p as string;
const wasm = { core: path(coreWasm), render: path(renderWasm), ocr: path(ocrWasm) };
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
  async model(name) {
    const file = Bun.file(join(modelDir(), name));
    const id = packOf(name);
    const expected = MODEL_PACKS[id].files.find((f) => f.name === name)!.bytes;
    if (!(await file.exists()) || file.size !== expected) throw modelMissing(id, `run \`pdfvault models pull ${id}\`, or pass --download`);
    return file.bytes();
  },
  font: async (name) => new Uint8Array(await Bun.file(at(fonts[name])).arrayBuffer()),
  systemFont,
};
