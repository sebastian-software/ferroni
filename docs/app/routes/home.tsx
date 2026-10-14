import { family, Mark, ProjectHero, Relations, Section, WorkWithUs } from "ferramenta-family";
import { Link, type MetaFunction } from "react-router";
import config from "virtual:ardo/config";

import { FitSection, StatusSection, SyntaxSection } from "../components/home-examples";
import { HomeNavigation } from "../components/home-navigation";
import { HomeRunSample } from "../components/home-run-sample";
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
  { title: "Ferroni — Oniguruma's regex engine, modernized in Rust" },
  {
    name: "description",
    content:
      "Oniguruma-compatible matching, extraction and replacement in Rust, including lookaround and backreferences. No C compiler, verified against upstream tests, with a TextMate Scanner API for grammar consumers.",
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
      what="Oniguruma's regex engine, modernized in Rust."
      lede={
        <>
          Match, extract and replace text with Oniguruma patterns in pure Rust, including
          look-behind and backreferences. No C compiler. Verified against Oniguruma&rsquo;s upstream
          tests.
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
      title="Familiar Rust, full Oniguruma syntax"
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

function SpeedSection() {
  return (
    <Section
      id="fr-evidence"
      layout="split"
      title="Compare the work your application needs"
      intro={
        <>
          Our regex corpus separates simple expressions, complex expressions and advanced Oniguruma
          features. Patterns come from identifiable projects, with pinned sources, explicit
          operation boundaries and checked results. TextMate Scanner replays have their own
          workload.
        </>
      }
      note={
        <>
          Repeated Blacksmith measurements are available for ARM64/macOS and x86-64/Linux. Inspect
          compile, first-use and steady-reuse costs, with run ranges, rejected syntax and differing
          results listed explicitly.
        </>
      }
    >
      <p>
        <Link to="/perf/regex-workloads">
          Inspect measured results, workloads and comparison rules
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
      <SpeedSection />
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
