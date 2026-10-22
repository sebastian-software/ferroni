// cspell:words Onigmo onig
import { ClosingAction, Ledger, Mark, Principles, Relations, Section } from "ferramenta-family";
import { Link } from "react-router";
import { limitsHtml, scannerHtml } from "virtual:ferroni-sample";

import { HighlightedRust } from "../components/highlighted-rust";
import { ScannerWalkthrough } from "../components/home-examples";
import { HomeRunSample } from "../components/home-run-sample";

/* -------------------------------------------------- */
/*  Scanner                                           */
/* -------------------------------------------------- */

export function ScannerSection() {
  return (
    <Section
      id="fr-scanner"
      layout="split"
      tone="dim"
      title="Your next highlighter starts here"
      intro={
        <>
          Building a tokenizer or a TextMate runtime? Give Ferroni&rsquo;s Scanner a set of patterns
          and ask which matches next. It follows vscode-oniguruma&rsquo;s scanning model, with
          UTF-16 positions for editor integrations. Your runtime handles the grammar rules and
          scopes; Ferroni handles the regex work.
        </>
      }
      note={
        <>
          <a href="https://ferriki.dev">Ferriki</a>, a Shiki-compatible highlighter in Rust, runs on
          it. <Link to="/guide/scanner">Build your first scanner</Link>
        </>
      }
    >
      <HighlightedRust caption="highlight.rs" html={scannerHtml} />
      <ScannerWalkthrough />
      <p className="fam-note">
        Loading the same grammar on every start? Enable <code>precompiled</code> to ship a pattern
        archive and decode programs as scanners request them. Keep one shared cache for the session.{" "}
        <Link to="/guide/precompiled-patterns">Reuse compilation across starts</Link>
      </p>
      <Relations current="ferroni" />
    </Section>
  );
}

/* -------------------------------------------------- */
/*  Memory safety and untrusted input                 */
/* -------------------------------------------------- */

/* The unsafe count is the output of scripts/count-unsafe.sh, as in ADR-002. */
export function SafetySection() {
  return (
    <Section
      id="fr-safety"
      layout="split"
      tone="dim"
      title="Powerful patterns. Practical guardrails."
      intro={
        <>
          Buffer and string processing use safe Rust. Ten unsafe sites remain, all linking
          parse-tree nodes. Ferroni also gives you per-search time and retry limits, plus a
          compile-time check that flags common backtracking risks. Set limits when input comes from
          outside your application; use the fallible search methods to handle a limit explicitly.
        </>
      }
      note={
        <>
          Fuzzers run compilation, matching and the scanner on every pull request.{" "}
          <Link to="/guide/untrusted-input">Untrusted input</Link>
          {" · "}
          <Link to="/adr/002-unsafe-code-policy">The unsafe code policy</Link>
        </>
      }
    >
      <HighlightedRust caption="limits.rs" html={limitsHtml} />
    </Section>
  );
}

/* -------------------------------------------------- */
/*  First match                                       */
/* -------------------------------------------------- */

export function StartSection() {
  return (
    <Section
      id="fr-run"
      title="From cargo add to first match"
      intro={
        <>
          Add Ferroni, paste this into <code>src/main.rs</code>, and run <code>cargo run</code>.
          This example uses look-behind to find a date after its label, then reads the year, month
          and day by name. If you know the <code>regex</code> crate, you&rsquo;ll feel at home.
        </>
      }
      note={
        <>
          <Link to="/guide/getting-started">Create a project and run the example</Link>
          {" · "}
          <a href="https://github.com/sebastian-software/ferroni/blob/main/examples/website_sample.rs">
            Source and verified output
          </a>
        </>
      }
    >
      <HomeRunSample />
    </Section>
  );
}

/* -------------------------------------------------- */
/*  Fit and scope                                     */
/* -------------------------------------------------- */

