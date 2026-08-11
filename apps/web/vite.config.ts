import path from "node:path"
import tailwindcss from "@tailwindcss/vite"
import react from "@vitejs/plugin-react"
import { defineConfig } from "vite"

const conductorProxyTarget =
  process.env.CONDUCTOR_PROXY_TARGET ?? "http://127.0.0.1:4700"

export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
      "@monaco": path.resolve(__dirname, "./node_modules/monaco-editor"),
    },
  },
  server: {
    port: 5174,
    proxy: {
      "/api": {
        target: conductorProxyTarget,
        changeOrigin: true,
      },
    },
  },
})
