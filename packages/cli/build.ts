// Build the CLI: an npm bundle (dist/pdfvault.js + assets, run with bun or
// bunx) and, with --binaries, standalone executables for each platform.
// Usage: bun packages/cli/build.ts [--binaries] [--target bun-linux-x64 ...]
import { chmod, rm } from "node:fs/promises";

const here = new URL(".", import.meta.url).pathname;
const dist = `${here}dist`;
const args = process.argv.slice(2);

export const TARGETS = [
  "bun-darwin-arm64",
  "bun-darwin-x64",
  "bun-linux-arm64",
  "bun-linux-x64",
  "bun-windows-x64",
  "bun-windows-arm64",
] as const;

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

if (args.includes("--binaries")) {
  const picked = args.filter((a) => a.startsWith("bun-"));
  for (const target of picked.length ? picked : TARGETS) {
    const name = `pdfvault-${target.replace(/^bun-/, "")}${target.includes("windows") ? ".exe" : ""}`;
    const proc = Bun.spawnSync(
      ["bun", "build", `${here}src/main.ts`, "--compile", "--minify", `--target=${target}`, `--outfile=${dist}/bin/${name}`],
      { stdout: "inherit", stderr: "inherit" },
    );
    if (proc.exitCode !== 0) process.exit(proc.exitCode ?? 1);
  }
}
