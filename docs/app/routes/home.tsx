// cspell:words mbstring
import {
  EvidenceFigures,
  family,
  Mark,
  Measured,
  ProjectHero,
  Section,
  WorkWithUs,
} from "ferramenta-family";
import { Link, type MetaFunction } from "react-router";
import config from "virtual:ardo/config";

import { comparison, factor, runDate, summary } from "../components/benchmark-summary";
import { SyntaxSection } from "../components/home-examples";
import { SpeedChart } from "../components/speed-chart";
import {
  ClosingSection,
  FitSection,
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
      "Oniguruma's regex engine in memory-safe Rust: look-behind, backreferences, recursion and the TextMate scanner, checked against every upstream UTF-8 test and faster than the C original on reused patterns. No C compiler required.",
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
      what="Oniguruma’s regex engine, in memory-safe Rust."
      lede={
        <>
          Run Oniguruma&rsquo;s full syntax, from look-behind to recursion, and the scanner that
          syntax highlighters drive. Ferroni passes every upstream UTF-8 test and, once a pattern is
          compiled, matches {speedupOverC()} faster than the C original.
        </>
      }
      actions={
        <>
          <Link to="/guide/getting-started" className="fam-btn fam-btn-primary">
            Get started <Mark name="arrow" className="icon" size={18} />
          </Link>
          <a href="https://github.com/sebastian-software/ferroni" className="fam-btn fam-btn-ghost">
            <Mark name="github" className="icon" size={18} /> GitHub
          </a>
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
 * The four claims the page then argues one by one. The parity count is the
 * total of the table in guide/compatibility.mdx; the unsafe count is the
 * output of scripts/count-unsafe.sh, as in ADR-002.
 */
function WhySection() {
  const arm = comparison("macos-arm64", "c").family_weighted_ratio;
  const x86 = comparison("linux-x86-64", "c").family_weighted_ratio;
  return (
    <Section
      id="fr-why"
      layout="split"
      title="Oniguruma ended. The engine goes on."
      intro={
        <>
          Oniguruma matches the patterns in every TextMate grammar, in PHP&rsquo;s mbstring and in
          jq. Its C project closed in April 2025. Ferroni continues the engine in Rust, ported
          function by function, and makes it faster and safer than it was.
        </>
      }
      note={
        <>
          Speed from the {runDate()} benchmark run.{" "}
          <Link to="/guide/compatibility">How compatibility is checked</Link>
        </>
      }
    >
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
            label: "Unsafe sites",
            value: "10",
            detail: "None in buffer or string code",
          },
          {
            label: "Unicode",
            value: "18.0",
            detail: "C Oniguruma ships 16.0",
          },
        ]}
      />
    </Section>
  );
}

/* The cost side of the speed claim: C prepares a pattern faster on both platforms. */
function CompileTradeoff() {
  const compile = [
    comparison("macos-arm64", "c", "compile"),
    comparison("linux-x86-64", "c", "compile"),
  ]
    .map((pair) => 1 / pair.family_weighted_ratio)
    .sort((a, b) => a - b);
  return (
    <p className="fr-speed-tradeoff">
      <strong>Where C still wins: compiling.</strong> C Oniguruma prepares a pattern{" "}
      {compile[0].toFixed(1)}–{factor(compile[1])} faster, so a pattern used once favors C. Ferroni
      pays off when a pattern runs many times, as in highlighters, parsers and validators.
    </p>
  );
}

function SpeedProvenance() {
  return (
    <Measured
      on={runDate()}
      machine="Blacksmith VMs, ARM64 macOS and x86-64 Linux"
      revision={
        <a href={`https://github.com/sebastian-software/ferroni/commit/${summary.run.head_sha}`}>
          {summary.run.head_sha.slice(0, 7)}
        </a>
      }
      more={[
        { label: "Corpus", value: `${summary.case_count} tasks, ${summary.input_count} inputs` },
        {
          label: "Repetitions",
          value: `${summary.platforms[0].repetitions} per platform, one VM each`,
        },
      ]}
    >
      <Link to="/perf/regex-workloads">Every task, phase and range</Link>
    </Measured>
  );
}

function SpeedSection() {
  const jitWins = [comparison("macos-arm64", "pcre2_jit"), comparison("linux-x86-64", "pcre2_jit")]
    .map((pair) => pair.ferroni_cases)
    .sort((a, b) => a - b);
  return (
    <Section
      id="fr-speed"
      tone="dim"
      title="Only a JIT is faster"
      intro={
        <>
          Ferroni runs compiled patterns in an interpreter and generates no machine code at runtime.
          On {summary.case_count} regex tasks taken from Django, Rails, WordPress and seven other
          projects, it outpaces every engine measured except PCRE2&rsquo;s JIT, and still wins{" "}
          {jitWins[0] === jitWins[1] ? jitWins[0] : jitWins.join(" to ")} of those tasks.
        </>
      }
    >
      <SpeedChart />
      <CompileTradeoff />
      <SpeedProvenance />
    </Section>
  );
}

export default function HomePage() {
  return (
    <div className="fam-page ferroni-home">
      <HeroSection />
      <WhySection />
      <SpeedSection />
      <SyntaxSection />
      <ScannerSection />
      <SafetySection />
      <StartSection />
      <FitSection />
      <StatusSection />
      <ClosingSection />
      <WorkWithUs />
    </div>
  );
}
