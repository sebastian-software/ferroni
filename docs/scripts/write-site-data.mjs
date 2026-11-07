/* eslint-disable security/detect-non-literal-fs-filename -- The fixed route directories and Cargo manifest are local build inputs. */
import { readdir, readFile, writeFile } from "node:fs/promises";

const cargo = await readFile(new URL("../../Cargo.toml", import.meta.url), "utf8");
const version = /^version\s*=\s*"([^"]+)"/m.exec(cargo)?.[1];
if (!version) throw new Error("Cargo.toml must declare Ferroni's release version.");
const rustVersion = /^rust-version\s*=\s*"([^"]+)"/m.exec(cargo)?.[1];
if (!rustVersion) throw new Error("Cargo.toml must declare Ferroni's minimum Rust version.");

function parseFields(frontmatter) {
  return new Map(
    frontmatter.split("\n").map((line) => {
      const colon = line.indexOf(":");
      return [line.slice(0, colon), line.slice(colon + 1).trim()];
    }),
  );
}

// Each section shows a line icon from the family mark sprite in the section rail.
const sections = [
  { id: "guide", label: "Guide", icon: "document" },
  { id: "perf", label: "Performance", icon: "check" },
  { id: "adr", label: "Architecture", icon: "pen" },
];
const navigation = await Promise.all(
  sections.map(async (section) => {
    const directory = new URL(`../app/routes/${section.id}/`, import.meta.url);
    const entries = await readdir(directory);
    const files = entries.filter((file) => file.endsWith(".mdx"));
    const pages = await Promise.all(
      files.map(async (file) => {
        const source = await readFile(new URL(file, directory), "utf8");
        const frontmatter = /^---\n([\s\S]*?)\n---/.exec(source)?.[1] ?? "";
        const fields = parseFields(frontmatter);
        const title = fields.get("title")?.replaceAll(/^"|"$/g, "");
        if (!title) throw new Error(`${section.id}/${file} is missing a title.`);
        return {
          title,
          href: `/${section.id}/${file.slice(0, -4)}`,
          order: Number(fields.get("order") ?? 999),
        };
      }),
    );
    pages.sort((a, b) => a.order - b.order || a.title.localeCompare(b.title, "en"));
    return { ...section, href: pages[0].href, pages };
  }),
);
await writeFile(
  new URL("../app/data/site.json", import.meta.url),
  `${JSON.stringify({ version, rustVersion, navigation }, null, 2)}\n`,
);