const fit = [
  {
    name: "TextMate runtimes and tokenizers",
    status: "Scanner API",
    settled: true,
    detail:
      "Find the next match across patterns, with byte or UTF-16 offsets. Your runtime interprets the grammar.",
  },
  {
    name: "Rust code on C Oniguruma",
    status: "Rust API",
    settled: true,
    detail: (
      <>
        Same patterns, no C toolchain.{" "}
        <Link to="/guide/coming-from-regex">Map an onig call to Ferroni</Link>.
      </>
    ),
  },
  {
    name: "Patterns from jq",
    status: "UTF-8 patterns",
    settled: true,
    detail:
      "Run Oniguruma patterns over UTF-8 text. Integrating Ferroni into jq is a separate application change.",
  },
  {
    name: "Patterns from PHP mbregex",
    status: "UTF-8 only",
    detail: "Shift_JIS, EUC-JP and the other non-UTF-8 encodings are not covered.",
  },
  {
    name: "Browsers and WebAssembly",
    status: "Crate only",
    detail:
      "Cargo builds it for wasm32; there is no npm package. In the browser, set a retry limit: that target has no clock for timeouts.",
  },
  {
    name: "Ruby",
    status: "Not a target",
    detail: "Ruby runs Onigmo, a fork with its own history and encodings.",
  },
  {
    name: "Untrusted patterns",
    status: "Use regex",
    detail:
      "Limits bound Ferroni's backtracking; only Rust's regex crate guarantees linear-time matching.",
  },
];

export function FitSection() {
  return (
    <Section
      id="fr-fit"
      title="Where Ferroni fits"
      intro={
        <>
          Ferroni covers ASCII and UTF-8, two of Oniguruma&rsquo;s 29 encodings, and leaves out the
          POSIX and GNU APIs. <Link to="/guide/compatibility">Compatibility and scope</Link>
        </>
      }
    >
      <Ledger entries={fit} />
    </Section>
  );
}

/* -------------------------------------------------- */
/*  Project status                                    */
/* -------------------------------------------------- */

const maintenance = [
  {
    heading: "Bug fixes",
    text: "Report a bug or an unexpected difference from C Oniguruma. Compatibility stays the priority.",
  },
  {
    heading: "Unicode",
    text: "The tables follow each annual Unicode Character Database release.",
  },
  {
    heading: "Toolchain",
    text: "The MSRV rises only when a fix or dependency needs it, with release notes.",
  },
  {
    heading: "Security",
    text: "Fixes target the latest release. Report vulnerabilities privately.",
  },
];

export function StatusSection() {
  return (
    <Section
      id="project-status"
      tone="dim"
      title="Feature-complete and maintained"
      intro="Ready to build on: Oniguruma syntax for ASCII and UTF-8, plus the scanner. The feature scope is complete. Maintenance focuses on correctness, Unicode, toolchains and security; new features are not planned."
      note={
        <>
          Issues are welcome; pull requests keep{" "}
          <Link to="/adr/001-one-to-one-parity-with-c-original">
            structural parity with C Oniguruma
          </Link>
          .{" "}
          <a href="https://github.com/sebastian-software/ferroni/blob/main/CONTRIBUTING.md">
            Contributing
          </a>
          {" · "}
          <a href="https://github.com/sebastian-software/ferroni/labels/help%20wanted">
            Help wanted
          </a>
          {" · "}
          <a href="https://github.com/sebastian-software/ferroni/blob/main/SECURITY.md">
            Security policy
          </a>
        </>
      }
    >
      <Principles items={maintenance} />
    </Section>
  );
}

export function ClosingSection() {
  return (
    <ClosingAction
      id="fr-start"
      title="Start building with Ferroni"
      actions={
        <>
          <Link to="/guide/getting-started" className="fam-btn fam-btn-primary">
            Run your first match <Mark name="arrow" className="icon" size={18} />
          </Link>
          <Link to="/perf/regex-workloads" className="fam-btn fam-btn-ghost">
            Benchmarks
          </Link>
        </>
      }
      links={
        <>
          <a href="https://crates.io/crates/ferroni">crates.io/crates/ferroni</a>
          <a href="https://docs.rs/ferroni">docs.rs/ferroni</a>
          <a href="https://github.com/sebastian-software/ferroni">GitHub</a>
          <a href="https://github.com/sebastian-software/ferroni/blob/main/LICENSE">BSD-2-Clause</a>
        </>
      }
    >
      <p className="fam-intro">
        Start with a working example, then make it yours. Building a highlighter? Follow the{" "}
        <Link to="/guide/scanner">Scanner guide</Link>. Bringing an existing application? Check the{" "}
        <Link to="/guide/coming-from-regex">regex and onig migration notes</Link>.
      </p>
    </ClosingAction>
  );
}
