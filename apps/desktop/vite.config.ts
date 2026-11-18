import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Tauri 需要固定端口
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  build: { target: "es2022" },
});
