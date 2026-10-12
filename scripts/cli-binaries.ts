// Standalone pdfvault executables (Bun runtime + CLI bundle + wasm + OCR
// models), named like the rest of maxgfr/homebrew-tap: pdfvault-<os>-<arch>.
//
//   bun scripts/cli-binaries.ts [--version 1.2.3] [target...]
//   targets: macos-arm64 macos-x64 linux-arm64 linux-x64 windows-x64 windows-arm64
//
// x64 uses Bun's baseline runtime, which runs on CPUs without AVX2. Appending
// the bundle leaves a macOS binary's signature stale and macOS kills it, so
// macOS binaries are re-signed ad hoc and must be built on macOS.
import { chmod, mkdir } from "node:fs/promises";

const BUN_TARGETS: Record<string, string> = {
  "macos-arm64": "bun-darwin-arm64",
  "macos-x64": "bun-darwin-x64-baseline",
  "linux-arm64": "bun-linux-arm64",
  "linux-x64": "bun-linux-x64-baseline",
  "windows-x64": "bun-windows-x64-baseline",
  "windows-arm64": "bun-windows-arm64",
};

const root = new URL("..", import.meta.url).pathname;
const args = process.argv.slice(2);
const vi = args.indexOf("--version");
const version = vi >= 0 ? args.splice(vi, 2)[1] : undefined;
const targets = args.length ? args : Object.keys(BUN_TARGETS);

if (version) {
  if (!/^\d+\.\d+\.\d+/.test(version)) throw new Error(`bad version "${version}"`);
  const path = `${root}packages/cli/package.json`;
  const pkg = await Bun.file(path).json();
  pkg.version = version;
  await Bun.write(path, JSON.stringify(pkg, null, 2) + "\n");
}

const NATIVE: Record<string, string> = { "darwin-arm64": "macos-arm64", "darwin-x64": "macos-x64", "linux-arm64": "linux-arm64", "linux-x64": "linux-x64" };
const native = NATIVE[`${process.platform}-${process.arch}`];
const outDir = `${root}packages/cli/dist/bin`;
await mkdir(outDir, { recursive: true });

for (const target of targets) {
  const bunTarget = BUN_TARGETS[target];
  if (!bunTarget) throw new Error(`unknown target ${target} (${Object.keys(BUN_TARGETS).join(", ")})`);
  if (target.startsWith("macos") && process.platform !== "darwin") throw new Error(`${target} must be built on macOS (it is re-signed)`);
  const file = `${outDir}/pdfvault-${target}${target.startsWith("windows") ? ".exe" : ""}`;
  const build = Bun.spawnSync(
    ["bun", "build", `${root}packages/cli/src/main.ts`, "--compile", "--minify", `--target=${bunTarget}`, `--outfile=${file}`],
    { stdout: "inherit", stderr: "inherit" },
  );
  if (build.exitCode !== 0) process.exit(build.exitCode ?? 1);
  await chmod(file, 0o755);
  if (target.startsWith("macos")) {
    const sign = Bun.spawnSync(["codesign", "--force", "--sign", "-", file], { stdout: "inherit", stderr: "inherit" });
    if (sign.exitCode !== 0) process.exit(sign.exitCode ?? 1);
  }
  // The binary this machine can run proves the build works end to end.
  if (target === native) {
    const v = Bun.spawnSync([file, "--version"]).stdout.toString().trim();
    const expected = version ?? (await Bun.file(`${root}packages/cli/package.json`).json()).version;
    if (v !== expected) throw new Error(`${file} --version printed "${v}", expected "${expected}"`);
  }
  console.log(`built ${file.replace(root, "")}`);
}
