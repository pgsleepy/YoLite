import { defineConfig, devices } from "@playwright/test";

export default defineConfig({
  testDir: "./tests",
  reporter: "list",
  use: {
    baseURL: "http://127.0.0.1:4173",
    trace: "on-first-retry",
  },
  webServer: {
    command: "npm start",
    url: "http://127.0.0.1:4173",
    reuseExistingServer: true,
    timeout: 15_000,
  },
  projects: [
    {
      name: "chromium",
      use: { ...devices["Desktop Chrome"] },
    },
  ],
});
