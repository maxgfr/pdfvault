// The PDF editor through the real UI: place things with the mouse, organize
// pages, fill a form, redact, save, and check what the engine wrote.
import { expect, test, type Page } from "@playwright/test";
import { execFileSync } from "node:child_process";
import { mkdtemp, readFile, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fixture, isPdf, watch } from "./helpers.ts";

// Tall enough for a whole page below the sticky toolbar.
test.use({ viewport: { width: 1280, height: 1400 } });

const cli = process.env.PDFVAULT_BIN ? [process.env.PDFVAULT_BIN] : ["bun", "packages/cli/src/main.ts"];

async function open(page: Page, name: string) {
  await page.goto("./#/edit");
  await expect(page.getByRole("heading", { level: 1, name: "Edit PDF" })).toBeVisible();
  await page.getByLabel("PDF to edit").setInputFiles(fixture(name));
  await expect(page.getByRole("region", { name: "Pages" }).getByRole("img", { name: "Page 1" })).toBeVisible({ timeout: 90_000 });
}

/** A point on page `n` (1-based) as fractions of its width and height. */
async function at(page: Page, n: number, fx: number, fy: number) {
  const el = page.getByRole("img", { name: `Page ${n}`, exact: true });
  await el.evaluate((e) => e.scrollIntoView({ block: "center" }));
  const box = (await el.boundingBox())!;
  return { x: box.x + box.width * fx, y: box.y + box.height * fy };
}

async function dragOn(page: Page, n: number, from: [number, number], to: [number, number]) {
  const a = await at(page, n, ...from);
  const b = await at(page, n, ...to);
  await page.mouse.move(a.x, a.y);
  await page.mouse.down();
  await page.mouse.move((a.x + b.x) / 2, (a.y + b.y) / 2, { steps: 4 });
  await page.mouse.move(b.x, b.y, { steps: 4 });
  await page.mouse.up();
}

async function save(page: Page): Promise<string> {
  await page.getByRole("button", { name: "Save PDF" }).click();
  const result = page.locator(".ed-result");
  await expect(result).toBeVisible({ timeout: 90_000 });
  const [dl] = await Promise.all([page.waitForEvent("download"), result.getByRole("button", { name: "Download" }).click()]);
  const bytes = new Uint8Array(await readFile((await dl.path())!));
  expect(isPdf(bytes)).toBe(true);
  const path = join(await mkdtemp(join(tmpdir(), "pdfvault-edit-")), dl.suggestedFilename());
  await writeFile(path, bytes);
  return path;
}

/** Whether `needle`'s characters appear in `hay` in the same order. */
function subsequence(hay: string, needle: string): boolean {
  let i = 0;
  for (const c of hay) if (c === needle[i]) i++;
  return i === needle.length;
}

const text = (pdf: string) => execFileSync(cli[0]!, [...cli.slice(1), "to-text", pdf, "--no-ocr", "--page-breaks", "-o", "-"]).toString();
/** The PDF with object streams unpacked, so dictionaries can be searched. */
const qdf = (pdf: string) => execFileSync("qpdf", ["--qdf", "--object-streams=disable", pdf, "-"], { maxBuffer: 256 << 20 }).toString("latin1");

