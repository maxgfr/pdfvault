import { expect, test } from "@playwright/test";
import { runTool, siteURL, watch } from "./helpers.ts";

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
