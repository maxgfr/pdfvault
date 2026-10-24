// Download the binary assets the engine ships, pinned by SHA-256: the ocrs
// models (CC-BY-SA 4.0, Robert Knight) into models/, and the Liberation
// fonts (SIL OFL 1.1, metric-compatible with Arial, Times New Roman and
// Courier New) into models/fonts/ for text the editor writes. The site and
// the CLI ship these exact files; nothing is fetched at run time.
import { mkdir, mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

const MODELS = {
  "text-detection.rten": "f15cfb56bd02c4bf478a20343986504a1f01e1665c2b3a0ad66340f054b1b5ca",
  "text-recognition.rten": "e484866d4cce403175bd8d00b128feb08ab42e208de30e42cd9889d8f1735a6e",
} as const;
const BASE = "https://ocrs-models.s3-accelerate.amazonaws.com";

const FONTS_ARCHIVE = {
  url: "https://github.com/liberationfonts/liberation-fonts/files/7261482/liberation-fonts-ttf-2.1.5.tar.gz",
  sha256: "7191c669bf38899f73a2094ed00f7b800553364f90e2637010a69c0e268f25d0",
};
const FONTS = {
  "LiberationSans-Regular.ttf": "76d04c18ea243f426b7de1f3ad208e927008f961dc5945e5aad352d0dfde8ee8",
  "LiberationSans-Bold.ttf": "788abee4c806d660e8aee46689dd8540cd4bb98da03dcc9d171ce3efd99a9173",
  "LiberationSans-Italic.ttf": "e5bae5c4cde31f22142753855f4f8fb86da6ff39955ed3c0a11248b0d16948b0",
  "LiberationSans-BoldItalic.ttf": "698da70fc191cc5f33ad4d6d3fe830fe4624b898ea2e3169955928b7c491f1ee",
  "LiberationSerif-Regular.ttf": "058ea80864aef09a23f45cbec2bb5400bc3dfbdea01c3f10538a21fcb497fb74",
  "LiberationSerif-Bold.ttf": "d754ba427cfe0bca54ae052384baa8f842da5bd6550ad4da024ac441e7a7d5ce",
  "LiberationSerif-Italic.ttf": "0e3dea9f8d613e006ccfa62201f33e265d19167bd0907725c3e145368b04fc2e",
  "LiberationSerif-BoldItalic.ttf": "f17db8af71e24d2066b587546021d4f0b296be389512b658dec3c09affeb11a7",
  "LiberationMono-Regular.ttf": "f2b83c763e8afd21709333370bed4774337fae82267937e2b5aea7e2fbd922c1",
  "LiberationMono-Bold.ttf": "bd62a0672d0b9b6710b01df434c80ad54fa5f0835207eb7b17b7a761463067bb",
  "LiberationMono-Italic.ttf": "605c01c711b44480a7508d349dfbf3264e81fa43d69e61cfa7d10b86e764c4d1",
  "LiberationMono-BoldItalic.ttf": "79451f3c09fe25116098853b7a2ca6e2436220ccc11af022979adbcf195be130",
  LICENSE: "93fed46019c38bbe566b479d22148e2e8a1e85ada614accb0211c37b2c61c19b",
} as const;

const dir = new URL("../models/", import.meta.url).pathname;
const fontDir = `${dir}fonts/`;

async function sha256(bytes: ArrayBuffer): Promise<string> {
  const digest = await crypto.subtle.digest("SHA-256", bytes);
  return Buffer.from(digest).toString("hex");
}

async function intact(path: string, expected: string): Promise<boolean> {
  const f = Bun.file(path);
  return (await f.exists()) && (await sha256(await f.arrayBuffer())) === expected;
}

async function download(url: string, expected: string, label: string): Promise<ArrayBuffer> {
  const res = await fetch(url);
  if (!res.ok) throw new Error(`${label}: HTTP ${res.status}`);
  const bytes = await res.arrayBuffer();
  const actual = await sha256(bytes);
  if (actual !== expected) throw new Error(`${label}: checksum ${actual} does not match ${expected}`);
  return bytes;
}

await mkdir(fontDir, { recursive: true });
for (const [name, expected] of Object.entries(MODELS)) {
  if (await intact(dir + name, expected)) {
    console.log(`${name}: ok`);
    continue;
  }
  await Bun.write(dir + name, await download(`${BASE}/${name}`, expected, name));
  console.log(`${name}: downloaded`);
}

const missing = [];
for (const [name, expected] of Object.entries(FONTS)) if (!(await intact(fontDir + name, expected))) missing.push(name);
if (missing.length === 0) {
  console.log("fonts: ok");
} else {
  const tmp = await mkdtemp(join(tmpdir(), "pdfvault-fonts-"));
  try {
    const archive = join(tmp, "fonts.tar.gz");
    await Bun.write(archive, await download(FONTS_ARCHIVE.url, FONTS_ARCHIVE.sha256, "fonts"));
    const tar = Bun.spawnSync(["tar", "-xzf", archive, "-C", tmp]);
    if (tar.exitCode !== 0) throw new Error(`fonts: tar failed: ${tar.stderr.toString()}`);
    const from = join(tmp, "liberation-fonts-ttf-2.1.5");
    for (const [name, expected] of Object.entries(FONTS)) {
      const bytes = await Bun.file(join(from, name)).arrayBuffer();
      const actual = await sha256(bytes);
      if (actual !== expected) throw new Error(`${name}: checksum ${actual} does not match ${expected}`);
      await Bun.write(fontDir + name, bytes);
    }
    console.log(`fonts: ${missing.length} file(s) extracted`);
  } finally {
    await rm(tmp, { recursive: true, force: true });
  }
}
