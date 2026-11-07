/* eslint-disable security/detect-non-literal-fs-filename -- The artifact paths are fixed in this script. */
/**
 * Keep the home page's specimens and the guide's syntax examples tied to
 * Ferroni's actual results: run each website example, then write its JSON
 * (`pnpm sample:write`) or compare it with the committed file
 * (`pnpm sample:check`). The written files are never edited by hand.
 */
import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";

const root = resolve(import.meta.dirname, "../..");
const check = process.argv.includes("--check");

const artifacts = [
  {
    example: "website_homepage",
    file: "docs/app/data/home-specimens.json",
    stale: "The home-page specimens changed. Run pnpm sample:write and commit the result.",
  },
  {
    example: "website_walkthrough",
    file: "docs/app/data/regex-walkthrough.json",
    stale: "The guide's syntax examples changed. Run pnpm sample:write and commit the result.",
  },
];

const written = [];
for (const { example, file, stale } of artifacts) {
  const actual = JSON.parse(
    execFileSync("cargo", ["run", "--locked", "--quiet", "--example", example], {
      cwd: root,
      encoding: "utf8",
    }),
  );
  const path = resolve(root, file);
  if (check) {
    const committed = JSON.parse(readFileSync(path, "utf8"));
    if (JSON.stringify(committed) !== JSON.stringify(actual)) throw new Error(stale);
  } else {
    writeFileSync(path, `${JSON.stringify(actual, null, 2)}\n`);
    written.push(path);
  }
}

if (check) {
  console.log("The committed specimens and syntax examples match Ferroni's actual results.");
} else {
  // Format in the generator, so the committed files pass format:check as written.
  execFileSync(resolve(root, "docs/node_modules/.bin/oxfmt"), written, {
    cwd: resolve(root, "docs"),
  });
  console.log(`Wrote ${artifacts.map((artifact) => artifact.file).join(" and ")}.`);
}
