// Download the binary assets, pinned by SHA-256: the PP-OCRv6 models
// (Apache-2.0, PaddlePaddle) into models/, and the Liberation fonts (SIL OFL
// 1.1, metric-compatible with Arial, Times New Roman and Courier New) into
// models/fonts/ for text the editor writes, with Carlito and Caladea for
// Office documents. The site and the CLI ship the
// fonts; the site serves the models for users to download on first OCR, and
// the tests read them from here.
import { mkdir, mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { servedFiles, sha256Hex } from "../packages/engine/src/models.ts";

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
// Carlito and Caladea (SIL OFL 1.1), metric-compatible with Calibri and
// Cambria, the defaults of Office documents: google/fonts at a pinned commit.
const GOOGLE_FONTS = "https://raw.githubusercontent.com/google/fonts/7af0d90b4d532ffa4b041937b560c6df4e7b1e95/ofl";
const OFFICE_FONTS = {
  "Carlito-Regular.ttf": ["carlito/Carlito-Regular.ttf", "f6418f708baede9789daef5d458c0f53d2a888af9820e8062934e504fedc6595"],
  "Carlito-Bold.ttf": ["carlito/Carlito-Bold.ttf", "bb5d20f79b82599ec72983597437373a80f2d2085fa91fc144fd74e876a594db"],
  "Carlito-Italic.ttf": ["carlito/Carlito-Italic.ttf", "0b019225e58d702bfedcbd35c21696769f8ee115cb6343f84c2f240312450d1c"],
  "Carlito-BoldItalic.ttf": ["carlito/Carlito-BoldItalic.ttf", "b32928186c119599e03ca6a1ffc680fdcb7fac95772f4b95d989cf6cd3861517"],
  "Carlito-OFL.txt": ["carlito/OFL.txt", "58402f82a7c332a700294988fe7554fbb0a63a8d27ccc1ee3bbc640311990a00"],
  "Caladea-Regular.ttf": ["caladea/Caladea-Regular.ttf", "f1e899278b7b4491aba5b6a8253c4b04c050cc59b21865be5c37559a775153cd"],
  "Caladea-Bold.ttf": ["caladea/Caladea-Bold.ttf", "ae3cb2dcbc925809dd29d2a44e9802211cab66be541bacbfc9c08c74b27c3742"],
  "Caladea-Italic.ttf": ["caladea/Caladea-Italic.ttf", "4359a8e24f748b6447b1ff6d7a174febe70961d29f8bb8634b56dacd740a3deb"],
  "Caladea-BoldItalic.ttf": ["caladea/Caladea-BoldItalic.ttf", "ccabaa7b7e2fdf253d2b1a5fa699dd8a3df8d835a9eb285ad82631a677eb76c0"],
  "Caladea-OFL.txt": ["caladea/OFL.txt", "ccdab61d371d8c8683a128a92cd7d498dbdb1d37689f7cb21f1bf6b16658d213"],
} as const;

const dir = new URL("../models/", import.meta.url).pathname;
const fontDir = `${dir}fonts/`;

async function intact(path: string, expected: string): Promise<boolean> {
  const f = Bun.file(path);
  return (await f.exists()) && (await sha256Hex(await f.bytes())) === expected;
}

async function download(url: string, expected: string, label: string): Promise<Uint8Array> {
  const res = await fetch(url);
  if (!res.ok) throw new Error(`${label}: HTTP ${res.status}`);
  const bytes = await res.bytes();
  const actual = await sha256Hex(bytes);
  if (actual !== expected) throw new Error(`${label}: checksum ${actual} does not match ${expected}`);
  return bytes;
}

await mkdir(fontDir, { recursive: true });
for (const { name, url, sha256: expected } of servedFiles) {
  if (await intact(dir + name, expected)) {
    console.log(`${name}: ok`);
    continue;
  }
  await Bun.write(dir + name, await download(url, expected, name));
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
      const bytes = await Bun.file(join(from, name)).bytes();
      const actual = await sha256Hex(bytes);
      if (actual !== expected) throw new Error(`${name}: checksum ${actual} does not match ${expected}`);
      await Bun.write(fontDir + name, bytes);
    }
    console.log(`fonts: ${missing.length} file(s) extracted`);
  } finally {
    await rm(tmp, { recursive: true, force: true });
  }
}

let fetched = 0;
for (const [name, [path, expected]] of Object.entries(OFFICE_FONTS)) {
  if (await intact(fontDir + name, expected)) continue;
  await Bun.write(fontDir + name, await download(`${GOOGLE_FONTS}/${path}`, expected, name));
  fetched++;
}
console.log(fetched ? `office fonts: ${fetched} file(s) downloaded` : "office fonts: ok");
