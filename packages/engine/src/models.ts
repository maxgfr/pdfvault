// The OCR models. Nothing ships with the engine: a model is downloaded the
// first time it is needed (after the user agrees), kept on the device, and
// can be deleted at any time. Every file is pinned to an upstream revision
// and accepted only with its SHA-256.

export interface ModelFile {
  /** The name it is stored under (the site serves it as models/<name>). */
  name: string;
  /** Where it comes from, at a fixed revision. */
  url: string;
  sha256: string;
  bytes: number;
}

export interface ModelPack {
  id: string;
  title: string;
  license: string;
  /**
   * Where the site downloads it from: "self" serves the files from its own
   * origin (models/<name>); "upstream" fetches each file's pinned `url`.
   */
  webSource: "self" | "upstream";
  files: readonly ModelFile[];
}

const PADDLE_DET = "https://huggingface.co/PaddlePaddle/PP-OCRv6_small_det_onnx/resolve/28fe5895c24fd108c19eb3e8479f4ab385fbfc62";
const PADDLE_REC = "https://huggingface.co/PaddlePaddle/PP-OCRv6_small_rec_onnx/resolve/b8f84f0b80c529de40b4fbb3544b84fa7233a513";
const GLM = "https://huggingface.co/onnx-community/GLM-OCR-ONNX/resolve/aea46198f09e3aa2b63422dd234f1cc66afffe52";

/** GLM-OCR's files keep the repository's layout under this folder, where Transformers.js looks for them. */
export const GLM_DIR = "glm-ocr";

export const MODEL_PACKS = {
  paddle: {
    id: "paddle",
    title: "PP-OCRv6 small (text recognition, 50 languages)",
    license: "Apache-2.0, PaddlePaddle",
    webSource: "self",
    files: [
      {
        name: "ppocrv6-small-det.onnx",
        url: `${PADDLE_DET}/inference.onnx`,
        sha256: "d73e0058b7a8086bbd57f3d10b8bcd4ff95363f67e06e2762b5e814fe9c9410e",
        bytes: 9_880_512,
      },
      {
        name: "ppocrv6-small-rec.onnx",
        url: `${PADDLE_REC}/inference.onnx`,
        sha256: "5435fd747c9e0efe15a96d0b378d5bd157e9492ed8fd80edf08f30d02fa24634",
        bytes: 21_159_378,
      },
      {
        name: "ppocrv6-small-rec.yml",
        url: `${PADDLE_REC}/inference.yml`,
        sha256: "ab078671bb49f06228eadccd34f1bb501e157f7a047095ffb943ba81512c77d1",
        bytes: 150_579,
      },
    ],
  },
  // Too large to serve from GitHub Pages: the site fetches it from Hugging Face.
  glm: {
    id: "glm",
    title: "GLM-OCR (whole-page Markdown, formulas)",
    license: "MIT, Z.ai",
    webSource: "upstream",
    files: [
      { name: `${GLM_DIR}/config.json`, url: `${GLM}/config.json`, sha256: "8bf81b89d42ae98917084dfeaca5a1dc20ac20d6145f088ec2aac8840c1e4b9f", bytes: 2_022 },
      { name: `${GLM_DIR}/generation_config.json`, url: `${GLM}/generation_config.json`, sha256: "8da3eae088e11c06a32660d428ce0ac170fefb50e6439469515c82f031d6be45", bytes: 166 },
      { name: `${GLM_DIR}/preprocessor_config.json`, url: `${GLM}/preprocessor_config.json`, sha256: "0d4b3cf5190e6b5b53ac52b1880b111163888bfc975db396561ee78887942fe4", bytes: 366 },
      { name: `${GLM_DIR}/processor_config.json`, url: `${GLM}/processor_config.json`, sha256: "8dd8262e7ac5495801a1e6b8d6c6035331b2381b930b23eb262fc46604527074", bytes: 1_495 },
      { name: `${GLM_DIR}/chat_template.jinja`, url: `${GLM}/chat_template.jinja`, sha256: "062e7ee4cc8defa88a5938b5d456dc60366ffd80647f918946ee747bf09ddc7c", bytes: 4_606 },
      { name: `${GLM_DIR}/tokenizer.json`, url: `${GLM}/tokenizer.json`, sha256: "c3e229a66a06267194e62055f70cf5580af83b3582f46928fefee8cb9618f499", bytes: 5_420_559 },
      { name: `${GLM_DIR}/tokenizer_config.json`, url: `${GLM}/tokenizer_config.json`, sha256: "49feefa948a21a9386864992b4b5fb7cabc9866a71e57edecb13009afe118fa5", bytes: 5_855 },
      { name: `${GLM_DIR}/onnx/vision_encoder_q4.onnx`, url: `${GLM}/onnx/vision_encoder_q4.onnx`, sha256: "23da873106bb5c2ffe544a338df577f5d60b555d8135eeebde5baefae560dea3", bytes: 424_697 },
      { name: `${GLM_DIR}/onnx/vision_encoder_q4.onnx_data`, url: `${GLM}/onnx/vision_encoder_q4.onnx_data`, sha256: "58be7ff915214b0816f4392edd992af45fa097676f85d458e9576caefac5adab", bytes: 304_547_840 },
      { name: `${GLM_DIR}/onnx/embed_tokens_q4.onnx`, url: `${GLM}/onnx/embed_tokens_q4.onnx`, sha256: "e65cf5c2a4c4cc11452e4559818853e9c3aed760b4043204ac27a1fe780a82cc", bytes: 854 },
      { name: `${GLM_DIR}/onnx/embed_tokens_q4.onnx_data`, url: `${GLM}/onnx/embed_tokens_q4.onnx_data`, sha256: "5b285463da8b2a61a4cf4638320b45350199f231cce71ae7762afa9dd2d8bf39", bytes: 58_441_728 },
      { name: `${GLM_DIR}/onnx/decoder_model_merged_q4.onnx`, url: `${GLM}/onnx/decoder_model_merged_q4.onnx`, sha256: "65ba95641621739e5ba7da0aeab0acb2684d379ba502351c9d5ba3b92c44d2ff", bytes: 313_836 },
      { name: `${GLM_DIR}/onnx/decoder_model_merged_q4.onnx_data`, url: `${GLM}/onnx/decoder_model_merged_q4.onnx_data`, sha256: "a0e1b1af9019761f493e93502010c5602a88a222ad5149c713e3d1f890a779c4", bytes: 373_217_280 },
    ],
  },
} as const satisfies Record<string, ModelPack>;

