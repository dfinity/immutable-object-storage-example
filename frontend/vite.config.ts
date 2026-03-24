import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  define: {
    // ic-agent uses process.env checks; provide a stub for browser builds.
    "process.env": {},
  },
  server: {
    proxy: {
      // Forward /api calls to the local dfx replica during development.
      "/api": {
        target: "http://127.0.0.1:8080",
        changeOrigin: true,
      },
    },
  },
  optimizeDeps: {
    include: ["@icp-sdk/core"],
  },
});
