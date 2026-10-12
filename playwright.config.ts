import { defineConfig, devices } from "@playwright/test";

// PDFVAULT_URL runs the suite against a deployed site instead of a local build.
const remote = process.env.PDFVAULT_URL;
export const baseURL = remote ?? "http://localhost:4173/pdfvault/";

export default defineConfig({
  testDir: "e2e",
  timeout: 120_000,
  fullyParallel: true,
  workers: process.env.CI ? 2 : undefined,
  retries: process.env.CI ? 1 : 0,
  reporter: process.env.CI ? [["github"], ["list"]] : "list",
  use: { baseURL, acceptDownloads: true, trace: "retain-on-failure" },
  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"] } }],
  webServer: remote
    ? undefined
    : {
        command: "bun packages/web/build.ts --serve",
        url: "http://localhost:4173/pdfvault/",
        reuseExistingServer: !process.env.CI,
        timeout: 180_000,
      },
});
