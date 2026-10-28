// Shapes of the reports the Rust engine returns (serde, camelCase).

export interface ImageReport {
  object: string;
  width: number;
  height: number;
  newWidth: number;
  newHeight: number;
  dpi: number | null;
  bytesBefore: number;
  bytesAfter: number;
  action: "jpeg" | "flate" | "indexed" | "kept" | "skipped";
  reason?: string;
}

export interface PdfaReport {
  pages: number;
  /** No blocker: the file declares itself PDF/A-2b. */
  conformant: boolean;
  /** What was taken out, as "kind:count". */
  removed: string[];
  /** Fonts that were not embedded, and the font embedded in their place. */
  fonts: { font: string; embedded: string }[];
  /** What still keeps the file from being PDF/A-2b. */
  blockers: string[];
}

export interface CompressReport {
  preset: "lossless" | "balanced" | "strong" | "extreme" | null;
  inputBytes: number;
  outputBytes: number;
  savedBytes: number;
  ratio: number;
  keptOriginal: boolean;
  pages: number;
  streamsRecompressed: number;
  duplicatesMerged: number;
  removed: string[];
  images: ImageReport[];
}
