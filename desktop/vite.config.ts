import { defineConfig } from "vite";

// Порт и настройки — как ждёт Tauri (tauri.conf.json → build.devUrl).
export default defineConfig({
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  build: { target: "es2022", outDir: "dist", rollupOptions: { input: { main: "index.html", orb: "orb.html", tray: "tray.html" } } },
});
