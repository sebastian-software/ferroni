import { ardo } from "ardo/vite";
import { readFileSync } from "node:fs";
import { defineConfig } from "vite";

const cargoToml = readFileSync("../Cargo.toml", "utf8");
const version = /^version\s*=\s*"(.+)"/m.exec(cargoToml)?.[1] ?? "0.0.0";

export default defineConfig({
  base: "/",
  plugins: [
    ardo({
      title: "Ferroni",
      description: "Oniguruma-compatible regex engine",
      githubPages: false,
      siteUrl: "https://ferroni.dev",

      project: { version },
      // The theme contract moved from the ADR list to docs/readme-theme.md,
      // which is not part of the site. Old links land on that file on GitHub.
      redirects: [
        {
          from: "/adr/readme-theme-composition",
          to: "https://github.com/sebastian-software/ferroni/blob/main/docs/readme-theme.md",
        },
      ],
    }),
  ],
});
