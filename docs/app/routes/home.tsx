// cspell:words mbstring
import type { ArdoMetaFunction } from "ardo/runtime";

import { ArdoLink as Link } from "ardo/runtime";
import { Mark, RegistryFacts } from "ferramenta-family";
import {
  FerraAction,
  FerraArticle,
  FerraColumns,
  FerraEditorialRow,
  FerraInstrument,
  FerraNotes,
  FerraProductLead,
  FerraProductPage,
  FerraReleaseFacts,
} from "ferramenta-family/ferra";
import { family, WORKSHOP } from "ferramenta-family/registry";

import { comparison, factor, summary } from "../components/benchmark-summary";
import { SyntaxSection } from "../components/home-examples";
import {
  ClosingSection,
  FitSection,
  PrecompiledSection,
  SafetySection,
  ScannerSection,
  StartSection,
  StatusSection,
} from "../components/home-sections";
import { version } from "../data/site.json";

// oxlint-disable-next-line react/only-export-components -- Ardo reads route metadata.
export const meta: ArdoMetaFunction = () => [
  { title: "Ferroni — Oniguruma regex in pure Rust" },
  {
    name: "description",
    content:
      "Keep your Oniguruma patterns in native Rust: lookbehind, backreferences, Unicode and a TextMate scanner. No C toolchain required.",
  },
];
// oxlint-disable-next-line react/only-export-components -- Ardo reads the route layout.
export const handle = { layout: "bare" };

const entry = family.find((member) => member.name === "ferroni");
if (entry === undefined) throw new Error("Ferroni is missing from the family registry.");
const tool = { ...entry, version };

function speedupOverC() {
  const values = [comparison("linux-x86-64", "c"), comparison("macos-arm64", "c")]
    .map((pair) => pair.family_weighted_ratio)
    .sort((a, b) => a - b);
  return `${values[0].toFixed(1)}–${factor(values[1])}`;
}

function Opening() {
  return (
    <FerraProductLead
      id="overview"
      title="Your Oniguruma patterns. Pure Rust."
      introduction="Keep the lookbehind, backreferences and Unicode matching your application relies on. Ferroni brings Oniguruma to native Rust, with a familiar API and a scanner for syntax highlighters. Cargo handles the build."
      art={<FerraInstrument product="ferroni" />}
      byline={
        <>
          An open-source project by <a href={WORKSHOP.openSource}>{WORKSHOP.company}</a>.
        </>
      }
    >
      <div className="ferra-actions">
        <FerraAction href="/guide/getting-started">Run your first match</FerraAction>
        <FerraAction variant="secondary" href="/guide/scanner">
          Build a scanner
        </FerraAction>
      </div>
    </FerraProductLead>
  );
}

function Overview() {
  return (
    <FerraColumns>
      <FerraArticle
        title="The syntax you came for."
        icon={<Mark name="pen" size={28} />}
        meta={<Link to="/guide/compatibility">Read the compatibility contract</Link>}
      >
        <p>
          Lookbehind, named captures, backreferences and recursion, across all twelve syntax modes.
          Ferroni passes every upstream UTF-8 test and keeps the original module structure close at
          hand.
        </p>
        <a href="#fr-syntax">See the patterns in action</a>
      </FerraArticle>
      <FerraArticle
        title="Built to keep searching."
        icon={<Mark name="document" size={28} />}
        meta={<FerraReleaseFacts tool={tool} />}
      >
        <p>
          Compile a pattern once, then reuse it. On our recorded workloads, Ferroni delivered{" "}
          {speedupOverC()} the matching throughput of C Oniguruma on ARM64 macOS and x86-64 Linux.
        </p>
        <a href="#fr-speed">See the measurements and tradeoffs</a>
      </FerraArticle>
    </FerraColumns>
  );
}

function WhySection() {
  return (
    <FerraArticle
      id="fr-why"
      title="An established engine. A Rust future."
      introduction="TextMate grammars, PHP’s mbstring and jq all draw on Oniguruma. Its C project closed in April 2025. Ferroni carries that work forward."
      meta={<Link to="/guide/compatibility">What carries over, and what changes</Link>}
    >
      <p>
        Rewriting a grammar to fit another regex engine can change what it matches. Ferroni ports
        the original instead, function by function, then checks the result against its upstream
        tests. The Rust API adds ownership and familiar capture methods around that core.
      </p>
      <p>
        The scope is deliberate: ASCII and UTF-8, plus the scanner that editors drive. You can adopt
        Ferroni on its own; no other Ferramenta engine is required.
      </p>
    </FerraArticle>
  );
}

function SpeedSection() {
  const compile = [
    comparison("macos-arm64", "c", "compile"),
    comparison("linux-x86-64", "c", "compile"),
  ]
    .map((pair) => 1 / pair.family_weighted_ratio)
    .sort((a, b) => a - b);
  return (
    <FerraArticle
      id="fr-speed"
      title="Compile once. Put it to work."
      introduction="Useful comparisons start with the work you actually do. Ours use regex tasks from Django, Rails, WordPress and eight other projects."
      meta={
        <Link to="/perf/regex-workloads">Explore the benchmarks, platforms and source data</Link>
      }
    >
      <p>
        Across {summary.case_count} tasks, Ferroni leads every measured engine in its aggregate
        reuse comparison except PCRE2’s JIT. Each comparison covers only the tasks both engines can
        run, with matching output checked before timing. These results describe isolated regex work,
        not whole applications.
      </p>
      <p>
        Reuse is the important qualification. C Oniguruma compiles patterns {compile[0].toFixed(1)}–
        {factor(compile[1])} faster on the measured aggregate. If you create a pattern for a single
        search, check compilation and first-use results too. Ferroni interprets compiled patterns
        and generates no machine code at runtime.
      </p>
    </FerraArticle>
  );
}

export default function HomePage() {
  return (
    <RegistryFacts>
      <FerraProductPage>
        <Opening />
        <Overview />
        <FerraEditorialRow
          notes={
            <FerraNotes label="Inside the port" notes={tool.editorial?.notes.slice(0, 1) ?? []} />
          }
        >
          <WhySection />
        </FerraEditorialRow>
        <StartSection />
        <SyntaxSection />
        <FerraEditorialRow
          notes={
            <FerraNotes label="Inside the scanner" notes={tool.editorial?.notes.slice(1) ?? []} />
          }
        >
          <ScannerSection />
        </FerraEditorialRow>
        <PrecompiledSection />
        <SpeedSection />
        <SafetySection />
        <FitSection />
        <StatusSection />
        <ClosingSection />
      </FerraProductPage>
    </RegistryFacts>
  );
}
