// Starting from files: drop a mix on the home page, act on the selection,
// keep results in place and chain the next action.
import { expect, test, type Page } from "@playwright/test";
import { readFile } from "node:fs/promises";
import { fixture, isPdf, watch } from "./helpers.ts";

async function drop(page: Page, files: (string | { name: string; mimeType: string; buffer: Buffer })[]) {
  // Playwright takes paths or buffers, not both: read the fixtures.
  const payload = await Promise.all(
    files.map(async (f) => (typeof f === "string" ? { name: f, mimeType: "application/octet-stream", buffer: await readFile(fixture(f)) } : f)),
  );
  await page.getByLabel(/^(Files to work on|Add files)$/).first().setInputFiles(payload);
}

const card = (page: Page, name: string | RegExp) => page.locator(".ws-file").filter({ hasText: name });

test("one compress entry on the home page, before any file", async ({ page }) => {
  await page.goto("./");
  await expect(page.getByRole("button", { name: "Choose files" })).toBeVisible();
  await expect(page.getByText(/Compress a PDF/)).toHaveCount(0);
  await expect(page.getByRole("link", { name: /^Compress PDF/ })).toHaveCount(1);
});

// A 16×16 AVIF: Chromium decodes it, so it takes the path a HEIC takes in Safari.
const AVIF =
  "AAAAIGZ0eXBhdmlmAAAAAGF2aWZtaWYxbWlhZk1BMUIAAAD5bWV0YQAAAAAAAAAvaGRscgAAAAAAAAAAcGljdAAAAAAAAAAAAAAAAFBpY3R1cmVIYW5kbGVyAAAAAA5waXRtAAAAAAABAAAAHmlsb2MAAAAARAAAAQABAAAAAQAAASEAAAAkAAAAKGlpbmYAAAAAAAEAAAAaaW5mZQIAAAAAAQAAYXYwMUNvbG9yAAAAAGppcHJwAAAAS2lwY28AAAAUaXNwZQAAAAAAAAAQAAAAEAAAABBwaXhpAAAAAAMICAgAAAAMYXYxQ4EADAAAAAATY29scm5jbHgAAgACAAIAAAAAF2lwbWEAAAAAAAAAAQABBAECgwQAAAAsbWRhdAoKAgAABUz/xq+QBDIWEACXgBBAggAAEABcEytNbG+TY8nR6A==";

test("photos the browser decodes become JPEG; a HEIC it cannot says what to do", async ({ page }) => {
  await page.goto("./");
  await drop(page, [
    { name: "IMG_0001.avif", mimeType: "image/avif", buffer: Buffer.from(AVIF, "base64") },
    // Chromium has no HEIC decoder: only the brand matters here.
    { name: "IMG_0002.HEIC", mimeType: "image/heic", buffer: Buffer.from("\0\0\0\x18ftypheic\0\0\0\0mif1heic") },
  ]);
  await expect(page.locator(".ws-file")).toHaveCount(2);
  await expect(card(page, "IMG_0001.jpg").getByRole("checkbox")).toBeEnabled();
  await expect(card(page, "IMG_0002.HEIC")).toContainText("This browser cannot read HEIC photos. Open pdfvault in Safari");
  await expect(card(page, "IMG_0002.HEIC").getByRole("checkbox")).toBeDisabled();
});

test("a mixed drop: compress each file with the right tool, keep the results, chain", async ({ page }) => {
  const w = await watch(page);
  await page.goto("./");
  await drop(page, ["photo.jpg", "text-3.pdf", { name: "report.docx", mimeType: "application/octet-stream", buffer: Buffer.from("PK\x03\x04 not really a document") }]);
  await expect(page.getByRole("heading", { level: 1, name: "Your files" })).toBeVisible();
  await expect(page.locator(".ws-file")).toHaveCount(3);
  await expect(card(page, "report.docx")).toContainText("Export it to PDF");
  await expect(card(page, "report.docx").getByRole("checkbox")).toBeDisabled();
  await expect(card(page, "text-3.pdf")).toContainText("3 pages", { timeout: 60_000 });

  await page.getByRole("button", { name: "Compress", exact: true }).click();
  const panel = page.locator(".ws-panel");
  await expect(panel.locator("legend")).toHaveText(["1 PDF", "1 image"]);
  await panel.getByRole("button", { name: "Compress", exact: true }).click();
  const results = panel.getByRole("region", { name: "Results" });
  await expect(results.locator(".outcome")).toHaveCount(2, { timeout: 90_000 });
  await expect(results.locator(".error-text")).toHaveCount(0);
  await results.getByRole("button", { name: "Keep all results in your files" }).click();
  await expect(page.locator(".ws-file .badge", { hasText: "Changed" })).toHaveCount(2);
  await expect(card(page, "text-3-compressed.pdf")).toBeVisible();

  // Chain: protect only the compressed PDF.
  await card(page, /photo/).getByRole("checkbox").uncheck();
  await page.getByRole("button", { name: "Protect", exact: true }).click();
  await panel.getByLabel("Password to open", { exact: true }).fill("s3cret");
  await panel.getByRole("button", { name: "Protect", exact: true }).click();
  const [dl] = await Promise.all([page.waitForEvent("download"), results.getByRole("button", { name: "Download", exact: true }).click()]);
  const bytes = new Uint8Array(await readFile((await dl.path())!));
  expect(isPdf(bytes)).toBe(true);
  expect(dl.suggestedFilename()).toBe("text-3-compressed-protected.pdf");

  await card(page, "text-3-compressed.pdf").getByRole("button", { name: "Restore original" }).click();
  await expect(card(page, "text-3.pdf")).toBeVisible();
  await w.assertClean();
});

