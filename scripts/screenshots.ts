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
  await page.goto(`${base}#/edit`);
  await page.getByLabel("PDF to edit").setInputFiles(`${fixtures}form.pdf`);
  const sheet = page.getByRole("img", { name: "Page 1", exact: true });
  await sheet.waitFor();
  await page.getByLabel("Form field name").fill("Grace Hopper");
  const box = (await sheet.boundingBox())!;
  await page.getByRole("button", { name: "Highlight" }).click();
  await page.mouse.move(box.x + box.width * 0.11, box.y + box.height * 0.045);
  await page.mouse.down();
  await page.mouse.move(box.x + box.width * 0.25, box.y + box.height * 0.075, { steps: 5 });
  await page.mouse.up();
  // The page's own title, rewritten in place ("Form": 24 pt on the baseline at y = 720).
  await page.getByRole("button", { name: "Edit text" }).click();
  await page.mouse.click(box.x + box.width * 0.14, box.y + box.height * ((792 - 728) / 792));
  await page.getByRole("textbox", { name: "Edit this line" }).fill("Application form");
  await page.keyboard.press("Escape");
  await page.getByRole("button", { name: "Add text" }).click();
  await page.mouse.click(box.x + box.width * 0.15, box.y + box.height * 0.25);
  await page.keyboard.type("Approuvé — 24 octobre");
  await page.keyboard.press("Escape");
  await page.getByRole("button", { name: "Note" }).click();
  await page.mouse.click(box.x + box.width * 0.8, box.y + box.height * 0.06);
  await page.keyboard.type("Check the date");
  await page.keyboard.press("Escape");
  await page.keyboard.press("Escape");
  await page.waitForTimeout(300);
  await page.screenshot({ path: `${out}editor-${scheme}.png` });
  await page.close();
}
await browser.close();
console.log("screenshots written to docs/");
