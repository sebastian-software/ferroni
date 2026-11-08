// cspell:words mbstring Kosako Onigmo
import {
  MonoCard,
  MonoCards,
  MonoCredit,
  MonoPassage,
  MonoSection,
  MonoVerdict,
  MonoWide,
} from "ferramenta-family/monografie";

import { CaptureSpecimen } from "./capture-specimen";
import { FeatureSpecimens } from "./feature-specimens";
import { featureCount, scannerSource, spelled } from "./home-specimen-data";
import { ScannerWalkthrough } from "./scanner-walkthrough";

const capitalized = (text: string) => `${text[0].toUpperCase()}${text.slice(1)}`;

/* The home page's reading chapters: what a regex is, the syntax, the scanner. */

export function Explainer() {
  return (
    <MonoSection id="fr-run" title="A pattern describes the text you want.">
      <MonoPassage
        note={{
          term: "Regular expression",
          children:
            "A pattern that describes text: digits, words, repetition, what comes before and after.",
        }}
      >
        <p>
          A regular expression is a small language for finding text.{" "}
          <code>{String.raw`\d{4}`}</code> means four digits. <code>(?&lt;year&gt;…)</code> gives
          the part it found a name. Search boxes, form checks, log filters and code editors all
          speak it. Most patterns are simple. Some need to look behind a label, refer back to a word
          they already found, or recognize the letters of one script.
        </p>
      </MonoPassage>
      <MonoPassage
        note={{
          side: "right",
          term: "Oniguruma",
          children:
            "K. Kosako’s regex engine, written in C, behind TextMate grammars, PHP’s mbstring and jq.",
        }}
      >
        <p>
          Oniguruma is the engine many of those patterns were written for. It reads the TextMate
          grammars that VS Code, Shiki and many other tools use to color code. It powers PHP’s
          mbstring and the jq command-line tool. When its C project closed, those patterns lost
          their maintained home.
        </p>
      </MonoPassage>
      <p>
        Ferroni gives them a new one in Rust: the same syntax and the same matches, behind an API
        that feels familiar to Rust developers. Cargo builds it, for native targets and for
        WebAssembly.
      </p>
      <CaptureSpecimen />
    </MonoSection>
  );
}

export function Syntax() {
  return (
    <MonoSection
      id="fr-syntax"
      title="Give your patterns room to work."
      standfirst={`Look behind a label, repeat a word you found, select a script, fold ß into ss. ${capitalized(spelled(featureCount.total))} features, each checked by a test that runs.`}
    >
      <p>
        Not every engine reads every pattern. Rust’s own <code>regex</code> crate leaves out
        lookbehind and backreferences on purpose, so it can promise to finish in linear time. That
        is the right choice for patterns that strangers type. For grammars and code written for
        Oniguruma, a missing feature changes what a pattern means, or rejects it outright.
      </p>
      <MonoPassage
        note={{
          side: "right",
          term: "Twelve syntax modes",
          children:
            "Oniguruma, Ruby, Perl, Python, Java, POSIX and more. Ferroni includes all twelve.",
        }}
      >
        <p>
          Ferroni passes every check in the comparison below, the only Rust engine that does. Each
          match in the table is Ferroni’s real output for that pattern and text.
        </p>
      </MonoPassage>
      <MonoWide>
        <FeatureSpecimens />
        <MonoCredit>
          Every Yes and No is an executable check in <code>tests/regex_feature_matrix.rs</code>.
          Ruby’s Onigmo is part of the{" "}
          <a href="/perf/regex-workloads#syntax-coverage">full syntax comparison</a>.
        </MonoCredit>
      </MonoWide>
    </MonoSection>
  );
}

/** The same scanner in Rust, and the sibling engine that runs on it. */
function ScannerCards() {
  return (
    <MonoWide>
      <MonoCards layout="lead">
        <MonoCard
          className="fr-scanner-code"
          title="The same scanner in Rust"
          api="Scanner::new, find_next_match"
        >
          <pre>{scannerSource}</pre>
        </MonoCard>
        <MonoCard title="Underneath Ferriki" api="Shiki-compatible highlighting">
          <MonoVerdict style={{ marginTop: 0 }}>
            Ferriki, a sibling engine, runs TextMate grammars on this scanner and returns
            highlighted HTML. You can build your own tokenizer the same way; no other Ferramenta
            engine is required.
          </MonoVerdict>
          <MonoVerdict>
            <a href="https://ferriki.dev">ferriki.dev</a>
          </MonoVerdict>
        </MonoCard>
      </MonoCards>
    </MonoWide>
  );
}

export function Scanner() {
  return (
    <MonoSection
      id="fr-scanner"
      title="Your next highlighter starts here."
      standfirst="Give the scanner a set of patterns and ask which one matches next. You handle the grammar; Ferroni handles the regex work."
    >
      <p>
        Syntax highlighting repeats one small race thousands of times per file. At each position,
        every pattern the grammar allows right now is tried, and the earliest match wins. The next
        search starts where that match ended. Ferroni’s Scanner follows the model of
        vscode-oniguruma, the engine inside VS Code’s highlighting.
      </p>
      <MonoPassage
        note={{
          side: "right",
          term: "UTF-16 positions",
          children:
            "Editors written in JavaScript count text in UTF-16 units, not bytes. The scanner reports both.",
        }}
      >
        <p>
          The positions come back the way an editor counts them. On a plain ASCII line, bytes and
          UTF-16 units agree. Add one ß and they part ways; switch to the second line to see it.
        </p>
      </MonoPassage>
      <ScannerWalkthrough />
      <ScannerCards />
    </MonoSection>
  );
}
