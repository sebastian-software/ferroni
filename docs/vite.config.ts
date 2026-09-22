import { ardo } from "ardo/vite";
import { readFileSync } from "node:fs";
import { defineConfig } from "vite";

const cargoToml = readFileSync("../Cargo.toml", "utf8");
const version = /^version\s*=\s*"(.+)"/m.exec(cargoToml)?.[1] ?? "0.0.0";

export default defineConfig({
  plugins: [
    ardo({
      title: "Ferroni",
      description: "Oniguruma-compatible regex engine",

      project: { version },
    }),
  ],
});
