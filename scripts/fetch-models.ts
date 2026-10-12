// Download the ocrs models (CC-BY-SA 4.0, Robert Knight) into models/, pinned
// by SHA-256. The site and the CLI ship these exact files; nothing is fetched
// at run time.
import { mkdir } from "node:fs/promises";

const MODELS = {
  "text-detection.rten": "f15cfb56bd02c4bf478a20343986504a1f01e1665c2b3a0ad66340f054b1b5ca",
  "text-recognition.rten": "e484866d4cce403175bd8d00b128feb08ab42e208de30e42cd9889d8f1735a6e",
} as const;
const BASE = "https://ocrs-models.s3-accelerate.amazonaws.com";
const dir = new URL("../models/", import.meta.url).pathname;

async function sha256(bytes: ArrayBuffer): Promise<string> {
  const digest = await crypto.subtle.digest("SHA-256", bytes);
  return Buffer.from(digest).toString("hex");
}

await mkdir(dir, { recursive: true });
for (const [name, expected] of Object.entries(MODELS)) {
  const path = dir + name;
  const existing = Bun.file(path);
  if ((await existing.exists()) && (await sha256(await existing.arrayBuffer())) === expected) {
    console.log(`${name}: ok`);
    continue;
  }
  const res = await fetch(`${BASE}/${name}`);
  if (!res.ok) throw new Error(`${name}: HTTP ${res.status}`);
  const bytes = await res.arrayBuffer();
  const actual = await sha256(bytes);
  if (actual !== expected) throw new Error(`${name}: checksum ${actual} does not match ${expected}`);
  await Bun.write(path, bytes);
  console.log(`${name}: downloaded`);
}
