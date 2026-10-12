import { expect, test } from "@playwright/test";
import { runTool } from "./helpers.ts";

test("create, save, lock, unlock, wipe", async ({ page }) => {
  await page.goto("./#/vault");
  await page.getByLabel("Choose a passphrase").fill("correct horse battery");
  await page.getByLabel("Repeat it").fill("correct horse battery");
  await page.getByRole("button", { name: "Create vault" }).click();
  await expect(page.getByRole("button", { name: "Lock now" })).toBeVisible({ timeout: 30_000 });

  await runTool(page, "rotate", "Rotate pages", [["text-3.pdf"]], {}, { download: false });
  await page.getByRole("button", { name: "Save to vault" }).click();
  await expect(page.getByRole("button", { name: "Saved" })).toBeDisabled();

  // Nothing readable is stored: the name only exists encrypted.
  const plaintextLeak = await page.evaluate(
    () =>
      new Promise<boolean>((resolve) => {
        const req = indexedDB.open("pdfvault-vault");
        req.onsuccess = () => {
          const all = req.result.transaction("files").objectStore("files").getAll();
          all.onsuccess = () => {
            req.result.close();
            resolve(JSON.stringify(all.result, (_k, v) => (v instanceof Uint8Array ? Array.from(v) : v)).includes("rotated"));
          };
        };
      }),
  );
  expect(plaintextLeak).toBe(false);

  await page.goto("./#/vault");
  await expect(page.getByText("text-3-rotated.pdf")).toBeVisible();
  await page.getByRole("button", { name: "Lock now" }).click();
  await page.getByLabel("Passphrase").fill("wrong passphrase");
  await page.getByRole("button", { name: "Unlock" }).click();
  await expect(page.getByText("wrong passphrase")).toBeVisible({ timeout: 30_000 });
  await page.getByLabel("Passphrase").fill("correct horse battery");
  await page.getByRole("button", { name: "Unlock" }).click();
  await expect(page.getByText("text-3-rotated.pdf")).toBeVisible({ timeout: 30_000 });

  // Leaving the tab locks it.
  await page.evaluate(() => {
    Object.defineProperty(document, "visibilityState", { value: "hidden", configurable: true });
    document.dispatchEvent(new Event("visibilitychange"));
  });
  await expect(page.getByRole("button", { name: "Unlock" })).toBeVisible();

  await page.getByRole("button", { name: "Wipe the vault" }).click();
  await page.getByRole("button", { name: "Yes, wipe everything" }).click();
  await expect(page.getByRole("button", { name: "Create vault" })).toBeVisible();
});
