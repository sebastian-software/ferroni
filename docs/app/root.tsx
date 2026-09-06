import { FAMILY_SITE, MarkDefs, SiteFooter } from "@ferramenta/family";
import {
  ArdoFooter,
  ArdoGeneratedSidebar,
  ArdoHeader,
  ArdoNav,
  ArdoNavLink,
  ArdoRoot,
  ArdoRootLayout,
  ArdoSidebar,
  ArdoSidebarSection,
} from "ardo/ui";
import config from "virtual:ardo/config";
import type { MetaFunction } from "react-router";
import "ardo/ui/styles.css";
import "@ferramenta/family/tokens.css";
import "@ferramenta/family/fonts.css";
import "@ferramenta/family/theme.css";
import "./site.css";
// Last on purpose (see the package README): the shared chrome has to win the
// ties the Ardo and site styles around it would otherwise take.
import "@ferramenta/family/chrome.css";

export const meta: MetaFunction = () => [{ title: config.title }];

export function Layout({ children }: { children: React.ReactNode }) {
  return <ArdoRootLayout>{children}</ArdoRootLayout>;
}

/**
 * The small print under the family columns: the version, license and build
 * lines the Ardo footer used to render on its own.
 */
function FooterLegal() {
  return (
    <>
      ferroni{config.project?.version ? ` v${config.project.version}` : ""} · Released under the
      BSD-2-Clause License · <a href="https://ardo-docs.dev">Built with Ardo</a>
      {config.buildTime ? (
        <>
          {" · Built on "}
          {new Date(config.buildTime).toLocaleDateString("en-US", {
            month: "long",
            day: "numeric",
            year: "numeric",
            timeZone: "UTC",
          })}
          {config.buildHash ? ` (${config.buildHash})` : ""}
        </>
      ) : null}
    </>
  );
}

export default function Root() {
  return (
    <ArdoRoot config={config}>
      <ArdoHeader>
        <ArdoNav>
          <ArdoNavLink to="/guide/getting-started">Guide</ArdoNavLink>
          <ArdoNavLink to="/perf/benchmark-results">Performance</ArdoNavLink>
          <ArdoNavLink to="/adr/001-one-to-one-parity-with-c-original">ADRs</ArdoNavLink>
          <ArdoNavLink href={FAMILY_SITE}>Ferramenta</ArdoNavLink>
        </ArdoNav>
      </ArdoHeader>

      <ArdoSidebar>
        <ArdoSidebarSection id="guide" label="Guide" to="/guide/getting-started">
          <ArdoGeneratedSidebar section="guide" />
        </ArdoSidebarSection>
        <ArdoSidebarSection id="perf" label="Performance" to="/perf/benchmark-results">
          <ArdoGeneratedSidebar section="perf" />
        </ArdoSidebarSection>
        <ArdoSidebarSection
          id="adr"
          label="Architecture Decision Records"
          to="/adr/001-one-to-one-parity-with-c-original"
        >
          <ArdoGeneratedSidebar section="adr" />
        </ArdoSidebarSection>
      </ArdoSidebar>

      {/*
        The shared family footer from `@ferramenta/family`. `ArdoRoot` only
        accepts an `ArdoFooter` element in its footer slot, so the family footer
        rides inside one and site.css strips the Ardo shell around it. Tool
        names, jobs and links come from the family registry, never from here.
      */}
      <ArdoFooter>
        <MarkDefs />
        <SiteFooter current="ferroni" legal={<FooterLegal />} />
      </ArdoFooter>
    </ArdoRoot>
  );
}
