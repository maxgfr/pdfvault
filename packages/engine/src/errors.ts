import type { ModelPackId } from "./models.ts";

/** Stable error codes shared by the Rust engine, the CLI's --json and the site. */
export type ErrorCode =
  | "invalid_pdf"
  | "password_required"
  | "wrong_password"
  | "limit"
  | "invalid_option"
  | "unsupported"
  | "image"
  | "missing_glyphs"
  | "ocr"
  | "ocr_model"
  /** The OCR model is not on this device yet: download it (`pdfvault models pull`), then run again. */
  | "model_missing"
  | "ocr_not_ready"
  | "internal";

export class EngineError extends Error {
  readonly code: ErrorCode;
  /** With `model_missing`: the model pack to download. */
  readonly pack?: ModelPackId;
  constructor(code: ErrorCode, message: string, pack?: ModelPackId) {
    super(message);
    this.name = "EngineError";
    this.code = code;
    if (pack) this.pack = pack;
  }
}

/** wasm-bindgen throws `Error("{\"code\":...,\"message\":...}")`; turn it back into an EngineError. */
export function toEngineError(e: unknown): EngineError {
  if (e instanceof EngineError) return e;
  const text = e instanceof Error ? e.message : String(e);
  try {
    const parsed = JSON.parse(text) as { code?: ErrorCode; message?: string };
    if (parsed && typeof parsed.code === "string") return new EngineError(parsed.code, parsed.message ?? text);
  } catch {
    // Not one of ours: a trap, out of memory, or a JS error.
  }
  if (/memory|out of bounds|unreachable/i.test(text)) {
    return new EngineError("limit", `the engine ran out of memory or crashed on this file (${text})`);
  }
  return new EngineError("internal", text);
}
