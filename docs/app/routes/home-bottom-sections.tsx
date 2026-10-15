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
      title="The engine under your syntax highlighter"
      intro={
        <>
          VS Code and Shiki highlight code with TextMate grammars, and those grammars are written
          for Oniguruma. Ferroni&rsquo;s Scanner keeps vscode-oniguruma&rsquo;s API, UTF-16 offsets
          included, and a multi-pattern DFA rules out patterns before any of them runs.
        </>
      }
      note={
        <>
          <a href="https://ferriki.dev">Ferriki</a>, a Shiki-compatible highlighter in Rust, runs on
          it. <Link to="/guide/getting-started#the-scanner-api">The Scanner API in the guide</Link>
        </>
      }
    >
      <HighlightedRust caption="highlight.rs" html={scannerHtml} />
      <ScannerWalkthrough />
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
      title="Memory-safe where C was not"
      intro={
        <>
          C Oniguruma fixed five memory-safety CVEs in 2019. Ferroni&rsquo;s engine is safe Rust:
          ten unsafe sites remain, all linking parse-tree nodes, none in buffer or string code. For
          hostile input, give a search a time or retry limit, and let the compiler flag risky
          patterns.
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
      tone="dim"
      title="From cargo add to first match"
      intro={
        <>
          No C compiler, no bindgen: Cargo builds Ferroni like any other crate, and CI tests it on
          Linux, macOS and Windows. Its <code>Regex</code> API will look familiar if you know the{" "}
          <code>regex</code> crate.
        </>
      }
      note={
        <a href="https://github.com/sebastian-software/ferroni/blob/main/examples/website_sample.rs">
          The output comes from running this example: cargo run --example website_sample
        </a>
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
    name: "TextMate grammars, VS Code, Shiki",
    status: "Covered",
    settled: true,
    detail: "The Scanner keeps vscode-oniguruma's API and its UTF-16 offsets.",
  },
  {
    name: "Rust code on C Oniguruma",
    status: "Covered",
    settled: true,
    detail: (
      <>
        Same patterns, no C toolchain.{" "}
        <Link to="/guide/coming-from-regex">Map an onig call to Ferroni</Link>.
      </>
    ),
  },
  {
    name: "jq",
    status: "Covered",
    settled: true,
    detail: "jq matches on UTF-8 text, which Ferroni handles in full.",
  },
  {
    name: "PHP mbregex",
    status: "UTF-8 only",
    detail: "Shift_JIS, EUC-JP and the other non-UTF-8 encodings are not covered.",
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
    text: "Bugs and differences from C Oniguruma get fixed.",
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
      intro="The scope is complete: Oniguruma syntax for ASCII and UTF-8, plus the scanner. No feature work is planned; the upkeep continues."
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
            Read the guide <Mark name="arrow" className="icon" size={18} />
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
        The guide takes you from a first match to captures, scanner loops and per-search limits.
        Moving an existing application? Start with the{" "}
        <Link to="/guide/compatibility">compatibility guide</Link>.
      </p>
    </ClosingAction>
  );
}
