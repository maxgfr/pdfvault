// The fonts the engine writes text with when the document's own font will
// not do: Liberation Sans, Serif and Mono (SIL OFL 1.1), metric-compatible
// with Arial, Times New Roman and Courier New, and for Office documents
// Carlito and Caladea (SIL OFL 1.1), metric-compatible with Calibri and
// Cambria. `bun run fetch-models` puts them in models/fonts; the site serves
// them from fonts/ and the CLI embeds them.

export const BUNDLED_FONTS = {
  "sans-regular": "LiberationSans-Regular.ttf",
  "sans-bold": "LiberationSans-Bold.ttf",
  "sans-italic": "LiberationSans-Italic.ttf",
  "sans-bolditalic": "LiberationSans-BoldItalic.ttf",
  "serif-regular": "LiberationSerif-Regular.ttf",
  "serif-bold": "LiberationSerif-Bold.ttf",
  "serif-italic": "LiberationSerif-Italic.ttf",
  "serif-bolditalic": "LiberationSerif-BoldItalic.ttf",
  "mono-regular": "LiberationMono-Regular.ttf",
  "mono-bold": "LiberationMono-Bold.ttf",
  "mono-italic": "LiberationMono-Italic.ttf",
  "mono-bolditalic": "LiberationMono-BoldItalic.ttf",
  "carlito-regular": "Carlito-Regular.ttf",
  "carlito-bold": "Carlito-Bold.ttf",
  "carlito-italic": "Carlito-Italic.ttf",
  "carlito-bolditalic": "Carlito-BoldItalic.ttf",
  "caladea-regular": "Caladea-Regular.ttf",
  "caladea-bold": "Caladea-Bold.ttf",
  "caladea-italic": "Caladea-Italic.ttf",
  "caladea-bolditalic": "Caladea-BoldItalic.ttf",
} as const;

export type FontKey = keyof typeof BUNDLED_FONTS;
export type FontFile = (typeof BUNDLED_FONTS)[FontKey];

export function isFontKey(key: string): key is FontKey {
  return Object.hasOwn(BUNDLED_FONTS, key);
}

/** TrueType, OpenType or a collection of them (not WOFF). */
export function isFontFile(b: Uint8Array): boolean {
  const tag = String.fromCharCode(...b.subarray(0, 4));
  return (b[0] === 0 && b[1] === 1 && b[2] === 0 && b[3] === 0) || tag === "true" || tag === "OTTO" || tag === "ttcf";
}
