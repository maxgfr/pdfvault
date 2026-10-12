// Render public/icon.svg to the PNG sizes the web manifest and iOS need.
// Run after changing the SVG: bun scripts/icons.ts (outputs are committed).
import { chromium } from "@playwright/test";

const dir = new URL("../packages/web/public/", import.meta.url).pathname;
const svg = await Bun.file(`${dir}icon.svg`).text();
const browser = await chromium.launch();
const page = await browser.newPage();
for (const size of [192, 512]) {
  await page.setViewportSize({ width: size, height: size });
  await page.setContent(`<style>html,body{margin:0;background:transparent}svg{width:${size}px;height:${size}px;display:block}</style>${svg}`);
  await page.locator("svg").screenshot({ path: `${dir}icon-${size}.png`, omitBackground: true });
}
await browser.close();
console.log("icons written");
