import { ardo, highlightCode } from "ardo/vite";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { defineConfig, type Plugin } from "vite";

import examples from "./app/data/home-code.json" with { type: "json" };
import walkthrough from "./app/data/regex-walkthrough.json" with { type: "json" };

const cargoToml = readFileSync("../Cargo.toml", "utf8");
const version = /^version\s*=\s*"(.+)"/m.exec(cargoToml)?.[1] ?? "0.0.0";

// Use one theme for Rust examples and regular-expression patterns on the home page.
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
      const html = await highlightCode(
        sample.input.replaceAll(/^\/\/!.*\n/gm, "").trimStart(),
        "rust",
        { theme: "gruvbox-dark-hard" },
      );
      const snippets = await Promise.all(
        Object.entries(examples).map(async ([name, code]) => {
          const highlighted = await highlightCode(code, "rust", { theme: "gruvbox-dark-hard" });
          return `export const ${name}Html = ${JSON.stringify(highlighted)};`;
        }),
      );
      const patterns = await Promise.all(
        walkthrough.syntax.map(async (example) =>
          highlightCode(example.pattern, "regexp", { theme: "gruvbox-dark-hard" }),
        ),
      );
      return `export default ${JSON.stringify(html)};\n${snippets.join("\n")}\nexport const patternHtml = ${JSON.stringify(patterns)};`;
    },
  };
}

export default defineConfig({
  base: "/",
  plugins: [
    sampleHighlight(),
    ardo({
      title: "Ferroni",
      description: "Oniguruma regex in pure Rust. Familiar API, TextMate scanner, no C toolchain.",
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
