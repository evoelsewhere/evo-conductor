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
    // Bind the literal IPv4 loopback explicitly — on some Node/OS
    // combinations, the default "localhost" host resolves to the IPv6
    // loopback (::1) only, so a link built from a literal 127.0.0.1
    // public_url (e.g. an invite email's connect_url) gets ECONNREFUSED.
    host: "127.0.0.1",
    port: 5174,
    proxy: {
      "/api": {
        target: conductorProxyTarget,
        changeOrigin: true,
      },
    },
  },
})