export type ModelPackId = keyof typeof MODEL_PACKS;
export type ModelName = (typeof MODEL_PACKS)[ModelPackId]["files"][number]["name"];

export const modelPackIds = Object.keys(MODEL_PACKS) as ModelPackId[];

export function isModelPackId(id: string): id is ModelPackId {
  return Object.hasOwn(MODEL_PACKS, id);
}

/** Total download size of a pack. */
export function packBytes(id: ModelPackId): number {
  return MODEL_PACKS[id].files.reduce((n: number, f: ModelFile) => n + f.bytes, 0);
}

/** The pack a model file belongs to. */
export function packOf(name: ModelName): ModelPackId {
  return modelPackIds.find((id) => MODEL_PACKS[id].files.some((f: ModelFile) => f.name === name))!;
}

export async function sha256Hex(bytes: Uint8Array): Promise<string> {
  const digest = await crypto.subtle.digest("SHA-256", bytes as Uint8Array<ArrayBuffer>);
  return Array.from(new Uint8Array(digest), (b) => b.toString(16).padStart(2, "0")).join("");
}

/** The host the site downloads a pack from, for the consent card; null for its own origin. */
export function webHost(id: ModelPackId): string | null {
  const pack: ModelPack = MODEL_PACKS[id];
  return pack.webSource === "self" ? null : new URL(pack.files[0]!.url).host;
}

/** "31 MB": sizes as people read them in a download prompt. */
export function megabytes(bytes: number): string {
  return `${Math.max(1, Math.round(bytes / 1_000_000))} MB`;
}
