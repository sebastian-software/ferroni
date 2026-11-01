// cspell:words Onigmo mbregex onig
import { ArdoLink as Link } from "ardo/runtime";
import { FerraAction, FerraArticle, FerraFeatureGrid } from "ferramenta-family/ferra";
import { WORKSHOP } from "ferramenta-family/registry";
import { limitsHtml, scannerHtml } from "virtual:ferroni-sample";

import { HighlightedRust } from "./highlighted-rust";
import { ScannerWalkthrough } from "./home-examples";
import { HomeRunSample } from "./home-run-sample";

export function StartSection() {
  return (
    <FerraArticle
      id="fr-run"
      title="From first pattern to first match."
      introduction="Add Ferroni to a Rust project, paste this into src/main.rs and run it. The captures have names; the API feels familiar."
      meta={<Link to="/guide/getting-started">Create a project and run the example</Link>}
    >
      <HomeRunSample />
    </FerraArticle>
  );
}

export function ScannerSection() {
  return (
    <FerraArticle
      id="fr-scanner"
      title="Your next highlighter starts here."
      introduction="Give the scanner a set of patterns and ask which matches next. You handle the grammar rules and scopes; Ferroni handles the regex work."
      meta={<Link to="/guide/scanner">Build your first scanner</Link>}
    >
      <p>
        The scanning model follows vscode-oniguruma, with UTF-16 positions for editor integrations.
        <a href="https://ferriki.dev"> Ferriki</a> uses it underneath its TextMate tokenizer.
      </p>
      <HighlightedRust caption="A multi-pattern search · Rust" html={scannerHtml} />
      <ScannerWalkthrough />
    </FerraArticle>
  );
}

export function PrecompiledSection() {
  return (
    <FerraArticle
      id="fr-precompiled"
      title="Prepare once. Load across starts."
      introduction="The same highlighter should not have to prepare the same static patterns on every start. Move reusable scanner preparation into your build."
      meta={
        <>
          <Link to="/guide/precompiled-eager">Prepared Eager archives</Link>
          {" · "}
          <Link to="/guide/precompiled-patterns">Compiled program archives</Link>
        </>
      }
    >
      <FerraFeatureGrid
        items={[
          {
            heading: "Prepare at build time",
            text: "Build static scanner sets in Eager mode. Export compiled programs together with their prepared group instructions.",
          },
          {
            heading: "Ship portable assets",
            text: "Commit, embed or serve the paired archives. Compatible native and WebAssembly builds read the same files.",
          },
          {
            heading: "Load what you use",
            text: "Decode requested programs and groups on demand. Keep the original patterns for fallback; search caches remain local.",
          },
        ]}
      />
      <p>
        Enable <code>precompiled-eager</code> to reuse compiled programs and group preparation
        through <code>ScannerPatternCache</code>. The smaller <code>precompiled</code> feature
        shares compiled programs while leaving group preparation local.
      </p>
    </FerraArticle>
  );
}

export function SafetySection() {
  return (
    <FerraArticle
      id="fr-safety"
      title="Powerful patterns. Practical guardrails."
      introduction="Backtracking makes expressive patterns possible. Limits help you keep that work under control when text comes from outside your application."
      meta={
        <>
          <Link to="/guide/untrusted-input">Handle untrusted input</Link>
          {" · "}
          <Link to="/adr/002-unsafe-code-policy">The unsafe code policy</Link>
        </>
      }
    >
      <p>
        Set per-search time and retry limits, then use fallible search methods to distinguish a
        stopped search from no match. A compile-time check flags common backtracking risks. Only
        Rust’s <code>regex</code> crate guarantees linear-time matching; choose it when you need
        that guarantee.
      </p>
      <HighlightedRust caption="Per-search limits · Rust" html={limitsHtml} />
      <p>
        Buffer and string processing use safe Rust. The remaining unsafe code links parse-tree
        nodes, under a documented policy. Fuzzers exercise compilation, matching and the scanner on
        every pull request.
      </p>
    </FerraArticle>
  );
}

export function FitSection() {
  return (
    <FerraArticle
      id="fr-fit"
      title="A clear scope. A deliberate fit."
      introduction="Choose Ferroni when Oniguruma’s syntax matters. Keep the encoding, runtime and safety requirements of your application in view."
      meta={<Link to="/guide/compatibility">Read the full compatibility scope</Link>}
    >
      <FerraFeatureGrid
        items={[
          {
            heading: "Editors and tokenizers",
            text: (
              <>
                Multi-pattern scanning with byte or UTF-16 offsets. Your runtime interprets the
                grammar. <Link to="/guide/scanner">Scanner guide</Link>.
              </>
            ),
          },
          {
            heading: "Rust applications",
            text: (
              <>
                Keep your Oniguruma patterns without a C toolchain.{" "}
                <Link to="/guide/coming-from-regex">Compare the regex and onig APIs</Link>.
              </>
            ),
          },
          {
            heading: "Native and WebAssembly",
            text: "Cargo builds both. There is no npm package. In browser WebAssembly, use retry limits: that target has no clock for timeouts.",
          },
          {
            heading: "ASCII and UTF-8",
            text: "The other 27 upstream encodings and the POSIX/GNU APIs are outside the port. Ruby uses Onigmo, a separate fork.",
          },
        ]}
      />
      <p>
        Patterns from jq or PHP mbregex can work within the UTF-8 scope. Replacing the engine in
        those applications is a separate integration; legacy encodings such as Shift_JIS are not
        covered.
      </p>
    </FerraArticle>
  );
}

export function StatusSection() {
  return (
    <FerraArticle
      id="project-status"
      title="Feature-complete. Still moving forward."
      introduction="The Oniguruma compatibility scope is complete. Maintenance keeps correctness, Unicode, toolchains and security in focus, while Rust-only scanner work extends the port."
      meta={
        <>
          <a href="https://github.com/sebastian-software/ferroni/blob/main/CONTRIBUTING.md">
            Contributing
          </a>
          {" · "}
          <a href="https://github.com/sebastian-software/ferroni/blob/main/SECURITY.md">
            Security policy
          </a>
        </>
      }
    >
      <FerraFeatureGrid
        items={[
          {
            heading: "Compatibility first",
            text: "Report unexpected differences from C Oniguruma. Contributions retain the original module structure and control flow.",
          },
          {
            heading: "Kept current",
            text: "Unicode tables follow annual releases. Toolchain requirements rise when a fix or dependency needs it, with release notes. Security fixes target the latest release.",
          },
        ]}
      />
    </FerraArticle>
  );
}

export function ClosingSection() {
  return (
    <section className="ferra-editorial-closing" id="fr-start" aria-labelledby="work">
      <h2 id="work">Put your patterns to work.</h2>
      <p>
        Start with a working match, or go straight to the scanner for tokenizers and highlighters.
      </p>
      <div className="ferra-actions">
        <FerraAction href="/guide/getting-started">Start with Rust</FerraAction>
        <FerraAction variant="secondary" href="/guide/scanner">
          Explore the scanner
        </FerraAction>
      </div>
      <p>
        <a href="https://crates.io/crates/ferroni">Crates.io</a>
        {" · "}
        <a href="https://docs.rs/ferroni">API reference</a>
        {" · "}
        <a href="https://github.com/sebastian-software/ferroni">GitHub</a>
        {" · "}
        <a href={WORKSHOP.consulting}>Integration and support</a>
      </p>
    </section>
  );
}
