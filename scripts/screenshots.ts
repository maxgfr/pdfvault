// README screenshots from the built site: bun packages/web/build.ts --serve, then this.
import { chromium } from "@playwright/test";

const out = new URL("../docs/", import.meta.url).pathname;
const fixtures = new URL("../fixtures/generated/", import.meta.url).pathname;
const base = "http://localhost:4173/pdfvault/";
const browser = await chromium.launch();
for (const scheme of ["light", "dark"] as const) {
  const page = await browser.newPage({ viewport: { width: 1200, height: 820 }, deviceScaleFactor: 2, colorScheme: scheme });
  await page.goto(base);
  await page.waitForTimeout(400);
  await page.screenshot({ path: `${out}home-${scheme}.png` });
  await page.goto(`${base}#/compress`);
  await page.locator('input[type="file"]').setInputFiles(`${fixtures}photo.pdf`);
  await page.getByLabel("Level").selectOption("strong");
  await page.getByRole("button", { name: "Compress PDF", exact: true }).click();
  await page.getByRole("region", { name: "Results" }).waitFor();
  await page.waitForTimeout(300);
  await page.screenshot({ path: `${out}compress-${scheme}.png`, fullPage: true });
  await page.close();
}
await browser.close();
console.log("screenshots written to docs/");
