/// <reference types="vitest" />
import path from "path"
import tailwindcss from "@tailwindcss/vite"
import react from "@vitejs/plugin-react"
import { defineConfig, loadEnv } from "vite"

const backendEnvDir = path.resolve(__dirname, "../voowork-backend")
const desktopEnvDir = __dirname

// https://vite.dev/config/
export default defineConfig(({ mode }) => {
  const backendEnv = loadEnv(mode, backendEnvDir, "")
  const desktopEnv = loadEnv(mode, desktopEnvDir, "")
  const mergedEnv = { ...backendEnv, ...desktopEnv }

  return {
    envDir: backendEnvDir,
    plugins: [react(), tailwindcss()],
    define: Object.fromEntries(
      Object.entries(mergedEnv)
        .filter(([key]) => key.startsWith("VITE_"))
        .map(([key, value]) => [`import.meta.env.${key}`, JSON.stringify(value)]),
    ),
    resolve: {
      alias: {
        "@": path.resolve(__dirname, "./src"),
      },
    },
    server: {
      port: 1420,
      strictPort: true,
    },
    test: {
      globals: true,
      environment: "jsdom",
      setupFiles: ["./src/test/setup.ts"],
    },
  }
})
