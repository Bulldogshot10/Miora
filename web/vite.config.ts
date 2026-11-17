import { svelte } from "@sveltejs/vite-plugin-svelte";
import tailwindcss from "@tailwindcss/vite";
import { defineConfig } from "vite";
import path from "node:path";

export default defineConfig({
  plugins: [svelte(), tailwindcss()],
  resolve: {
    alias: {
      $lib: path.resolve(process.cwd(), "src/lib"),
    },
  },
  clearScreen: false,
  server: {
    host: "0.0.0.0",
    port: 5173,
    strictPort: true,
  },
});
