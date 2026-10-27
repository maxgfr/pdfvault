// Whether this browser can run GLM-OCR: on the graphics card through WebGPU,
// with memory to spare, on a desktop. The page asks before offering it.

interface GpuLike {
  requestAdapter(): Promise<unknown>;
}

/** Why GLM-OCR cannot run here, or null when it can. */
export async function glmUnsupported(): Promise<string | null> {
  const nav = navigator as Navigator & { gpu?: GpuLike; deviceMemory?: number };
  // iPadOS reports itself as a Mac, with a touch screen.
  if (/iPhone|iPad|iPod/.test(nav.userAgent) || (/Macintosh/.test(nav.userAgent) && nav.maxTouchPoints > 1)) {
    return "GLM-OCR needs a computer: iPhone and iPad browsers do not have the memory for it.";
  }
  if (nav.deviceMemory !== undefined && nav.deviceMemory < 4) return "GLM-OCR needs at least 4 GB of memory.";
  if (!nav.gpu) return "GLM-OCR needs WebGPU, which this browser does not offer.";
  const adapter = await nav.gpu.requestAdapter().catch(() => null);
  return adapter ? null : "GLM-OCR needs WebGPU, and this browser found no graphics card for it.";
}
