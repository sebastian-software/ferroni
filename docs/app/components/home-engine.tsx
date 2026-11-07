// cspell:words Onigmo
import {
  MonoCredit,
  MonoLeadIn,
  MonoPassage,
  MonoProofs,
  MonoSection,
  MonoTerms,
  MonoWide,
} from "ferramenta-family/monografie";

import { GuardrailCards } from "./guardrail-cards";
import { specimens } from "./home-specimen-data";
import { PortMap } from "./port-map";

/** Upstream test files and the cases Ferroni passes, from the compatibility guide's parity table. */
const PROOFS = [
  { label: "test_utf8.c", value: "1,554 / 1,554" },
  { label: "test_back.c", value: "1,225 / 1,225" },
  { label: "test_syntax.c", value: "144 / 144" },
  { label: "test_options.c and test_regset.c", value: "51 / 51" },
  { label: "vscode-oniguruma scanner tests", value: "15 / 15" },
];

/* The home page's engine chapters: prepared archives, the port, guardrails, scope. */

export function Prepared() {
  return (
    <MonoSection id="fr-precompiled" title="Prepare once. Load across starts.">
      <p>
        <MonoLeadIn>A highlighter loads</MonoLeadIn> the same grammars every time it starts, and
        preparing their patterns costs time on every start. With the optional{" "}
        <code>precompiled-eager</code> feature, that preparation moves into your build. Export it
        once, ship it with your application, and load only the parts a document needs. Patterns that
        depend on captured text still resolve locally, and anything missing falls back to normal
        preparation.
      </p>
      <MonoWide>
        <MonoTerms
          prose
          columns={3}
          items={[
            {
              term: "At build time",
              description:
                "Prepare the static scanner sets and export compiled programs with their group instructions.",
            },
            {
              term: "In your release",
              description:
                "Commit, embed or serve the paired archives. Native and WebAssembly builds read the same files.",
            },
            {
              term: "On each start",
              description:
                "Decode the programs and groups you request. Search caches stay local to the process.",
            },
          ]}
        />
      </MonoWide>
    </MonoSection>
  );
}

export function Port() {
  return (
    <MonoSection
      id="fr-why"
      title="Ported function by function."
      standfirst="Bindings would keep the C code. A new engine would change what grammars match. Ferroni ports the original instead."
    >
      <MonoPassage
        note={{
          term: "A port",
          children: "The same program, rewritten in another language with the same structure.",
        }}
      >
        <p>
          <MonoLeadIn>Every C source file</MonoLeadIn> of Oniguruma has one Rust module, with the
          same function names and control flow. When a test fails, the corresponding C function is
          one search away. A fix upstream can be compared line by line. The idiomatic Rust API, the
          scanner and the guardrails sit beside the port and do not change how a pattern matches.
        </p>
      </MonoPassage>
      <PortMap />
      <MonoCredit className="fr-port-credit">
        Eight of thirteen modules shown.{" "}
        <a href="/guide/compatibility#how-the-port-maps-to-c">The complete mapping</a>
      </MonoCredit>
      <MonoPassage
        note={{
          side: "right",
          term: "Unicode 18.0",
          children:
            "Ferroni’s tables come from the current Unicode release; C’s last release ships 16.0.",
        }}
      >
        <p>
          Then the original’s own tests decide. Every test in Oniguruma’s suite that targets UTF-8
          is ported case by case, with the same patterns, texts and expected positions, and
          vscode-oniguruma’s scanner tests join them. Where Ferroni differs on purpose, the
          difference is written down: newer Unicode tables, and a backtracking stack that stops at a
          limit instead of running out of memory.
        </p>
      </MonoPassage>
      <MonoProofs items={PROOFS} />
    </MonoSection>
  );
}

export function Guardrails() {
  return (
    <MonoSection
      id="fr-safety"
      title="Powerful patterns. Practical guardrails."
      standfirst="Backtracking makes expressive patterns possible. Limits keep that work in hand when the text comes from outside."
    >
      <p>
        <MonoLeadIn>A backtracking engine</MonoLeadIn> tries one way to match, and when it fails, it
        goes back and tries another. That is what makes lookbehind and backreferences possible. With
        an unlucky pattern and hostile text, the number of ways explodes. Ferroni flags such
        patterns when they compile, can refuse them outright, and stops any search at a time or
        retry limit you set. Only Rust’s <code>regex</code> crate guarantees linear time; choose it
        when you need that guarantee.
      </p>
      <MonoPassage note={{ figure: "10", children: "unsafe sites, all linking parse-tree nodes" }}>
        <p>
          Buffer and string processing use safe Rust. The remaining unsafe code links parse-tree
          nodes under a documented policy. Four fuzzers exercise compilation, matching and the
          scanner on every pull request.
        </p>
      </MonoPassage>
      <GuardrailCards />
      <MonoCredit>
        Real output of ferroni {specimens.ferroni}. In browser WebAssembly there is no clock: use a
        retry limit there. <a href="/guide/untrusted-input">Untrusted input</a>
      </MonoCredit>
    </MonoSection>
  );
}

export function Scope() {
  return (
    <MonoSection id="fr-fit" title="A clear scope. A deliberate fit.">
      <p>
        Choose Ferroni when Oniguruma’s syntax matters to you. Keep your application’s encodings,
        runtime and safety needs in view: the port covers two of Oniguruma’s 29 encodings, and it
        leaves out the POSIX and GNU interfaces.
      </p>
      <MonoWide>
        <MonoTerms
          prose
          items={[
            {
              term: "Editors and tokenizers",
              description:
                "Multi-pattern scanning with byte or UTF-16 positions. Your runtime reads the grammar.",
            },
            {
              term: "Rust applications",
              description: "Keep your Oniguruma patterns without a C toolchain or bindgen.",
            },
            {
              term: "Native and WebAssembly",
              description: "Cargo builds both. There is no npm package.",
            },
            {
              term: "ASCII and UTF-8",
              description:
                "Shift_JIS, EUC-JP and the other legacy encodings stay with C. Ruby runs Onigmo.",
            },
          ]}
        />
      </MonoWide>
    </MonoSection>
  );
}
