// cspell:words subexpression
import { family, Mark, ProjectHero, Relations, Section, WorkWithUs } from "ferramenta-family";
import { Link, type MetaFunction } from "react-router";
import config from "virtual:ardo/config";

import { FitSection, StatusSection, SyntaxSection } from "../components/home-examples";
import { HomeNavigation } from "../components/home-navigation";
import { HomeRunSample } from "../components/home-run-sample";
import { ProductionSummary } from "../components/production-summary";
import {
  ClosingSection,
  CoverageSection,
  ProofSection,
  SafetySection,
  ScannerSection,
} from "./home-bottom-sections";
// React Router requires `meta` as a named route export.
// oxlint-disable-next-line react/only-export-components -- React Router requires this route export.
export const meta: MetaFunction = () => [
  { title: "Ferroni — Oniguruma regex in pure Rust" },
  {
    name: "description",
    content:
      "Match, extract and replace UTF-8 text with Oniguruma patterns in pure Rust. Lookaround, backreferences, upstream compatibility tests and benchmarks from real projects. No C compiler required.",
  },
];

function ferroni() {
  const tool = family.find((entry) => entry.name === "ferroni");
  if (tool === undefined) throw new Error("Ferroni is missing from the family registry.");
  return tool;
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
      what="Oniguruma regex in pure Rust."
      lede={
        <>
          Match, extract and replace UTF-8 text with look-behind, backreferences and subexpression
          calls. Ferroni ports Oniguruma&rsquo;s engine to Rust, checked against its upstream tests.
          Add it with Cargo; no C compiler required.
        </>
      }
      actions={
        <>
          <Link to="/guide/getting-started" className="fam-btn fam-btn-primary">
            Read the quickstart <Mark name="arrow" className="icon" size={18} />
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

function SampleSection() {
  return (
    <Section
      id="fr-run"
      title="Look-behind and named captures, in Rust"
      intro={
        <>
          Select a date with look-behind, then read its named captures with a familiar Rust API. The
          output below comes from running this example.
        </>
      }
      note={
        <a href="https://github.com/sebastian-software/ferroni/blob/main/examples/website_sample.rs">
          Run the example: cargo run --example website_sample
        </a>
      }
    >
      <HomeRunSample />
    </Section>
  );
}

function PerformanceSection() {
  return (
    <Section
      id="fr-evidence"
      title="See how Ferroni compares"
      intro={
        <>
          Measure the regex work your application needs: validation, capture extraction, replacement
          and Unicode text processing. These comparisons use patterns and test inputs from projects
          including Django, Fluent Bit, Rails and WordPress, covering simple expressions, complex
          patterns and advanced Oniguruma features.
        </>
      }
      note={
        <>
          The measured work is each project&rsquo;s regex step. Complete validators, parsers and
          rendering pipelines include additional work.
        </>
      }
    >
      <ProductionSummary />
      <p>
        <Link to="/perf/regex-workloads">
          Inspect every case, preparation costs and measurement ranges
        </Link>
      </p>
    </Section>
  );
}

function RelationsSection() {
  return (
    <Section
      id="fr-relations"
      title="Where Ferroni sits"
      intro="Ferroni supplies the regex engine for Ferriki, whose Shiki-compatible highlighter tokenizes code with TextMate grammars. Each tool also works on its own."
    >
      <Relations current="ferroni" />
    </Section>
  );
}

export default function HomePage() {
  return (
    <div className="fam-page ferroni-home">
      <HeroSection />
      <HomeNavigation />
      <FitSection />
      <SampleSection />
      <SyntaxSection />
      <PerformanceSection />
      <ScannerSection />
      <SafetySection />
      <ProofSection />
      <CoverageSection />
      <StatusSection />
      <RelationsSection />
      <ClosingSection />
      <WorkWithUs />
    </div>
  );
}
