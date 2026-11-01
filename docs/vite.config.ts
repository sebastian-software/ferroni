import { highlightCode } from "ardo/vite";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { defineConfig, type Plugin } from "vite";

import examples from "./app/data/home-code.json" with { type: "json" };
import walkthrough from "./app/data/regex-walkthrough.json" with { type: "json" };

function codeSpans(html: string) {
  const content = /<code[^>]*>([\s\S]*)<\/code>/.exec(html)?.[1];
  if (content === undefined) throw new Error("The highlighter must return one code block.");
  return content;
}

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
        { theme: "github-light-default" },
      );
      const snippets = await Promise.all(
        Object.entries(examples).map(async ([name, code]) => {
          const highlighted = await highlightCode(code, "rust", { theme: "github-light-default" });
          return `export const ${name}Html = ${JSON.stringify(codeSpans(highlighted))};`;
        }),
      );
      const patterns = await Promise.all(
        walkthrough.syntax.map(async (example) =>
          highlightCode(example.pattern, "regexp", { theme: "github-light-default" }),
        ),
      );
      return `export default ${JSON.stringify(html)};\n${snippets.join("\n")}\nexport const patternHtml = ${JSON.stringify(patterns)};`;
    },
  };
}

export default defineConfig({ plugins: [sampleHighlight()] });
