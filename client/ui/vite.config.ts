import { sveltekit } from "@sveltejs/kit/vite";
import { defineConfig } from "vitest/config";

export default defineConfig({
  plugins: [sveltekit()],
  // Tauri expects a fixed port and prints its own output.
  clearScreen: false,
  server: { port: 5173, strictPort: true },
  test: { include: ["src/**/*.test.ts"] },
});
