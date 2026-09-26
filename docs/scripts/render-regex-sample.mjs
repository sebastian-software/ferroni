/** Commit the example's actual stdout; escape it only for safe HTML display. */
import { execFileSync } from "node:child_process";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";

const root = resolve(import.meta.dirname, "../..");
const artifact = resolve(root, "docs/app/data/regex-sample.json");
const input = readFileSync(resolve(root, "examples/website_sample.rs"), "utf8");
const stdout = execFileSync(
  "cargo",
  ["run", "--locked", "--quiet", "--example", "website_sample"],
  {
    cwd: root,
    encoding: "utf8",
  },
);
const escaped = stdout.replaceAll("&", "&amp;").replaceAll("<", "&lt;").replaceAll(">", "&gt;");
const output = `<pre><code>${escaped}</code></pre>`;

if (process.argv.includes("--check")) {
  const committed = JSON.parse(readFileSync(artifact, "utf8"));
  // The caption retains the version that produced the artifact. Later crate
  // releases may reproduce it without rewriting that historical attribution.
  if (committed.input !== input || committed.stdout !== stdout || committed.output !== output) {
    throw new Error("The home-page sample changed. Run pnpm sample:write and commit the result.");
  }
  console.log("The committed home-page sample matches the example's actual output.");
} else {
  const cargo = readFileSync(resolve(root, "Cargo.toml"), "utf8");
  const version = /^version\s*=\s*"(.+)"/m.exec(cargo)?.[1];
  if (version === undefined) throw new Error("Cargo.toml has no package version.");
  mkdirSync(resolve(root, "docs/app/data"), { recursive: true });
  writeFileSync(artifact, `${JSON.stringify({ version, input, stdout, output }, null, 2)}\n`);
  console.log("Wrote docs/app/data/regex-sample.json from cargo run --example website_sample.");
}
