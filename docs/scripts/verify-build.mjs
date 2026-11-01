/* eslint-disable security/detect-non-literal-fs-filename -- Paths come from the generated local route manifest and build output. */
import { readdir, readFile } from "node:fs/promises";

function requireFragment(html, fragment, label) {
  if (!html.includes(fragment)) throw new Error(`${label} is missing ${fragment}`);
}

const output = new URL("../build/client/", import.meta.url);
const { version, navigation } = JSON.parse(
  await readFile(new URL("../app/data/site.json", import.meta.url), "utf8"),
);
const appFiles = await readdir(new URL("../app/", import.meta.url), { recursive: true });
if (appFiles.some((file) => file.endsWith(".css"))) {
  throw new Error(
    "Keep shared presentation in ferramenta-family; the Ferroni site has no local CSS.",
  );
}

const routes = ["/", ...navigation.flatMap((section) => section.pages.map((page) => page.href))];
for (const route of routes) {
  const file = route === "/" ? "index.html" : `${route.slice(1)}/index.html`;
  const source = await readFile(new URL(file, output), "utf8");
  const html = source.replaceAll(/<!--.*?-->/g, "");
  for (const fragment of [
    'class="ferra-header ferra-wrap"',
    'class="ferra-footer ferra-wrap"',
    'rel="icon"',
    `Ferroni v${version}`,
    "https://ferramenta.dev",
  ]) {
    requireFragment(html, fragment, route);
  }
  for (const [tag, pattern] of [
    ["h1", /<h1(?:\s|>)/g],
    ["main", /<main(?:\s|>)/g],
  ]) {
    if ((html.match(pattern) ?? []).length !== 1) {
      throw new Error(`${route} must have one ${tag}.`);
    }
  }
  if (html.includes('class="ardo-header') || html.includes('class="ardo-footer')) {
    throw new Error(`${route} must use the family chrome once.`);
  }
  if (route !== "/" && !html.includes('class="ferra-guide-navigation"')) {
    throw new Error(`${route} is missing its documentation navigation.`);
  }
}

const home = await readFile(new URL("index.html", output), "utf8");
for (const id of [
  "fr-run",
  "fr-why",
  "fr-syntax",
  "fr-scanner",
  "fr-precompiled",
  "fr-speed",
  "fr-safety",
  "fr-fit",
  "project-status",
  "fr-start",
  "work",
]) {
  if (!home.includes(`id="${id}"`))
    throw new Error(`Preserve the published homepage anchor #${id}.`);
}
if (home.includes("ferra-benchmark-table") || home.includes('class="fam-page')) {
  throw new Error(
    "The homepage should use the product story layout and link to detailed benchmarks.",
  );
}

const benchmarkSource = await readFile(new URL("perf/regex-workloads/index.html", output), "utf8");
const benchmark = benchmarkSource.replaceAll(/<!--.*?-->/g, "");
const summary = JSON.parse(
  await readFile(new URL("../app/data/production-summary.json", import.meta.url), "utf8"),
);
for (const platform of summary.platforms) {
  for (const phase of ["steady", "compile", "first_use"]) {
    requireFragment(
      benchmark,
      `id="comparison-${platform.profile}-${phase}"`,
      "Static benchmark page",
    );
    for (const entry of platform.comparisons.filter(
      (pair) =>
        pair.lane === phase &&
        ["c", "onigmo", "pcre2", "pcre2_jit", "fancy_regex", "regex"].includes(pair.engine),
    )) {
      requireFragment(
        benchmark,
        `${entry.family_weighted_ratio.toFixed(2)}×`,
        `${platform.profile}/${phase}/${entry.engine}`,
      );
    }
  }
}
const assets = await readdir(new URL("assets/", output));
if (!assets.some((file) => /^search-index-.*\.js$/.test(file))) {
  throw new Error("The site must ship its static search index.");
}
console.log(
  `Verified shared chrome, navigation, release data and benchmark evidence across ${routes.length} pages.`,
);
