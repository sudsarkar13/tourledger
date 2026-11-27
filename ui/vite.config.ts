import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: process.env.TAURI_DEV_HOST || "127.0.0.1",
    // In a plain browser the UI talks to the development bridge.
    proxy: { "/api": "http://127.0.0.1:4777" },
  },
  build: { target: ["es2022", "chrome105", "safari15"], sourcemap: false },
});
