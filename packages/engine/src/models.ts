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
  files: readonly ModelFile[];
}

const PADDLE_DET = "https://huggingface.co/PaddlePaddle/PP-OCRv6_small_det_onnx/resolve/28fe5895c24fd108c19eb3e8479f4ab385fbfc62";
const PADDLE_REC = "https://huggingface.co/PaddlePaddle/PP-OCRv6_small_rec_onnx/resolve/b8f84f0b80c529de40b4fbb3544b84fa7233a513";

export const MODEL_PACKS = {
  paddle: {
    id: "paddle",
    title: "PP-OCRv6 small (text recognition, 50 languages)",
    license: "Apache-2.0, PaddlePaddle",
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

/** "31 MB": sizes as people read them in a download prompt. */
export function megabytes(bytes: number): string {
  return `${Math.max(1, Math.round(bytes / 1_000_000))} MB`;
}
