// The scanner: camera (Chromium's fake one), imported photos, corners, the
// PDF, and photos shared to the installed app.
import { expect, test, type Page } from "@playwright/test";
import { execFileSync } from "node:child_process";
import { readFile } from "node:fs/promises";
import { fixture, watch } from "./helpers.ts";

test.use({
  launchOptions: { args: ["--use-fake-device-for-media-stream", "--use-fake-ui-for-media-stream"] },
  permissions: ["camera"],
});

const pageCount = (path: string) => Number(execFileSync("qpdf", ["--show-npages", path]).toString().trim());

async function makePdf(page: Page, pages: number) {
  const previews = page.getByRole("list", { name: "Pages" }).locator("img");
  await expect(previews).toHaveCount(pages, { timeout: 30_000 });
  await page.getByRole("button", { name: `Make the PDF (${pages} page${pages > 1 ? "s" : ""})` }).click();
  const result = page.getByRole("region", { name: "Result" });
  await expect(result).toBeVisible({ timeout: 60_000 });
  const [dl] = await Promise.all([page.waitForEvent("download"), result.getByRole("button", { name: "Download" }).click()]);
  const path = (await dl.path())!;
  expect((await readFile(path)).subarray(0, 5).toString()).toBe("%PDF-");
  return path;
}

test("scan two pages with the camera", async ({ page }) => {
  const w = await watch(page);
  await page.goto("./");
  await page.getByRole("link", { name: "Scan paper" }).click();
  await expect(page.getByRole("heading", { level: 1, name: "Scan documents" })).toBeVisible();
  await page.getByRole("button", { name: "Use the camera" }).click();
  const camera = page.getByRole("dialog", { name: "Camera" });
  const shutter = camera.getByRole("button", { name: "Take the picture" });
  await expect(shutter).toBeEnabled({ timeout: 15_000 });
  await shutter.click();
  await expect(camera.getByRole("button", { name: "1 page, review them" })).toBeVisible();
  await shutter.click();
  await camera.getByRole("button", { name: "2 pages, review them" }).click();
  await expect(camera).toBeHidden();

  const pdf = await makePdf(page, 2);
  expect(pageCount(pdf)).toBe(2);
  // The PDF joined your files.
  await expect(page.getByRole("link", { name: /Your files/ })).toContainText("1");
  await w.assertClean();
});

test("imported photos: corners by keyboard, turn, black and white", async ({ page }) => {
  const w = await watch(page);
  await page.goto("./#/scan");
  await page.getByLabel("Photos to scan").setInputFiles([fixture("photo.jpg"), fixture("photo.png")]);
  const list = page.getByRole("list", { name: "Pages" });
  await expect(list.locator("img")).toHaveCount(2, { timeout: 30_000 });

  await page.getByRole("button", { name: "Move page 2 earlier" }).click();
  await page.getByRole("button", { name: "Page 1: adjust the corners" }).click();
  const handle = page.getByRole("button", { name: /^Top-left corner/ });
  await handle.focus();
  await handle.press("Shift+ArrowRight");
  await page.getByRole("button", { name: "Turn" }).click();
  await page.getByRole("button", { name: "Done" }).click();
  await page.getByRole("button", { name: "Black and white" }).click();

  const pdf = await makePdf(page, 2);
  expect(pageCount(pdf)).toBe(2);
  await w.assertClean();
});

test("photos shared from another app open in the scanner", async ({ page }) => {
  await page.goto("./");
  await page.waitForFunction(() => navigator.serviceWorker.controller !== null, null, { timeout: 30_000 });
  const photo = [...(await readFile(fixture("photo.jpg")))];
  // What the system share sheet sends to the manifest's share_target.
  const status = await page.evaluate(async (bytes) => {
    const form = new FormData();
    form.append("files", new File([new Uint8Array(bytes)], "shared.jpg", { type: "image/jpeg" }));
    const res = await fetch("./share-target", { method: "POST", body: form });
    return res.status;
  }, photo);
  expect(status).toBe(200);
  await page.goto("./#/scan?shared");
  await expect(page.getByRole("list", { name: "Pages" }).locator("img")).toHaveCount(1, { timeout: 30_000 });
  expect(page.url()).not.toContain("shared");
  // The inbox is emptied as soon as the page takes the photos.
  expect(await page.evaluate(() => caches.has("pdfvault-inbox"))).toBe(false);
});

test("a PDF shared with a photo takes them both to your files", async ({ page }) => {
  await page.goto("./");
  await page.waitForFunction(() => navigator.serviceWorker.controller !== null, null, { timeout: 30_000 });
  const files = [
    { name: "shared.pdf", type: "application/pdf", bytes: [...(await readFile(fixture("text-3.pdf")))] },
    { name: "shared.jpg", type: "image/jpeg", bytes: [...(await readFile(fixture("photo.jpg")))] },
  ];
  await page.evaluate(async (files) => {
    const form = new FormData();
    for (const f of files) form.append("files", new File([new Uint8Array(f.bytes)], f.name, { type: f.type }));
    await fetch("./share-target", { method: "POST", body: form });
  }, files);
  await page.goto("./#/scan?shared");
  await expect(page.getByRole("heading", { level: 1, name: "Your files" })).toBeVisible({ timeout: 30_000 });
  await expect(page.locator(".ws-file")).toHaveCount(2);
  await expect(page.locator(".ws-file").filter({ hasText: "shared.pdf" })).toContainText("3 pages", { timeout: 60_000 });
  expect(await page.evaluate(() => caches.has("pdfvault-inbox"))).toBe(false);
});