test("text, whiteout, highlight, note, signature and page order in one save", async ({ page }) => {
  const w = await watch(page);
  await open(page, "text-3.pdf");

  await page.getByRole("button", { name: "Add text" }).click();
  const p = await at(page, 1, 0.12, 0.55);
  await page.mouse.click(p.x, p.y);
  await page.getByRole("textbox", { name: "Text" }).fill("Signed by Ada");
  await page.keyboard.press("Escape");
  await expect(page.locator(".ed-text text")).toHaveText("Signed by Ada");

  await page.getByRole("button", { name: "Whiteout" }).click();
  await dragOn(page, 1, [0.1, 0.1], [0.5, 0.16]);
  await page.getByRole("button", { name: "Highlight" }).click();
  await dragOn(page, 2, [0.1, 0.1], [0.5, 0.14]);
  await page.getByRole("button", { name: "Note" }).click();
  const n = await at(page, 3, 0.8, 0.1);
  await page.mouse.click(n.x, n.y);
  await page.getByRole("textbox", { name: "Comment" }).fill("Check this");
  await page.keyboard.press("Escape");

  await page.getByRole("button", { name: "Sign / image" }).click();
  const pad = page.getByLabel("Signature pad");
  const box = (await pad.boundingBox())!;
  await page.mouse.move(box.x + 40, box.y + box.height * 0.6);
  await page.mouse.down();
  for (let i = 1; i <= 12; i++) await page.mouse.move(box.x + 40 + i * 30, box.y + box.height * (0.6 + 0.2 * Math.sin(i)));
  await page.mouse.up();
  await page.getByRole("button", { name: "Use this signature" }).click();
  const s = await at(page, 2, 0.7, 0.85);
  await page.mouse.click(s.x, s.y);
  await expect(page.locator(".ed-image")).toHaveCount(1);

  // Undo and redo the signature.
  await page.getByRole("button", { name: "Undo" }).click();
  await expect(page.locator(".ed-image")).toHaveCount(0);
  await page.getByRole("button", { name: "Redo" }).click();
  await expect(page.locator(".ed-image")).toHaveCount(1);

  const order = page.getByRole("navigation", { name: "Page order" });
  await order.getByRole("button", { name: "Move page 3 up" }).click();
  await order.getByRole("button", { name: "Turn page 1 right" }).click();
  await order.getByRole("button", { name: "Go to page 3" }).click();
  await order.getByRole("button", { name: "Blank page after 3" }).click();

  const out = await save(page);
  await expect(page.locator(".ed-result")).toContainText("4 pages");
  // Rotated pages come out glyph by glyph: compare without whitespace.
  const pages = text(out)
    .split("\f")
    .map((t) => t.replace(/\s+/g, ""));
  expect(pages).toHaveLength(4);
  // Turned 90°, the page's lines come out interleaved: the added text is there in order.
  expect(subsequence(pages[0]!, "SignedbyAda")).toBe(true);
  expect(pages[1]).toContain("Page3");
  expect(pages[2]).toContain("Page2");
  const raw = qdf(out);
  expect(raw).toContain("/Subtype /Highlight");
  expect(raw).toContain("/Subtype /Text");
  expect(raw).toContain("/Rotate 90");
  await w.assertClean();
});

test("a form is filled in place", async ({ page }) => {
  await open(page, "form.pdf");
  await page.getByLabel("Form field name").fill("Grace Hopper");
  await page.getByLabel("Form field agree").uncheck();
  const raw = qdf(await save(page));
  expect(raw).toContain("(Grace Hopper)");
  expect(raw).toContain("/V /Off");
});

test("redaction removes the text under the box for good", async ({ page }) => {
  await open(page, "text-3.pdf");
  await page.getByRole("button", { name: "Redact" }).click();
  await dragOn(page, 1, [0.02, 0.02], [0.98, 0.4]);
  await expect(page.locator(".ed-redaction")).toContainText("1 redaction");
  const out = await save(page);
  await expect(page.locator(".ed-result")).toContainText("redacted page 1");
  const pages = text(out).split("\f");
  expect(pages[0]).not.toContain("Page 1");
  expect(pages[1]).toContain("Page 2");
});

test("redaction keeps the rest of the page as text", async ({ page }) => {
  const w = await watch(page);
  await open(page, "text-3.pdf");
  await page.getByRole("button", { name: "Redact" }).click();
  // Over "The quick brown" (body line at y = 702 of 792, from x = 72).
  await dragOn(page, 1, [0.11, 0.105], [0.32, 0.12]);
  const out = await save(page);
  await expect(page.locator(".ed-result")).toContainText("redacted page 1");
  await expect(page.locator(".ed-result")).not.toContainText("turned into images");
  const first = text(out).split("\f")[0]!;
  expect(first).not.toContain("quick");
  expect(first).toContain("Page 1");
  expect(first).toContain("lazy dog");
  await w.assertClean();
});

