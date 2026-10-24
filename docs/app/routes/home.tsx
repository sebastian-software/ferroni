// cspell:words mbstring
import { EvidenceFigures, family, Mark, ProjectHero, Section, WorkWithUs } from "ferramenta-family";
import { Link, type MetaFunction } from "react-router";
import config from "virtual:ardo/config";

import { comparison, factor, runDate, summary } from "../components/benchmark-summary";
import { SyntaxSection } from "../components/home-examples";
import { SpeedSummary } from "../components/speed-summary";
import {
  ClosingSection,
  FitSection,
  PrecompiledSection,
  SafetySection,
  ScannerSection,
  StartSection,
  StatusSection,
} from "./home-bottom-sections";
// React Router requires `meta` as a named route export.
// oxlint-disable-next-line react/only-export-components -- React Router requires this route export.
export const meta: MetaFunction = () => [
  { title: "Ferroni — Oniguruma regex in pure Rust" },
  {
    name: "description",
    content:
      "Oniguruma regex in pure Rust. Look-behind, backreferences, recursion and portable prepared scanner archives for native and WebAssembly apps.",
  },
];

function ferroni() {
  const tool = family.find((entry) => entry.name === "ferroni");
  if (tool === undefined) throw new Error("Ferroni is missing from the family registry.");
  return tool;
}

/* Ferroni's reuse throughput against C Oniguruma, low to high across the two platforms. */
function speedupOverC() {
  const values = [comparison("linux-x86-64", "c"), comparison("macos-arm64", "c")]
    .map((pair) => pair.family_weighted_ratio)
    .sort((a, b) => a - b);
  return `${values[0].toFixed(1)}–${factor(values[1])}`;
}

/*
 * What it succeeds and what it is checked against come from the registry; the
 * release is the version this site was built from, read from Cargo.toml.
 */
function HeroSection() {
  const tool = ferroni();
  const version = config.project?.version;
  return (
    <ProjectHero
      icon="ferroni"
      title={<span translate="no">Ferroni</span>}
      what="Your Oniguruma patterns. Pure Rust."
      lede={
        <>
          Keep the look-behind, backreferences and recursion your patterns rely on. Get a familiar
          Rust API and a scanner for syntax highlighters. Prepare scanner groups once and ship their
          portable archives to native and WebAssembly apps. Cargo handles the build, with no C
          compiler or bindgen.
        </>
      }
      actions={
        <>
          <Link to="/guide/getting-started" className="fam-btn fam-btn-primary">
            Run your first match <Mark name="arrow" className="icon" size={18} />
          </Link>
          <Link to="/guide/scanner" className="fam-btn fam-btn-ghost">
            Explore the Scanner
          </Link>
        </>
      }
      install={<code translate="no">cargo add ferroni</code>}
      facts={[
        { label: "Succeeds", value: tool.succeeds },
        { label: "Checked against", value: tool.evidence },
        ...(version == null ? [] : [{ label: "Release", value: `v${version}` }]),
      ]}
    />
  );
}

/*
 * The parity count is the total of the table in guide/compatibility.mdx.
 */
function EngineEvidence() {
  const arm = comparison("macos-arm64", "c").family_weighted_ratio;
  const x86 = comparison("linux-x86-64", "c").family_weighted_ratio;
  return (
    <EvidenceFigures
      figures={[
        {
          label: "Faster than C",
          value: factor(arm),
          detail: <>Reused patterns on ARM64; {factor(x86)} on x86-64</>,
        },
        {
          label: "Upstream tests",
          value: "2,974",
          detail: "Every UTF-8 test of C Oniguruma passes",
        },
        {
          label: "Syntax modes",
          value: "12",
          detail: "From Oniguruma and Ruby to Python",
        },
        {
          label: "Unicode",
          value: "18.0",
          detail: "C Oniguruma ships 16.0",
        },
      ]}
    />
  );
}

function WhySection() {
  return (
    <Section
      id="fr-why"
      layout="split"
      tone="dim"
      title="An established engine. A Rust future."
      intro={
        <>
          Oniguruma powers regex matching in TextMate grammars, PHP&rsquo;s mbstring and jq. Its C
          project closed in April 2025. Ferroni carries that work forward, ported function by
          function and checked against every upstream UTF-8 test. On the published workloads,
          compiled patterns run {speedupOverC()} faster than C Oniguruma.
        </>
      }
      note={
        <>
          Speed from the {runDate()} benchmark run.{" "}
          <Link to="/guide/compatibility">How compatibility is checked</Link>
        </>
      }
    >
      <EngineEvidence />
    </Section>
  );
}

function SpeedSection() {
  const jitWins = [comparison("macos-arm64", "pcre2_jit"), comparison("linux-x86-64", "pcre2_jit")]
    .map((pair) => pair.ferroni_cases)
    .sort((a, b) => a - b);
  return (
    <Section
      id="fr-speed"
      title="Compile once. Put it to work."
      intro={
        <>
          Ferroni runs compiled patterns in an interpreter and generates no machine code at runtime.
          On {summary.case_count} regex tasks taken from Django, Rails, WordPress and eight other
          projects, Ferroni is faster in each aggregate comparison except against PCRE2&rsquo;s JIT.
          Each pair uses the tasks both engines can run. Ferroni even leads the JIT on{" "}
          {jitWins[0] === jitWins[1] ? jitWins[0] : jitWins.join(" to ")} tasks across the two
          platforms. The chart shows the shared task counts and the tradeoffs.
        </>
      }
      note={<Link to="/perf/regex-workloads">How the benchmarks work</Link>}
    >
      <SpeedSummary />
    </Section>
  );
}

export default function HomePage() {
  return (
    <div className="fam-page ferroni-home">
      <HeroSection />
      <StartSection />
      <WhySection />
      <SyntaxSection />
      <ScannerSection />
      <PrecompiledSection />
      <SpeedSection />
      <SafetySection />
      <FitSection />
      <StatusSection />
      <ClosingSection />
      <WorkWithUs />
    </div>
  );
}
