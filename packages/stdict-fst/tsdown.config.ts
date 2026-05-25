import { defineConfig } from "tsdown";

export default defineConfig({
  entry: ["./index.ts"],
  format: "esm",
  dts: true,
  outDir: "dist",
  hash: false,
});
