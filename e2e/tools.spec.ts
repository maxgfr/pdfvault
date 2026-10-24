// Every tool, once, through the real UI: drop files, set options, run, download.
import { expect, test } from "@playwright/test";
import { execFileSync } from "node:child_process";
import { fixture, isPdf, runTool, watch } from "./helpers.ts";

type Case = { id: string; title: string; slots: string[][]; fill?: Record<string, string>; expect: "pdf" | "png" | "jpg" | "webp" | "text" | "epub" | "report" };

const cases: Case[] = [
  { id: "compress", title: "Compress PDF", slots: [["photo.pdf"]], fill: { Level: "strong" }, expect: "pdf" },
  { id: "merge", title: "Merge PDFs", slots: [["text-3.pdf", "text-5.pdf"]], expect: "pdf" },
  { id: "combine", title: "Combine into one PDF", slots: [["photo.jpg", "text-3.pdf"]], expect: "pdf" },
  { id: "split", title: "Split PDF", slots: [["text-5.pdf"]], fill: { "Pages per file": "2" }, expect: "pdf" },
  { id: "extract", title: "Extract pages", slots: [["text-5.pdf"]], fill: { "Pages to keep": "2-3" }, expect: "pdf" },
  { id: "delete", title: "Delete pages", slots: [["text-5.pdf"]], fill: { "Pages to delete": "1" }, expect: "pdf" },
  { id: "reorder", title: "Reorder pages", slots: [["text-3.pdf"]], fill: { "New order": "3,1,2" }, expect: "pdf" },
  { id: "rotate", title: "Rotate pages", slots: [["text-3.pdf"]], fill: { Rotation: "180" }, expect: "pdf" },
  { id: "blank", title: "Insert blank pages", slots: [["text-3.pdf"]], fill: { "After pages": "1" }, expect: "pdf" },
  { id: "crop", title: "Crop pages", slots: [["text-3.pdf"]], fill: { Top: "40" }, expect: "pdf" },
  { id: "nup", title: "Pages per sheet", slots: [["text-5.pdf"]], fill: { "Pages per sheet": "4" }, expect: "pdf" },
  { id: "img2pdf", title: "Images to PDF", slots: [["photo.jpg", "signature.png"]], expect: "pdf" },
  { id: "pdf2img", title: "PDF to images", slots: [["text-3.pdf"]], fill: { Resolution: "50" }, expect: "png" },
  { id: "watermark", title: "Watermark", slots: [["text-3.pdf"]], fill: { Text: "DRAFT" }, expect: "pdf" },
  { id: "number", title: "Page numbers", slots: [["text-3.pdf"]], fill: { Format: "Page {n} of {total}" }, expect: "pdf" },
  { id: "header-footer", title: "Header & footer", slots: [["text-3.pdf"]], fill: { "Header left": "ACME" }, expect: "pdf" },
  { id: "sign", title: "Sign (visual)", slots: [["text-3.pdf"], ["signature.png"]], expect: "pdf" },
  { id: "meta", title: "Edit metadata", slots: [["text-3.pdf"]], fill: { Title: "New title" }, expect: "pdf" },
  { id: "encrypt", title: "Protect with password", slots: [["text-3.pdf"]], fill: { "Password to open": "s3cret" }, expect: "pdf" },
  { id: "permissions", title: "Restrict permissions", slots: [["text-3.pdf"]], fill: { "Owner password": "boss" }, expect: "pdf" },
  { id: "decrypt", title: "Remove password", slots: [["locked.pdf"]], fill: { Password: "secret" }, expect: "pdf" },
  { id: "flatten", title: "Flatten forms", slots: [["form.pdf"]], expect: "pdf" },
  { id: "repair", title: "Repair PDF", slots: [["text-3.pdf"]], expect: "pdf" },
  { id: "compare", title: "Compare PDFs", slots: [["text-3.pdf"], ["text-5.pdf"]], expect: "report" },
  { id: "ocr", title: "OCR (searchable PDF)", slots: [["scan.pdf"]], fill: { Pages: "1" }, expect: "pdf" },
  { id: "to-md", title: "PDF to Markdown", slots: [["text-3.pdf"]], expect: "text" },
  { id: "to-text", title: "PDF to text", slots: [["scan.pdf"]], expect: "text" },
  { id: "to-epub", title: "PDF to EPUB", slots: [["text-3.pdf"]], expect: "epub" },
  { id: "info", title: "PDF info", slots: [["photo.pdf"]], expect: "report" },
  { id: "img-compress", title: "Compress images", slots: [["photo.jpg"]], fill: { "JPEG quality": "40" }, expect: "jpg" },
  { id: "img-resize", title: "Resize images", slots: [["photo.png"]], fill: { "Max width": "200" }, expect: "png" },
  { id: "img-convert", title: "Convert images", slots: [["photo.jpg"]], fill: { To: "webp" }, expect: "webp" },
  { id: "img-strip", title: "Remove image metadata", slots: [["photo.jpg"]], expect: "jpg" },
  { id: "img-edit", title: "Crop & rotate images", slots: [["photo.jpg"]], fill: { Rotate: "90" }, expect: "jpg" },
];

