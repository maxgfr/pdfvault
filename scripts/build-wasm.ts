// Build the three wasm modules into packages/engine/wasm/<name>.
// Usage: bun scripts/build-wasm.ts [core|render|ocr ...]
import { $ } from "bun";
import { rm, stat } from "node:fs/promises";

const root = new URL("..", import.meta.url).pathname;
const all = ["core", "render", "ocr"] as const;
const wanted = process.argv.slice(2);
const names = wanted.length ? all.filter((n) => wanted.includes(n)) : all;

for (const name of names) {
  const out = `${root}packages/engine/wasm/${name}`;
  await rm(out, { recursive: true, force: true });
  await $`wasm-pack build ${root}crates/wasm-${name} --target web --release --no-pack --out-dir ${out} --out-name ${name}`.quiet();
  await rm(`${out}/.gitignore`, { force: true });
  const { size } = await stat(`${out}/${name}_bg.wasm`);
  console.log(`${name}: ${(size / 1024 / 1024).toFixed(2)} MiB`);
}
