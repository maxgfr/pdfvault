import { expect, type Page } from "@playwright/test";
import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";

export const siteURL = process.env.PDFVAULT_URL ?? "http://localhost:4173/pdfvault/";
const siteOrigin = new URL(siteURL).origin;

export const fixtures = new URL("../fixtures/generated/", import.meta.url).pathname;
export const fixture = (name: string) => `${fixtures}${name}`;

export const sha256 = (b: Uint8Array) => createHash("sha256").update(b).digest("hex");

/** Records every request leaving the page's origin, and every CSP violation. */
export async function watch(page: Page) {
  const external: string[] = [];
  page.on("request", (r) => {
    const u = new URL(r.url());
    if (![siteOrigin, "null"].includes(u.origin) && !["blob:", "data:"].includes(u.protocol)) external.push(r.url());
  });
  await page.addInitScript(() => {
    (window as unknown as { __csp: string[] }).__csp = [];
    document.addEventListener("securitypolicyviolation", (e) =>
      (window as unknown as { __csp: string[] }).__csp.push(`${e.violatedDirective} ${e.blockedURI}`),
    );
  });
  const errors: string[] = [];
  page.on("pageerror", (e) => errors.push(e.message));
  return {
    async assertClean() {
      expect(external, "requests to other origins").toEqual([]);
      expect(await page.evaluate(() => (window as unknown as { __csp: string[] }).__csp), "CSP violations").toEqual([]);
      expect(errors, "page errors").toEqual([]);
    },
  };
}

/** Upload files into the tool's slots, set options by label, run, and return the first download. */
export async function runTool(
  page: Page,
  id: string,
  title: string,
  slots: string[][],
  fill: Record<string, string> = {},
  opts: { download?: boolean } = {},
) {
  await page.goto(`./#/${id}`);
  await expect(page.getByRole("heading", { level: 1, name: title })).toBeVisible();
  const inputs = page.locator('input[type="file"]');
  for (const [i, files] of slots.entries()) await inputs.nth(i).setInputFiles(files.map(fixture));
  const more = page.locator("details.more");
  for (const [label, value] of Object.entries(fill)) {
    let field = page.getByLabel(label, { exact: true });
    if (!(await field.isVisible()) && (await more.count())) {
      await more.locator("summary").click();
      field = page.getByLabel(label, { exact: true });
    }
    const tag = await field.evaluate((el) => (el as HTMLElement).tagName + ":" + (el as HTMLInputElement).type);
    if (tag.startsWith("SELECT")) await field.selectOption(value);
    else if (tag.endsWith("checkbox")) await (value === "true" ? field.check() : field.uncheck());
    else await field.fill(value);
  }
  await page.getByRole("button", { name: title, exact: true }).click();
  const results = page.getByRole("region", { name: "Results" });
  // A fresh browser has no OCR model: agree to the download, as a user would.
  const getModel = page.getByRole("button", { name: "Download and continue" });
  await expect(results.or(getModel)).toBeVisible({ timeout: 90_000 });
  if (await getModel.isVisible()) await getModel.click();
  await expect(results).toBeVisible({ timeout: 90_000 });
  await expect(results.locator(".error-text")).toHaveCount(0);
  if (opts.download === false) return { results, bytes: null };
  const [download] = await Promise.all([page.waitForEvent("download"), results.getByRole("button", { name: "Download", exact: true }).first().click()]);
  const bytes = new Uint8Array(await readFile((await download.path())!));
  return { results, bytes, name: download.suggestedFilename() };
}

export const isPdf = (b: Uint8Array) => Buffer.from(b.subarray(0, 5)).toString() === "%PDF-";
