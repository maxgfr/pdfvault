// The site and the CLI run the same wasm: the same input and options must
// give byte-identical output.
import { expect, test } from "@playwright/test";
import { readFile, rm, mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { execFileSync } from "node:child_process";
import { fixture, runTool, sha256 } from "./helpers.ts";

const cli = process.env.PDFVAULT_BIN ? [process.env.PDFVAULT_BIN] : ["bun", "packages/cli/src/main.ts"];

const cases: { id: string; title: string; files: string[]; ui: Record<string, string>; args: string[] }[] = [
  { id: "compress", title: "Compress PDF", files: ["photo.pdf"], ui: { Level: "strong" }, args: ["--preset", "strong"] },
  { id: "merge", title: "Merge PDFs", files: ["text-3.pdf", "text-5.pdf"], ui: {}, args: [] },
  { id: "watermark", title: "Watermark", files: ["text-3.pdf"], ui: { Text: "DRAFT" }, args: ["--text", "DRAFT"] },
  { id: "pdf2img", title: "PDF to images", files: ["text-3.pdf"], ui: { Resolution: "50" }, args: ["--dpi", "50"] },
  { id: "img-compress", title: "Compress images", files: ["photo.jpg"], ui: { "JPEG quality": "40" }, args: ["--quality", "40"] },
  { id: "to-md", title: "PDF to Markdown", files: ["text-3.pdf"], ui: {}, args: [] },
  { id: "md2pdf", title: "Markdown to PDF", files: ["notes.md"], ui: {}, args: [] },
  { id: "combine", title: "Combine into one PDF", files: ["photo.jpg", "text-3.pdf"], ui: {}, args: [] },
];

test("parity: edit (the editor's page strip)", async ({ page }) => {
  await page.goto("./#/edit");
  await page.getByLabel("PDF to edit").setInputFiles(fixture("text-3.pdf"));
  const order = page.getByRole("navigation", { name: "Page order" });
  await order.getByRole("button", { name: "Turn page 1 right" }).click();
  await order.getByRole("button", { name: "Delete page 3" }).click();
  await page.getByRole("button", { name: "Save PDF" }).click();
  const [dl] = await Promise.all([page.waitForEvent("download"), page.locator(".ed-result").getByRole("button", { name: "Download" }).click()]);
  const bytes = new Uint8Array(await readFile((await dl.path())!));
  const ops = JSON.stringify({ order: [{ page: 0, rotate: 90 }, { page: 1 }] });
  const out = execFileSync(cli[0]!, [...cli.slice(1), "edit", fixture("text-3.pdf"), "--ops", ops, "-o", "-"]);
  expect(sha256(bytes)).toBe(sha256(new Uint8Array(out)));
});

test("parity: scan (detected corners sent back by the page)", async ({ page }) => {
  await page.goto("./#/scan");
  await page.getByLabel("Photos to scan").setInputFiles(fixture("photo.jpg"));
  await expect(page.getByRole("list", { name: "Pages" }).locator("img")).toHaveCount(1, { timeout: 30_000 });
  await page.getByRole("button", { name: "Make the PDF (1 page)" }).click();
  const result = page.getByRole("region", { name: "Result" });
  const [dl] = await Promise.all([page.waitForEvent("download"), result.getByRole("button", { name: "Download" }).click()]);
  const bytes = new Uint8Array(await readFile((await dl.path())!));
  const out = execFileSync(cli[0]!, [...cli.slice(1), "scan", fixture("photo.jpg"), "-o", "-"]);
  expect(sha256(bytes)).toBe(sha256(new Uint8Array(out)));
});

for (const c of cases) {
  test(`parity: ${c.id}`, async ({ page }) => {
    const { bytes } = await runTool(page, c.id, c.title, [c.files], c.ui);
    const dir = await mkdtemp(join(tmpdir(), "pdfvault-parity-"));
    try {
      const out = execFileSync(cli[0]!, [...cli.slice(1), c.id, ...c.files.map(fixture), ...c.args, "-o", `${dir}/`, "--json"]);
      const first = JSON.parse(out.toString()).results[0].outputs[0].path as string;
      const cliBytes = new Uint8Array(await readFile(first));
      expect(sha256(bytes!)).toBe(sha256(cliBytes));
    } finally {
      await rm(dir, { recursive: true, force: true });
    }
  });
}
