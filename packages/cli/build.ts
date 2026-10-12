// Build the CLI's npm bundle: dist/pdfvault.js plus its wasm and models, run
// with bun or bunx. Standalone executables: scripts/cli-binaries.ts.
import { chmod, rm } from "node:fs/promises";

const here = new URL(".", import.meta.url).pathname;
const dist = `${here}dist`;

await rm(dist, { recursive: true, force: true });

const bundle = await Bun.build({
  entrypoints: [`${here}src/main.ts`],
  outdir: dist,
  target: "bun",
  minify: true,
  naming: { entry: "pdfvault.js", asset: "[name]-[hash].[ext]" },
});
if (!bundle.success) {
  for (const log of bundle.logs) console.error(log);
  process.exit(1);
}
await chmod(`${dist}/pdfvault.js`, 0o755);
console.log(`bundle: ${bundle.outputs.map((o) => o.path.replace(here, "")).join(", ")}`);
