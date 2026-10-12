import { ardo, highlightCode } from "ardo/vite";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { defineConfig, type Plugin } from "vite";

const cargoToml = readFileSync("../Cargo.toml", "utf8");
const version = /^version\s*=\s*"(.+)"/m.exec(cargoToml)?.[1] ?? "0.0.0";

// Render the committed Rust sample with the site's existing Ferriki highlighter.
function sampleHighlight(): Plugin {
  const virtualId = "virtual:ferroni-sample";
  const resolvedId = `\0${virtualId}`;
  return {
    name: "ferroni-sample-highlight",
    resolveId(id) {
      if (id === virtualId) return resolvedId;
    },
    async load(id) {
      if (id !== resolvedId) return;
      const path = new URL("app/data/regex-sample.json", import.meta.url);
      this.addWatchFile(fileURLToPath(path));
      const sample: unknown = JSON.parse(readFileSync("app/data/regex-sample.json", "utf8"));
      if (
        typeof sample !== "object" ||
        sample === null ||
        !("input" in sample) ||
        typeof sample.input !== "string"
      ) {
        throw new Error("The committed sample must contain Rust source in input.");
      }
      const html = await highlightCode(sample.input, "rust", { theme: "gruvbox-dark-hard" });
      return `export default ${JSON.stringify(html)};`;
    },
  };
}

export default defineConfig({
  base: "/",
  plugins: [
    sampleHighlight(),
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
