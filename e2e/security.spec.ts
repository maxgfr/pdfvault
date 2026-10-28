import { expect, test } from "@playwright/test";
import { fixture, runTool, siteURL, watch } from "./helpers.ts";

test("the page sets a strict CSP and no referrer", async ({ page }) => {
  await page.goto("./");
  const csp = await page.locator('meta[http-equiv="Content-Security-Policy"]').getAttribute("content");
  for (const d of ["default-src 'self'", "connect-src 'self'", "object-src 'none'", "form-action 'none'", "require-trusted-types-for 'script'"]) {
    expect(csp).toContain(d);
  }
  expect(await page.locator('meta[name="referrer"]').getAttribute("content")).toBe("no-referrer");
});

test("nothing leaves the origin while working, including the heavy engines", async ({ page }) => {
  const w = await watch(page);
  await page.goto("./");
  await runTool(page, "compress", "Compress PDF", [["photo.pdf"]], { Level: "extreme" });
  await runTool(page, "ocr", "OCR (searchable PDF)", [["scan.pdf"]], { Pages: "1" });
  await w.assertClean();
});

test("the CSP blocks a request to another origin", async ({ page }) => {
  await page.goto("./");
  const blocked = await page.evaluate(async () => {
    try {
      await fetch("https://example.com/");
      return false;
    } catch {
      return true;
    }
  });
  expect(blocked).toBe(true);
});

test("Trusted Types reject a script URL from elsewhere", async ({ page }) => {
  await page.goto("./");
  const threw = await page.evaluate(() => {
    try {
      new Worker("data:text/javascript,1");
      return false;
    } catch {
      return true;
    }
  });
  expect(threw).toBe(true);
});

test("refuses to run inside a frame", async ({ page }) => {
  await page.goto("about:blank");
  await page.setContent(`<iframe src="${siteURL}" width="400" height="300"></iframe>`);
  const frame = page.frames()[1]!;
  await frame.waitForLoadState();
  expect(await frame.locator("#root").count()).toBe(0);
});

test("works offline after the first visit", async ({ page, context }) => {
  await page.goto("./");
  await page.evaluate(() => navigator.serviceWorker.ready);
  await expect(page.getByText("Available offline")).toBeVisible();
  // Use the renderer once so it is cached too.
  await runTool(page, "pdf2img", "PDF to images", [["text-3.pdf"]], { Resolution: "40" });
  await context.setOffline(true);
  await page.reload();
  await runTool(page, "compress", "Compress PDF", [["photo.pdf"]]);
  await runTool(page, "pdf2img", "PDF to images", [["text-3.pdf"]], { Resolution: "40" });
  await context.setOffline(false);
});

test("the OCR model is downloaded only when asked, works offline, and can be deleted", async ({ page, context }) => {
  const models: string[] = [];
  page.on("request", (r) => {
    if (r.url().includes("/models/")) models.push(r.url());
  });
  await page.goto("./");
  await page.evaluate(() => navigator.serviceWorker.ready);
  await page.goto("./#/ocr");
  await page.locator('input[type="file"]').first().setInputFiles(fixture("scan.pdf"));
  await page.getByRole("button", { name: "OCR (searchable PDF)", exact: true }).click();
  const prompt = page.getByRole("alert").filter({ hasText: "31 MB" });
  await expect(prompt).toBeVisible({ timeout: 60_000 });
  expect(models, "nothing downloaded before the user agrees").toEqual([]);
  await prompt.getByRole("button", { name: "Not now" }).click();
  await expect(prompt).toBeHidden();
  expect(models).toEqual([]);

  // Agreeing downloads it once; then OCR runs offline.
  await runTool(page, "ocr", "OCR (searchable PDF)", [["scan.pdf"]], { Pages: "1" });
  expect(models.length).toBe(3);
  await context.setOffline(true);
  await page.reload();
  await runTool(page, "ocr", "OCR (searchable PDF)", [["scan.pdf"]], { Pages: "1" });
  await context.setOffline(false);
  expect(models.length).toBe(3);

  // The Privacy page deletes it, and the next OCR asks again.
  await page.goto("./#/privacy");
  await page.getByRole("button", { name: "Delete" }).click();
  await expect(page.getByText("Not downloaded")).toBeVisible();
  await page.goto("./#/ocr");
  await page.locator('input[type="file"]').first().setInputFiles(fixture("scan.pdf"));
  await page.getByRole("button", { name: "OCR (searchable PDF)", exact: true }).click();
  await expect(page.getByRole("button", { name: "Download and continue" })).toBeVisible({ timeout: 60_000 });
});

test("the OCR model downloads on an origin that reports itself over quota", async ({ page }) => {
  // maxgfr.github.io hosts other apps: storage.estimate() counts them all and
  // can report more used than allowed. That must not stop the download.
  await page.addInitScript(() => {
    navigator.storage.estimate = async () => ({ quota: 2 ** 31, usage: 2 ** 32 });
  });
  await page.goto("./");
  await runTool(page, "ocr", "OCR (searchable PDF)", [["scan.pdf"]], { Pages: "1" });
});
