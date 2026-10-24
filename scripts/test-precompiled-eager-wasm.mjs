#!/usr/bin/env node
// Run the import-free browser-target Rust conformance example in V8 and,
// optionally, a real Chromium process with an isolated temporary profile.
import { spawn } from "node:child_process";
import { randomUUID } from "node:crypto";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import { basename, join } from "node:path";

const [file, flag, browser] = process.argv.slice(2);
if (!file || (flag !== undefined && (flag !== "--browser" || !browser)) || process.argv.length > 5) {
  throw new Error("Usage: test-precompiled-eager-wasm.mjs FILE.wasm [--browser EXECUTABLE]");
}
const bytes = await readFile(file);
const module = await WebAssembly.compile(bytes);
if (WebAssembly.Module.imports(module).length !== 0) throw new Error("Unexpected host imports");
const instance = await WebAssembly.instantiate(module);
if (instance.exports.main(0, 0) !== 0) throw new Error("Conformance main failed");
console.log("PASS: native-produced Eager archives in browser-target Wasm (Node/V8)");

if (browser) {
  const profile = await mkdtemp(join(tmpdir(), "ferroni-eager-browser-"));
  const token = randomUUID();
  let child;
  let stderr = "";
  let timer;
  let resolveResult;
  let rejectResult;
  const result = new Promise((resolve, reject) => { resolveResult = resolve; rejectResult = reject; });
  const server = createServer((request, response) => {
    if (request.url === `/${token}/test.wasm`) {
      response.writeHead(200, { "Content-Type": "application/wasm" }).end(bytes);
    } else if (request.url === `/${token}/result` && request.method === "POST") {
      let body = "";
      request.on("data", chunk => { body += chunk; if (body.length > 4096) request.destroy(); });
      request.on("end", () => {
        response.writeHead(200).end("ok");
        if (body === "PASS") resolveResult();
        else rejectResult(new Error(`Browser Wasm failed: ${body}`));
      });
    } else if (request.url === `/${token}/`) {
      response.writeHead(200, { "Content-Type": "text/html" }).end(`<!doctype html>
<meta charset="utf-8"><title>Ferroni prepared Eager conformance</title><pre>RUNNING</pre>
<script type="module">
try {
  const bytes = await (await fetch('test.wasm')).arrayBuffer();
  const module = await WebAssembly.compile(bytes);
  if (WebAssembly.Module.imports(module).length) throw new Error('Unexpected host imports');
  const instance = await WebAssembly.instantiate(module);
  if (instance.exports.main(0, 0) !== 0) throw new Error('Conformance main failed');
  document.querySelector('pre').textContent = 'PASS';
  await fetch('result', {method: 'POST', body: 'PASS'});
} catch (error) {
  document.querySelector('pre').textContent = String(error);
  await fetch('result', {method: 'POST', body: String(error)});
}
</script>`);
    } else response.writeHead(404).end();
  });
  try {
    await new Promise((resolve, reject) => {
      server.once("error", reject);
      server.listen(0, "127.0.0.1", resolve);
    });
    const url = `http://127.0.0.1:${server.address().port}/${token}/`;
    const args = ["--headless", "--no-sandbox", "--disable-gpu", `--user-data-dir=${profile}`, url];
    child = spawn(browser, args, {
      stdio: ["ignore", "ignore", "pipe"],
      detached: process.platform !== "win32",
    });
    child.stderr.on("data", chunk => { stderr = (stderr + chunk).slice(-4096); });
    child.once("error", rejectResult);
    child.once("exit", code => rejectResult(new Error(`Browser exited (${code}): ${stderr}`)));
    timer = setTimeout(() => rejectResult(new Error(`Browser timeout: ${stderr}`)), 45_000);
    await result;
    console.log(`PASS: native-produced Eager archives in ${basename(browser)}`);
  } finally {
    clearTimeout(timer);
    if (child?.pid && child.exitCode === null && child.signalCode === null) {
      await new Promise(resolve => {
        child.once("exit", resolve);
        // Terminate the isolated browser process group, including renderers.
        if (process.platform === "win32") child.kill("SIGKILL");
        else process.kill(-child.pid, "SIGKILL");
      });
    }
    server.closeAllConnections();
    await new Promise(resolve => server.close(resolve));
    await rm(profile, { recursive: true, force: true });
  }
}
