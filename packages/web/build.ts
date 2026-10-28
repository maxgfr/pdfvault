// Build the site into dist/: bundle the app, the worker and the service
// worker, copy the wasm modules, models and fonts, and write the offline manifest.
// `--serve` then serves dist/ under /pdfvault/ like GitHub Pages does.
import { createHash } from "node:crypto";
import { cp, mkdir, readdir, rm, stat } from "node:fs/promises";
import { dirname, join, relative } from "node:path";
import { servedFiles } from "../engine/src/models.ts";

const here = new URL(".", import.meta.url).pathname;
const root = new URL("../../", import.meta.url).pathname;
const dist = `${here}dist`;
const serve = process.argv.includes("--serve");
const BASE = "/pdfvault/";

await rm(dist, { recursive: true, force: true });

function check(result: Awaited<ReturnType<typeof Bun.build>>) {
  if (!result.success) {
    for (const log of result.logs) console.error(log);
    process.exit(1);
  }
}

check(
  await Bun.build({
    entrypoints: [`${here}index.html`],
    outdir: dist,
    minify: true,
    publicPath: "./",
    naming: { entry: "[name].[ext]", chunk: "assets/[name]-[hash].[ext]", asset: "assets/[name]-[hash].[ext]" },
    define: { "process.env.NODE_ENV": '"production"' },
  }),
);
// GLM-OCR (Transformers.js) is a chunk of its own, assets/glm-*.js, imported only by GLM jobs.
check(
  await Bun.build({
    entrypoints: [`${here}src/worker.ts`],
    outdir: dist,
    minify: true,
    target: "browser",
    format: "esm",
    splitting: true,
    naming: { entry: "worker.js", chunk: "assets/[name]-[hash].[ext]", asset: "assets/[name]-[hash].[ext]" },
  }),
);
// ONNX Runtime's WebGPU build for GLM-OCR, served from here, never from a CDN.
const ort = dirname(Bun.resolveSync("onnxruntime-web/webgpu", Bun.resolveSync("@huggingface/transformers", here)));
await mkdir(`${dist}/ort`, { recursive: true });
for (const ext of ["mjs", "wasm"]) await cp(`${ort}/ort-wasm-simd-threaded.asyncify.${ext}`, `${dist}/ort/ort-wasm-simd-threaded.asyncify.${ext}`);

await cp(`${here}public`, dist, { recursive: true });
// Links to files under public/ are added after bundling: the HTML bundler
// would otherwise try to resolve and rename them.
const html = await Bun.file(`${dist}/index.html`).text();
await Bun.write(
  `${dist}/index.html`,
  html.replace(
    "</head>",
    '<link rel="icon" href="./icon.svg" type="image/svg+xml"><link rel="apple-touch-icon" href="./icon-192.png"><link rel="manifest" href="./manifest.webmanifest"></head>',
  ),
);
for (const name of ["core", "render", "ocr"]) {
  await mkdir(`${dist}/wasm`, { recursive: true });
  await cp(`${root}packages/engine/wasm/${name}/${name}_bg.wasm`, `${dist}/wasm/${name}.wasm`);
}
// The OCR models served for users to download when they first need OCR (GLM-OCR comes from Hugging Face).
await mkdir(`${dist}/models`, { recursive: true });
for (const { name } of servedFiles) await cp(`${root}models/${name}`, `${dist}/models/${name}`);
// The fonts the editor writes and previews text with (SIL OFL 1.1, licence alongside).
await mkdir(`${dist}/fonts`, { recursive: true });
for (const f of await readdir(`${root}models/fonts`)) {
  if (f.endsWith(".ttf")) await cp(`${root}models/fonts/${f}`, `${dist}/fonts/${f}`);
}
await cp(`${root}models/fonts/LICENSE`, `${dist}/fonts/LICENSE.txt`);
await Bun.write(`${dist}/.nojekyll`, "");

async function* walk(dir: string): AsyncGenerator<string> {
  for (const e of await readdir(dir, { withFileTypes: true })) {
    const p = join(dir, e.name);
    if (e.isDirectory()) yield* walk(p);
    else yield p;
  }
}

const files: string[] = [];
const hash = createHash("sha256");
for await (const p of walk(dist)) {
  const rel = relative(dist, p);
  if (rel === ".nojekyll") continue;
  files.push(rel);
  hash.update(rel).update(new Uint8Array(await Bun.file(p).arrayBuffer()));
}
files.sort();
// GLM-OCR's code and ONNX Runtime (tens of MB) are cached only by those who use it.
const lazy = files.filter((f) => f.startsWith("fonts/") || /^assets\/glm-\w+\.js$/.test(f) || f.startsWith("ort/") || f === "wasm/render.wasm" || f === "wasm/ocr.wasm");
// Models stay out of both lists: only the user's "Download" puts them on the device.
const precache = ["./", ...files.filter((f) => f !== "index.html" && !f.startsWith("models/") && !lazy.includes(f))];
const manifest = { version: hash.digest("hex").slice(0, 16), precache, lazy };

check(
  await Bun.build({
    entrypoints: [`${here}src/sw.ts`],
    outdir: dist,
    minify: true,
    target: "browser",
    naming: "sw.js",
    define: { __MANIFEST__: JSON.stringify(manifest) },
  }),
);

let total = 0;
for (const f of files) total += (await stat(`${dist}/${f}`)).size;
console.log(`site: ${files.length + 1} files, ${(total / 1048576).toFixed(1)} MiB, version ${manifest.version}`);

if (serve) {
  const port = Number(process.env.PORT ?? 4173);
  const types: Record<string, string> = { wasm: "application/wasm", webmanifest: "application/manifest+json", mjs: "text/javascript" };
  Bun.serve({
    port,
    async fetch(req) {
      const url = new URL(req.url);
      if (!url.pathname.startsWith(BASE)) return Response.redirect(`${url.origin}${BASE}`, 302);
      let path = url.pathname.slice(BASE.length) || "index.html";
      if (path.endsWith("/")) path += "index.html";
      const file = Bun.file(join(dist, path));
      if (!(await file.exists())) return new Response("not found", { status: 404 });
      const ext = path.split(".").pop() ?? "";
      return new Response(file, types[ext] ? { headers: { "content-type": types[ext] } } : undefined);
    },
  });
  console.log(`serving http://localhost:${port}${BASE}`);
}
