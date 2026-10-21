import { readdir, readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const clientRoot = fileURLToPath(new URL("../build/client/", import.meta.url));
const siteOrigin = "https://ferroni.dev";

async function htmlFiles(directory) {
  // Every path starts at the generated docs build and comes from its own directory entries.
  // eslint-disable-next-line security/detect-non-literal-fs-filename
  const entries = await readdir(directory, { withFileTypes: true });
  const nested = await Promise.all(
    entries.map(async (entry) => {
      const entryPath = path.join(directory, entry.name);
      if (entry.isDirectory()) return htmlFiles(entryPath);
      return entry.isFile() && entry.name.endsWith(".html") ? [entryPath] : [];
    }),
  );
  return nested.flat();
}

function decodeHtml(value) {
  return value.replaceAll(/&(#x[\da-f]+|#\d+|amp|quot|lt|gt);/gi, (match, entity) => {
    if (entity[0] === "#") {
      const codePoint =
        entity[1].toLowerCase() === "x"
          ? Number.parseInt(entity.slice(2), 16)
          : Number.parseInt(entity.slice(1), 10);
      return Number.isInteger(codePoint) && codePoint >= 0 && codePoint <= 1_114_111
        ? String.fromCodePoint(codePoint)
        : match;
    }
    return {
      amp: "&",
      quot: '"',
      lt: "<",
      gt: ">",
    }[entity.toLowerCase()];
  });
}

function attributesFor(tag) {
  return new Map(
    Array.from(tag.matchAll(/\s(id|name|href)\s*=\s*(?:"([^"]*)"|'([^']*)')/gi), (match) => [
      match[1].toLowerCase(),
      decodeHtml(match[2] ?? match[3]),
    ]),
  );
}

function routeForFile(file) {
  const relative = path.relative(clientRoot, file).split(path.sep).join("/");
  if (relative === "__spa-fallback.html") return null;
  if (relative === "index.html") return "/";
  if (relative.endsWith("/index.html")) {
    return `/${relative.slice(0, -"/index.html".length)}`;
  }
  if (relative.endsWith(".html")) {
    return `/${relative.slice(0, -".html".length)}`;
  }
  return null;
}

function inspectTag(tag, anchors, links) {
  const tagName = /^<([a-z][\w:-]*)\b/i.exec(tag)?.[1].toLowerCase();
  const attributes = attributesFor(tag);
  const id = attributes.get("id");
  if (id) anchors.add(id);
  if (tagName === "a") {
    const name = attributes.get("name");
    const href = attributes.get("href");
    if (name) anchors.add(name);
    if (href !== undefined) links.push(href);
  }
}

function inspectPage(route, html) {
  const anchors = new Set();
  const links = [];
  for (const match of html.matchAll(/<([a-z][\w:-]*)\b[^>]*>/gi)) {
    inspectTag(match[0], anchors, links);
  }
  return { route, anchors, links };
}

function normalizedRoute(pathname) {
  let route = decodeURIComponent(pathname);
  while (route.length > 1 && route.endsWith("/")) route = route.slice(0, -1);
  return route || "/";
}

let files;
try {
  files = await htmlFiles(clientRoot);
} catch (error) {
  throw new Error(`Could not read ${clientRoot}; run pnpm build first.`, { cause: error });
}
if (files.length === 0) {
  throw new Error(`No built HTML pages found in ${clientRoot}; run pnpm build first.`);
}

const pages = new Map();
for (const file of files) {
  const route = routeForFile(file);
  if (!route) continue;
  if (pages.has(route)) {
    throw new Error(`More than one built page maps to ${route}.`);
  }
  // The file list is collected only by walking the generated docs build directory.
  // eslint-disable-next-line security/detect-non-literal-fs-filename
  pages.set(route, inspectPage(route, await readFile(file, "utf8")));
}

if (pages.size === 0) {
  throw new Error(`No route pages found in ${clientRoot}; expected built HTML.`);
}

const errors = [];
let checkedLinks = 0;
for (const page of pages.values()) {
  // A route is served as a directory URL, so relative links resolve against a
  // trailing slash just as they do on the published site.
  const sourceUrl = new URL(page.route === "/" ? "/" : `${page.route}/`, siteOrigin);
  for (const href of page.links) {
    let targetUrl;
    try {
      targetUrl = new URL(href, sourceUrl);
    } catch {
      errors.push(`${page.route}: invalid link ${JSON.stringify(href)}`);
      continue;
    }
    if (targetUrl.origin !== siteOrigin) continue;

    checkedLinks += 1;
    let targetRoute;
    try {
      targetRoute = normalizedRoute(targetUrl.pathname);
    } catch {
      errors.push(`${page.route}: invalid encoded route in ${JSON.stringify(href)}`);
      continue;
    }
    const targetPage = pages.get(targetRoute);
    if (!targetPage) {
      errors.push(
        `${page.route}: ${JSON.stringify(href)} resolves to missing route ${targetRoute}`,
      );
      continue;
    }

    let fragment;
    try {
      fragment = decodeURIComponent(targetUrl.hash.slice(1));
    } catch {
      errors.push(`${page.route}: invalid encoded anchor in ${JSON.stringify(href)}`);
      continue;
    }
    if (fragment && !targetPage.anchors.has(fragment)) {
      errors.push(
        `${page.route}: ${JSON.stringify(href)} resolves to missing anchor #${fragment} on ${targetRoute}`,
      );
    }
  }
}

if (errors.length > 0) {
  console.error(`Found ${errors.length} broken internal route or anchor link(s):`);
  for (const error of errors) console.error(`- ${error}`);
  process.exitCode = 1;
} else {
  console.log(`Checked ${checkedLinks} internal links across ${pages.size} built routes.`);
}
