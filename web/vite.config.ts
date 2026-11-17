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
    host: "127.0.0.1",
    port: 5173,
    strictPort: true,
    hmr: {
      protocol: "ws",
      host: "127.0.0.1",
      port: 5174,
    },
  },
  preview: {
    host: "127.0.0.1",
    port: 4173,
    strictPort: true,
  },
});
