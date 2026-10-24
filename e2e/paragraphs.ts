// Print a PDF page's paragraphs as JSON, through the engine: the e2e tests
// check reflowed text with it (to-text joins a paragraph's lines back).
// Usage: bun e2e/paragraphs.ts file.pdf [page]
import { engine } from "../packages/engine/test/loader.ts";

const pdf = new Uint8Array(await Bun.file(process.argv[2]!).arrayBuffer());
const r = await engine.core<{ paragraphs: { text: string; lines: number; left: number; right: number }[] }>("text-runs", [pdf], {
  pages: process.argv[3] ?? "1",
});
console.log(JSON.stringify(r.report.paragraphs.map((p) => ({ text: p.text, lines: p.lines, left: p.left, right: p.right }))));
