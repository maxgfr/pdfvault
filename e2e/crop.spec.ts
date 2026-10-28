// The crop box on Crop pages and Crop & rotate images: dragged, it writes the
// numeric fields, and the run crops what the box shows.
import { expect, test, type Locator, type Page } from "@playwright/test";
import { execFileSync } from "node:child_process";
import { readFile } from "node:fs/promises";
import { fixture, sha256, watch } from "./helpers.ts";

const cli = process.env.PDFVAULT_BIN ? [process.env.PDFVAULT_BIN] : ["bun", "packages/cli/src/main.ts"];
const run = (...args: string[]) => execFileSync(cli[0]!, [...cli.slice(1), ...args]);
const pageSize = (file: string) => (JSON.parse(run("info", file, "--json").toString()) as { results: { report: { pageSizes: { width: number; height: number }[] } }[] }).results[0]!.report.pageSizes[0]!;

async function dragBy(page: Page, handle: Locator, dx: number, dy: number) {
  await handle.scrollIntoViewIfNeeded();
  const b = (await handle.boundingBox())!;
  await page.mouse.move(b.x + b.width / 2, b.y + b.height / 2);
  await page.mouse.down();
  await page.mouse.move(b.x + b.width / 2 + dx / 2, b.y + b.height / 2 + dy / 2);
  await page.mouse.move(b.x + b.width / 2 + dx, b.y + b.height / 2 + dy);
  await page.mouse.up();
}

const value = async (page: Page, label: string) => Number(await page.getByLabel(label, { exact: true }).inputValue());

test("crop pages: drag the box, the margins follow and the page shrinks", async ({ page }) => {
  const w = await watch(page);
  await page.goto("./#/crop");
  await page.locator('input[type="file"]').setInputFiles(fixture("text-3.pdf"));
  await expect(page.locator(".crop-tool img")).toBeVisible({ timeout: 90_000 });

  await dragBy(page, page.getByRole("button", { name: /^Top-left corner/ }), 60, 80);
  const [top, left] = [await value(page, "Top"), await value(page, "Left")];
  expect(top).toBeGreaterThan(0);
  expect(left).toBeGreaterThan(0);
  // Keyboard: Shift+Arrow moves a corner 10 units.
  await page.getByRole("button", { name: /^Bottom-right corner/ }).press("Shift+ArrowLeft");
  expect(await value(page, "Right")).toBe(10);
  // The fields stay the precise way in, and move the box.
  const before = pageSize(fixture("text-3.pdf"));
  await page.getByLabel("Bottom", { exact: true }).fill("30");
  const box = page.getByRole("button", { name: /^Crop box/ });
  await expect.poll(async () => Number(await box.getAttribute("height"))).toBeCloseTo(before.height - top - 30, 1);

  await page.getByRole("button", { name: "Crop pages", exact: true }).click();
  const results = page.getByRole("region", { name: "Results" });
  const [dl] = await Promise.all([page.waitForEvent("download"), results.getByRole("button", { name: "Download", exact: true }).click({ timeout: 90_000 })]);
  const out = (await dl.path())!;
  const after = pageSize(out);
  expect(after.width).toBeCloseTo(before.width - left - 10, 1);
  expect(after.height).toBeCloseTo(before.height - top - 30, 1);
  // Same bytes as the CLI given the numbers the box wrote.
  const cliOut = run("crop", fixture("text-3.pdf"), "--top", String(top), "--right", "10", "--bottom", "30", "--left", String(left), "-o", "-");
  expect(sha256(new Uint8Array(await readFile(out)))).toBe(sha256(new Uint8Array(cliOut)));
  await w.assertClean();
});

test("crop & rotate images: drag the box, the rectangle follows and the image is cropped", async ({ page }) => {
  const w = await watch(page);
  await page.goto("./#/img-edit");
  await page.locator('input[type="file"]').setInputFiles(fixture("photo.jpg"));
  await expect(page.locator(".crop-tool img")).toBeVisible();
  // Untouched, nothing is cropped.
  await expect(page.getByLabel("Crop width", { exact: true })).toHaveValue("");

  await dragBy(page, page.getByRole("button", { name: /^Bottom-right corner/ }), -80, -60);
  await dragBy(page, page.getByRole("button", { name: /^Crop box/ }), 20, 20);
  const [x, y, width, height] = [await value(page, "Crop from left"), await value(page, "Crop from top"), await value(page, "Crop width"), await value(page, "Crop height")];
  expect(x).toBeGreaterThan(0);
  expect(y).toBeGreaterThan(0);
  expect(width).toBeGreaterThan(0);
  expect(height).toBeGreaterThan(0);

  await page.getByRole("button", { name: "Crop & rotate images", exact: true }).click();
  const report = page.getByRole("region", { name: "Results" }).locator(".report");
  await expect(report).toContainText(`width${width}`, { timeout: 90_000 });
  await expect(report).toContainText(`height${height}`);
  await w.assertClean();
});
