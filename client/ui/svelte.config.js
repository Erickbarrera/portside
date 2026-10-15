import adapter from "@sveltejs/adapter-static";
import { vitePreprocess } from "@sveltejs/vite-plugin-svelte";

/** @type {import('@sveltejs/kit').Config} */
const config = {
  preprocess: vitePreprocess(),
  kit: {
    // SPA: Tauri serves the files; every route falls back to index.html.
    adapter: adapter({ fallback: "index.html" }),
  },
};

export default config;