test("existing text is changed in place, in its own font", async ({ page }) => {
  const w = await watch(page);
  await open(page, "text-3.pdf");
  await page.getByRole("button", { name: "Edit text" }).click();
  // "Page 1": 24 pt Helvetica on the baseline at y = 720, from x = 72.
  const p = await at(page, 1, 0.15, (792 - 728) / 792);
  await page.mouse.click(p.x, p.y);
  const line = page.getByRole("textbox", { name: "Edit this paragraph" });
  await expect(line).toHaveValue("Page 1");
  await line.fill("Première page");
  await page.keyboard.press("Escape");
  await expect(page.getByLabel("Font")).toHaveValue("auto");
  const out = await save(page);
  const first = text(out).split("\f")[0]!;
  expect(first).toContain("Première page");
  expect(first).not.toContain("Page 1");
  // Helvetica had every letter: nothing new was embedded.
  expect(qdf(out)).not.toContain("/FontFile2");
  await w.assertClean();
});

test("a digit the embedded font lacks: the rest of the line keeps its font", async ({ page }) => {
  // "Total 2025" in an embedded subset of Liberation Sans, written by the CLI.
  const dir = await mkdtemp(join(tmpdir(), "pdfvault-subset-"));
  const subset = join(dir, "subset.pdf");
  const add = JSON.stringify({ items: [{ type: "text", page: 0, x: 72, y: 400, text: "Total 2025", size: 18 }] });
  execFileSync(cli[0]!, [...cli.slice(1), "edit", fixture("text-3.pdf"), "--ops", add, "-o", subset]);
  await page.goto("./#/edit");
  await page.getByLabel("PDF to edit").setInputFiles(subset);
  await expect(page.getByRole("region", { name: "Pages" }).getByRole("img", { name: "Page 1" })).toBeVisible({ timeout: 90_000 });
  await page.getByRole("button", { name: "Edit text" }).click();
  // Baseline at 400 − 0.8 × 18 = 385.6 points from the bottom.
  const p = await at(page, 1, 0.15, (792 - 390) / 792);
  await page.mouse.click(p.x, p.y);
  const line = page.getByRole("textbox", { name: "Edit this paragraph" });
  await expect(line).toHaveValue("Total 2025");
  await line.fill("Total 2026");
  await page.keyboard.press("Escape");
  const out = await save(page);
  await expect(page.locator(".ed-result")).toContainText("kept their font except for characters it did not include");
  expect(text(out).split("\f")[0]).toContain("Total 2026");
});

test("a paragraph reflows, and the engine's own preview replaces the approximation", async ({ page }) => {
  const w = await watch(page);
  await open(page, "text-3.pdf");
  await page.getByRole("button", { name: "Edit text" }).click();
  // The body paragraph: two 12 pt lines from x = 72, baselines at 702 and 684.
  const p = await at(page, 1, 0.2, (792 - 700) / 792);
  await page.mouse.click(p.x, p.y);
  const box = page.getByRole("textbox", { name: "Edit this paragraph" });
  await expect(box).toHaveValue("The quick brown fox jumps over the lazy dog. Second line of body text.");
  const longer =
    "The quick brown fox jumps over the lazy dog. Second line of body text, now long enough to need a third and a fourth line once it is laid out again across the page width.";
  await box.fill(longer);
  await page.keyboard.press("Escape");
  // First the browser's approximation, then the engine's rendering of the page.
  await expect(page.locator(".ed-para-view")).toHaveCount(0, { timeout: 30_000 });
  const out = await save(page);
  // Reflowed inside the same column: more lines, same left edge.
  const paras = JSON.parse(execFileSync("bun", ["e2e/paragraphs.ts", out]).toString()) as { text: string; lines: number; left: number; right: number }[];
  const body = paras.find((q) => q.text.includes("now long enough"))!;
  expect(body.text).toBe(longer);
  expect(body.lines).toBeGreaterThanOrEqual(4);
  expect(body.left).toBeCloseTo(72, 1);
  expect(body.right).toBeLessThanOrEqual(313);
  await w.assertClean();
});

