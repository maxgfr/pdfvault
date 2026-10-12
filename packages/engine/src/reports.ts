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