test("photos and PDFs combine into one PDF, in the order shown", async ({ page }) => {
  await page.goto("./");
  await drop(page, ["text-3.pdf", "photo.jpg", "signature.png"]);
  await card(page, "photo.jpg").getByRole("button", { name: "Move photo.jpg up" }).click();
  await expect(page.locator(".ws-file .name").first()).toHaveText("photo.jpg");
  await page.getByRole("button", { name: "Combine into one PDF" }).first().click();
  const panel = page.locator(".ws-panel");
  await panel.getByRole("button", { name: "Combine into one PDF", exact: true }).click();
  const results = panel.getByRole("region", { name: "Results" });
  await expect(results.getByText("photo-combined.pdf")).toBeVisible({ timeout: 90_000 });
  await results.getByRole("button", { name: "Add to your files" }).click();
  await expect(page.locator(".ws-file")).toHaveCount(4);
});

test("every registry tool is reachable from a selection", async ({ page }) => {
  await page.goto("./");
  await drop(page, ["text-3.pdf"]);
  await page.locator(".ws-more summary").click();
  for (const name of ["Merge PDFs", "Split PDF", "OCR (searchable PDF)", "PDF to Markdown", "Remove password", "PDF info"]) {
    if (name === "Merge PDFs") await expect(page.locator(".ws-more").getByRole("button", { name })).toHaveCount(0);
    else await expect(page.locator(".ws-more").getByRole("button", { name })).toBeVisible();
  }
  await drop(page, ["text-5.pdf"]);
  await expect(page.locator(".ws-more").getByRole("button", { name: "Merge PDFs" })).toBeVisible();
  await expect(page.locator(".ws-more").getByRole("button", { name: "Compare PDFs" })).toBeVisible();
});

test("viewing and editing are cards of their own", async ({ page }) => {
  await page.goto("./");
  await expect(page.getByRole("link", { name: /^View PDF/ })).toHaveAttribute("href", "#/view");
  await expect(page.getByRole("link", { name: /^Edit PDF/ })).toHaveCount(1);
});

test("a dropped PDF opens in the viewer, then the editor", async ({ page }) => {
  await page.goto("./");
  await drop(page, ["text-3.pdf"]);
  await expect(card(page, "text-3.pdf")).toContainText("3 pages", { timeout: 60_000 });
  await card(page, "text-3.pdf").getByRole("button", { name: "View text-3.pdf" }).click();
  await expect(page.getByRole("heading", { level: 1, name: "PDF viewer" })).toBeVisible();
  await expect(page.getByRole("img", { name: "Page 3" })).toBeVisible({ timeout: 60_000 });
  await page.getByRole("button", { name: "Edit", exact: true }).click();
  await expect(page.getByRole("region", { name: "Pages" }).getByRole("img", { name: "Page 1" })).toBeVisible({ timeout: 60_000 });
  await page.getByRole("link", { name: "Your files" }).first().click();
  // Still one file: the viewer handed the same one to the editor.
  await expect(page.locator(".ws-file")).toHaveCount(1);
});

test("pasting a screenshot adds it", async ({ page }) => {
  await page.goto("./");
  const png = (await readFile(fixture("signature.png"))).toString("base64");
  await page.evaluate((b64) => {
    const bytes = Uint8Array.from(atob(b64), (c) => c.charCodeAt(0));
    const dt = new DataTransfer();
    dt.items.add(new File([bytes], "image.png", { type: "image/png" }));
    document.body.dispatchEvent(new ClipboardEvent("paste", { clipboardData: dt, bubbles: true }));
  }, png);
  await expect(card(page, "image.png")).toBeVisible();
  await expect(page.getByRole("button", { name: "Crop & rotate" })).toBeVisible();
});

test("a locked PDF unlocks in its card", async ({ page }) => {
  await page.goto("./");
  await drop(page, ["locked.pdf"]);
  const c = card(page, "locked.pdf");
  await expect(c.getByText("Locked", { exact: true })).toBeVisible({ timeout: 60_000 });
  await c.getByLabel("Password").fill("secret");
  await c.getByRole("button", { name: "Unlock" }).click();
  await expect(c).toContainText(/\d+ pages?/, { timeout: 60_000 });
  await page.getByRole("button", { name: "Compress", exact: true }).click();
  await page.locator(".ws-panel").getByRole("button", { name: "Compress", exact: true }).click();
  await expect(page.locator(".ws-panel").getByRole("button", { name: "Download", exact: true })).toBeVisible({ timeout: 90_000 });
});

test("files opened with the installed app join your files", async ({ page }) => {
  const w = await watch(page);
  // Playwright cannot open a file with an installed app: stand in for the
  // browser's launchQueue, handing the page file handles as it would.
  const files = [
    { name: "opened.pdf", bytes: [...(await readFile(fixture("text-3.pdf")))] },
    { name: "opened.jpg", bytes: [...(await readFile(fixture("photo.jpg")))] },
  ];
  await page.addInitScript((files) => {
    const handles = files.map((f) => ({ kind: "file", name: f.name, getFile: async () => new File([new Uint8Array(f.bytes)], f.name) }));
    // Chromium has its own launchQueue, a read-only getter: shadow it.
    Object.defineProperty(window, "launchQueue", { value: { setConsumer: (fn: (p: { files: unknown[] }) => void) => fn({ files: handles }) } });
  }, files);
  await page.goto("./");
  await expect(page.getByRole("heading", { level: 1, name: "Your files" })).toBeVisible();
  await expect(page.locator(".ws-file")).toHaveCount(2);
  await expect(card(page, "opened.pdf")).toContainText("3 pages", { timeout: 60_000 });
  await w.assertClean();
});