test("the page's own image moves, then goes", async ({ page }) => {
  // photo.pdf: a photo at (36, 36), 360 × 240 points, on a 432 × 312 page.
  await open(page, "photo.pdf");
  const from = await at(page, 1, 0.5, 0.5);
  const to = await at(page, 1, 0.6, 0.4);
  await page.mouse.move(from.x, from.y);
  await page.mouse.down();
  await page.mouse.move((from.x + to.x) / 2, (from.y + to.y) / 2, { steps: 4 });
  await page.mouse.move(to.x, to.y, { steps: 4 });
  await page.mouse.up();
  await expect(page.locator(".ed-object")).toHaveCount(1);
  let out = await save(page);
  await expect(page.locator(".ed-result")).toContainText("1 image or shape moved or removed");
  expect(qdf(out)).toMatch(/q [\d.-]+ 0 0 [\d.-]+ [\d.-]+ [\d.-]+ cm\s*\n?\/Im0 Do/);
  await page.keyboard.press("Delete");
  out = await save(page);
  // Page 2 keeps its own photo: one drawing of an image is left, not two.
  expect(qdf(out).match(/\/Im0 Do/g)?.length).toBe(1);
});

test("text in any script, and a font asked for when none has the characters", async ({ page }) => {
  await open(page, "text-3.pdf");
  await page.getByRole("button", { name: "Add text" }).click();
  const p = await at(page, 1, 0.12, 0.6);
  await page.mouse.click(p.x, p.y);
  await page.getByRole("textbox", { name: "Text" }).fill("Привет — Ωμέγα — 日本");
  await page.keyboard.press("Escape");
  await page.getByRole("button", { name: "Save PDF" }).click();
  const alert = page.locator(".ed-missing");
  await expect(alert).toContainText("日本", { timeout: 90_000 });
  // A font without those characters is accepted, and the editor asks again.
  await alert.getByLabel("Font file").setInputFiles({
    name: "LiberationMono-Regular.ttf",
    mimeType: "font/ttf",
    buffer: await readFile(new URL("../models/fonts/LiberationMono-Regular.ttf", import.meta.url)),
  });
  await expect(page.locator(".ed-fonts")).toContainText("LiberationMono-Regular.ttf");
  await expect(page.locator(".ed-missing")).toContainText("日本", { timeout: 90_000 });
  // Without the Japanese, it saves.
  await page.getByRole("button", { name: "Select", exact: true }).click();
  await page.locator(".ed-text").dblclick();
  await page.getByRole("textbox", { name: "Text" }).fill("Привет — Ωμέγα");
  await page.keyboard.press("Escape");
  const out = await save(page);
  expect(text(out).split("\f")[0]).toContain("Привет — Ωμέγα");
});

test("from your files to the editor and back", async ({ page }) => {
  await page.goto("./");
  await page.getByLabel("Files to work on").setInputFiles(fixture("text-3.pdf"));
  await page.getByRole("button", { name: "Edit", exact: true }).first().click();
  await expect(page.getByRole("region", { name: "Pages" }).getByRole("img", { name: "Page 1" })).toBeVisible({ timeout: 90_000 });
  await page.getByRole("navigation", { name: "Page order" }).getByRole("button", { name: "Delete page 3" }).click();
  await page.getByRole("button", { name: "Save PDF" }).click();
  await page.getByRole("button", { name: "Back to your files with this version" }).click();
  await expect(page.getByRole("heading", { level: 1, name: "Your files" })).toBeVisible();
  await expect(page.locator(".ws-file").filter({ hasText: "text-3-edited.pdf" })).toContainText("2 pages", { timeout: 60_000 });
});
