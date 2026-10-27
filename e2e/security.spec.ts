import { expect, test } from "@playwright/test";
import { fixture, runTool, siteURL, watch } from "./helpers.ts";

test("the page sets a strict CSP and no referrer", async ({ page }) => {
  await page.goto("./");
  const csp = await page.locator('meta[http-equiv="Content-Security-Policy"]').getAttribute("content");
  // Hugging Face only for GLM-OCR, which the user downloads from there after agreeing.
  for (const d of ["default-src 'self';", "connect-src 'self' https://huggingface.co https://*.hf.co;", "object-src 'none'", "form-action 'none'", "require-trusted-types-for 'script'"]) {
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
  await expect(prompt).toContainText("download from this site");
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
  const stored = page.locator("ul.files > li");
  const paddle = stored.filter({ hasText: "PP-OCRv6" });
  await paddle.getByRole("button", { name: "Delete" }).click();
  await expect(paddle).toContainText("Not downloaded");
  await expect(stored.filter({ hasText: "GLM-OCR" })).toContainText("Not downloaded");
  await page.goto("./#/ocr");
  await page.locator('input[type="file"]').first().setInputFiles(fixture("scan.pdf"));
  await page.getByRole("button", { name: "OCR (searchable PDF)", exact: true }).click();
  await expect(page.getByRole("button", { name: "Download and continue" })).toBeVisible({ timeout: 60_000 });
});

test("GLM-OCR is offered for text outputs and asks before anything goes to Hugging Face", async ({ page }) => {
  const w = await watch(page);
  // Pretend WebGPU works (CI has no GPU): the consent card comes before any GPU work, and it is not clicked.
  await page.addInitScript(() => Object.defineProperty(navigator, "gpu", { value: { requestAdapter: async () => ({}) } }));
  await page.goto("./");
  await page.evaluate(() => navigator.serviceWorker.ready);
  await page.goto("./#/to-md");
  await page.locator('input[type="file"]').first().setInputFiles(fixture("scan.pdf"));
  const engine = page.getByLabel("Text recognition", { exact: true });
  await expect(engine).toHaveValue("paddle");
  await expect(engine.locator("option")).toHaveText(["PP-OCRv6 · fast", "GLM-OCR · tables and formulas, slow"]);
  // Only with OCR on.
  await page.getByLabel("OCR pages without text", { exact: true }).uncheck();
  await expect(engine).toBeHidden();
  await page.getByLabel("OCR pages without text", { exact: true }).check();
  await engine.selectOption("glm");
  await page.getByRole("button", { name: "PDF to Markdown", exact: true }).click();
  const prompt = page.getByRole("alert").filter({ hasText: "GLM-OCR" });
  await expect(prompt).toBeVisible({ timeout: 60_000 });
  await expect(prompt).toContainText("742 MB");
  await expect(prompt).toContainText("download from huggingface.co");
  await prompt.getByRole("button", { name: "Not now" }).click();
  await expect(prompt).toBeHidden();
  await w.assertClean();
});

test("GLM-OCR cannot be picked where WebGPU is missing", async ({ page }) => {
  await page.addInitScript(() => Object.defineProperty(navigator, "gpu", { value: undefined }));
  await page.goto("./#/to-text");
  await expect(page.getByRole("option", { name: /GLM-OCR/ })).toBeDisabled();
  await expect(page.getByText("GLM-OCR needs WebGPU")).toBeVisible();
  await page.goto("./#/ocr");
  await expect(page.getByLabel("Text recognition", { exact: true })).toHaveCount(0);
});
