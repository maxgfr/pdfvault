// Fail the build if the shipped site could reach another origin: any
// external URL in a bundle is an error unless allowlisted below with a
// reason, and network APIs pointed at a literal external URL are always an
// error. Same idea as tok/scripts/check-no-network.mjs.
import { readdir } from "node:fs/promises";
import { join, relative } from "node:path";

const dist = process.argv[2] ?? "packages/web/dist";

// Hosts that may appear as inert strings, each with the reason it is safe.
const ALLOWED_HOSTS: [string, string][] = [
  ["react.dev", "React's minified-error decoder URL inside thrown Error messages"],
  ["www.w3.org", "SVG/XHTML namespace URIs, never dereferenced"],
  ["maxgfr.github.io", "the site's own canonical address in a <link rel=canonical>"],
  ["github.com", "the 'Source' links: anchors the user chooses to follow, not requests"],
  ["www.idpf.org", "EPUB XML namespace, a string written into generated books"],
  ["purl.org", "Dublin Core namespace, a string written into generated books"],
  ["ns.adobe.com", "XMP namespace strings inside the PDF engine"],
  [
    "huggingface.co",
    "upstream source of each OCR model in the shared catalog (models.ts), which the CLI downloads from; the site downloads the same pinned files from its own origin, and its CSP (connect-src 'self') would block this host",
  ],
];

const URL_RE = /\bhttps?:\/\/([a-z0-9.-]+\.[a-z]{2,})/gi;
const FETCH_LITERAL = /\b(?:fetch|import|importScripts|sendBeacon|WebSocket|EventSource)\s*\(\s*["'`](?:https?:)?\/\/[^"'`]+/gi;

async function* walk(dir: string): AsyncGenerator<string> {
  for (const e of await readdir(dir, { withFileTypes: true })) {
    const p = join(dir, e.name);
    if (e.isDirectory()) yield* walk(p);
    else yield p;
  }
}

const allowed = new Set(ALLOWED_HOSTS.map(([h]) => h));
const problems: string[] = [];
let scanned = 0;
for await (const path of walk(dist)) {
  if (!/\.(js|html|css|json|webmanifest|wasm)$/.test(path)) continue;
  scanned++;
  const bytes = await Bun.file(path).arrayBuffer();
  const rel = relative(dist, path);
  if (path.endsWith(".wasm")) {
    // wasm reaches the network only through imported JS functions; strings
    // in its data (such as brotli's dictionary) are inert. Check the imports.
    for (const imp of WebAssembly.Module.imports(new WebAssembly.Module(bytes))) {
      if (/fetch|xmlhttp|websocket|beacon|eventsource|import_?script/i.test(imp.name)) problems.push(`${rel}: imports ${imp.module}.${imp.name}`);
    }
    continue;
  }
  const text = new TextDecoder("latin1").decode(bytes);
  for (const m of text.matchAll(URL_RE)) {
    const host = m[1]!.toLowerCase();
    if (!allowed.has(host)) problems.push(`${rel}: external URL ${m[0]}`);
  }
  for (const m of text.matchAll(FETCH_LITERAL)) problems.push(`${rel}: network call to a literal URL: ${m[0]}`);
}

if (scanned === 0) {
  console.error(`no files scanned in ${dist}; build the site first`);
  process.exit(1);
}
if (problems.length) {
  console.error([...new Set(problems)].join("\n"));
  console.error(`\n${problems.length} problem(s). Allowlist a host only with a reason, in scripts/check-no-network.ts.`);
  process.exit(1);
}
console.log(`no-network check: ${scanned} files clean`);
