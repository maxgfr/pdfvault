// GLM-OCR through Transformers.js, shared by the CLI (onnxruntime-node, CPU)
// and the site's worker (WebGPU). The caller loads Transformers.js, points it
// at the downloaded files and forbids it the network; this module only reads
// GLM_DIR through it.
import { GLM_DIR } from "./models.ts";

/** Page pixels in, the page as Markdown out (no word positions). */
export interface GlmRecognizer {
  recognize(rgba: Uint8Array, width: number, height: number): Promise<string>;
}

interface TokenIds {
  dims: number[];
  slice(...ranges: ((number | null)[] | null)[]): TokenIds;
}

interface Processor {
  (text: string, image: unknown): Promise<{ input_ids: TokenIds }>;
  apply_chat_template(messages: object[], options: { add_generation_prompt: boolean }): string;
  batch_decode(ids: TokenIds, options: { skip_special_tokens: boolean }): string[];
}

interface Model {
  generate(options: object): Promise<unknown>;
}

/** The part of @huggingface/transformers used here. */
export interface Transformers {
  AutoProcessor: { from_pretrained(id: string): Promise<unknown> };
  GlmOcrForConditionalGeneration: { from_pretrained(id: string, options: object): Promise<unknown> };
  RawImage: new (data: Uint8ClampedArray, width: number, height: number, channels: 3) => unknown;
}

/** GLM-OCR's vision encoder works on 28-pixel tiles, at least 4 × 4 of them. */
const TILE = 28;
const MIN_SIDE = 4 * TILE;
/** A dense page is a few thousand tokens; the cap stops a model that loops. */
const MAX_TOKENS = 4096;

/**
 * Load GLM-OCR (q4) on `device`. Pages are padded with white to whole tiles
 * (and its minimum size), so the processor never resizes them: in Node that
 * would need sharp. Callers keep pages under its maximum (9.6 MP).
 */
export async function glmRecognizer(tf: Transformers, device: "cpu" | "webgpu"): Promise<GlmRecognizer> {
  const processor = (await tf.AutoProcessor.from_pretrained(GLM_DIR)) as Processor;
  const model = (await tf.GlmOcrForConditionalGeneration.from_pretrained(GLM_DIR, {
    device,
    dtype: "q4",
    // The repository's config only declares external data for fp32 and fp16.
    use_external_data_format: { "vision_encoder_q4.onnx": 1, "embed_tokens_q4.onnx": 1, "decoder_model_merged_q4.onnx": 1 },
  })) as Model;
  const prompt = processor.apply_chat_template([{ role: "user", content: [{ type: "image" }, { type: "text", text: "Text Recognition:" }] }], {
    add_generation_prompt: true,
  });
  return {
    async recognize(rgba, width, height) {
      const w = Math.max(MIN_SIDE, Math.ceil(width / TILE) * TILE);
      const h = Math.max(MIN_SIDE, Math.ceil(height / TILE) * TILE);
      const rgb = new Uint8ClampedArray(w * h * 3).fill(255);
      for (let y = 0; y < height; y++) {
        for (let x = 0; x < width; x++) {
          const i = (y * width + x) * 4;
          const o = (y * w + x) * 3;
          rgb[o] = rgba[i]!;
          rgb[o + 1] = rgba[i + 1]!;
          rgb[o + 2] = rgba[i + 2]!;
        }
      }
      const inputs = await processor(prompt, new tf.RawImage(rgb, w, h, 3));
      const out = (await model.generate({ ...inputs, max_new_tokens: MAX_TOKENS, do_sample: false })) as TokenIds;
      const text = processor.batch_decode(out.slice(null, [inputs.input_ids.dims[1]!, null]), { skip_special_tokens: true })[0] ?? "";
      return text.trim();
    },
  };
}
