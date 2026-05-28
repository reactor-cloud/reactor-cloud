import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    globals: true,
    environment: "jsdom",
    include: ["tests/**/*.test.ts"],
    exclude: ["e2e/**/*", "node_modules/**/*"],
    coverage: {
      reporter: ["text", "json", "html"],
    },
  },
});