const magic: Record<string, (b: Uint8Array) => boolean> = {
  pdf: isPdf,
  png: (b) => b[0] === 0x89 && b[1] === 0x50,
  jpg: (b) => b[0] === 0xff && b[1] === 0xd8,
  webp: (b) => Buffer.from(b.subarray(8, 12)).toString() === "WEBP",
  epub: (b) => Buffer.from(b.subarray(30, 38)).toString() === "mimetype",
  text: (b) => b.length > 0 && !b.includes(0),
};

test("every tool in the registry is covered", async ({ page }) => {
  await page.goto("./");
  const ids = await page.locator(".card").evaluateAll((as) => as.map((a) => (a as HTMLAnchorElement).hash.slice(2)));
  // The editor and the viewer have their own pages and specs.
  expect(ids.sort()).toEqual([...cases.map((c) => c.id), "edit", "view"].sort());
});

for (const c of cases) {
  test(`${c.id}`, async ({ page }) => {
    const w = await watch(page);
    if (c.expect === "report") {
      const { results } = await runTool(page, c.id, c.title, c.slots, c.fill, { download: false });
      await expect(results.locator(".report")).toBeVisible();
    } else {
      const { bytes, name } = await runTool(page, c.id, c.title, c.slots, c.fill);
      expect(bytes!.length).toBeGreaterThan(0);
      expect(magic[c.expect]!(bytes!), `${name} looks like ${c.expect}`).toBe(true);
    }
    await w.assertClean();
  });
}

test("a locked PDF asks for its password inline", async ({ page }) => {
  await page.goto("./#/compress");
  await page.locator('input[type="file"]').setInputFiles("fixtures/generated/locked.pdf");
  await page.getByRole("button", { name: "Compress PDF", exact: true }).click();
  await expect(page.getByText("This PDF is locked. Its password:")).toBeVisible();
  await page.getByLabel("This PDF is locked. Its password:").fill("secret");
  await page.getByRole("button", { name: "Compress PDF", exact: true }).click();
  await expect(page.getByRole("button", { name: "Download", exact: true })).toBeVisible();
});

test("split offers a ZIP of all parts", async ({ page }) => {
  await runTool(page, "split", "Split PDF", [["text-5.pdf"]], { "Pages per file": "1" }, { download: false });
  const [dl] = await Promise.all([page.waitForEvent("download"), page.getByRole("button", { name: "Download all as ZIP" }).click()]);
  expect(dl.suggestedFilename()).toBe("split-results.zip");
  const listing = execFileSync("unzip", ["-l", (await dl.path())!]).toString();
  for (let i = 1; i <= 5; i++) expect(listing).toContain(`text-5-part-${i}-${i}.pdf`);
});

test("the viewer shows a dropped PDF page by page", async ({ page }) => {
  const w = await watch(page);
  await page.goto("./#/view");
  await expect(page.getByRole("heading", { level: 1, name: "PDF viewer" })).toBeVisible();
  await page.locator('input[type="file"]').setInputFiles(fixture("text-3.pdf"));
  const pages = page.getByRole("region", { name: "Pages" }).getByRole("img");
  await expect(pages).toHaveCount(3, { timeout: 90_000 });
  await expect(pages.first()).toHaveAttribute("alt", "Page 1");
  await page.getByRole("button", { name: "Zoom in" }).click();
  await expect(page.getByRole("group", { name: "Zoom" })).toContainText("125%");
  await w.assertClean();
});

test("the viewer asks for a locked PDF's password", async ({ page }) => {
  await page.goto("./#/view");
  await page.locator('input[type="file"]').setInputFiles(fixture("locked.pdf"));
  await page.getByLabel("This PDF is locked. Its password:").fill("secret");
  await page.getByRole("button", { name: "Open", exact: true }).click();
  await expect(page.getByRole("region", { name: "Pages" }).getByRole("img").first()).toBeVisible({ timeout: 90_000 });
});
