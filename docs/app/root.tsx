import {
  ArdoErrorBoundary,
  ArdoGeneratedSidebar,
  ArdoRoot,
  ArdoRootLayout,
  ArdoSearch,
  ArdoSidebar,
  ArdoSidebarSection,
  ArdoThemeToggle,
} from "ardo/ui";
import { MarkDefs, SiteFooter, SiteHeader, SiteMenu } from "ferramenta-family";
import displayFont from "ferramenta-family/fonts/barlow-condensed-700.woff2?url";
import { type LinksFunction, type MetaFunction, NavLink, useHref, useLocation } from "react-router";
import config from "virtual:ardo/config";
import "ardo/ui/styles.css";
import "ferramenta-family/tokens.css";
import "ferramenta-family/fonts.css";
import "ferramenta-family/theme.css";
import "ferramenta-family/docs.css";
// The landing kit before the site's own stylesheet, so it adjusts the kit on ties.
import "ferramenta-family/landing.css";

import "./routes/home.css";
// Last on purpose (see the package README): the shared chrome has to win the
// ties the Ardo and site styles around it would otherwise take.
import "ferramenta-family/chrome.css";

// The display face sets every heading in the family chrome; preloading it
// avoids a swap on first paint, as on the Ferromark and Ferriki sites.
// oxlint-disable-next-line react/only-export-components -- React Router requires this route export.
export const links: LinksFunction = () => [
  {
    rel: "preload",
    href: displayFont,
    as: "font",
    type: "font/woff2",
    crossOrigin: "anonymous",
  },
];

// React Router requires `meta` as a named route export.
// oxlint-disable-next-line react/only-export-components -- React Router requires this route export.
export const meta: MetaFunction = () => [{ title: config.title }];

// React Router consumes `Layout` as a named route export.
// oxlint-disable-next-line react/only-export-components -- React Router requires this route export.
export function Layout({ children }: { children: React.ReactNode }) {
  return <ArdoRootLayout>{children}</ArdoRootLayout>;
}

// A render error shows Ardo's error page inside the family chrome.
export const ErrorBoundary = ArdoErrorBoundary;

/*
 * The family chrome replaces Ardo's own header and footer, so Ardo must not
 * render either: `chrome` is read from every route match, and no route below
 * this one overrides it. The sidebar is not part of that switch -- the docs
 * rail and its generated navigation stay exactly as they were.
 */
// oxlint-disable-next-line react/only-export-components -- Ardo reads this route handle.
export const handle = { chrome: false };

type DocsSection = {
  id: string;
  /** The name in the header bar and the narrow-viewport menu. */
  label: string;
  /** The name on the sidebar rail, where there is room for the long form. */
  sidebarLabel?: string;
  to: string;
};

/*
 * The three sections of this site, in reading order. One list feeds all three
 * places that name them: the links in the header bar, the menu that stands in
 * for those links on a narrow viewport, and the sidebar rail.
 */
const sections: DocsSection[] = [
  { id: "guide", label: "Guide", to: "/guide/getting-started" },
  { id: "perf", label: "Performance", to: "/perf/benchmark-results" },
  {
    id: "adr",
    label: "ADRs",
    sidebarLabel: "Architecture Decision Records",
    to: "/adr/001-one-to-one-parity-with-c-original",
  },
];

/** The section links, for the bar where there is room and for the menu where there is not. */
function SectionLinks() {
  return sections.map((section) => (
    <NavLink key={section.id} to={section.to}>
      {section.label}
    </NavLink>
  ));
}

/**
 * The small print after the workshop's copyright line, which the family footer
 * carries itself: the version, license and build lines the Ardo footer used to
 * render on its own.
 */
function FooterLegal() {
  return (
    <>
      {`Ferroni${config.project?.version != null ? ` v${config.project.version}` : ""}`} · Released
      under the BSD-2-Clause License · <a href="https://ardo-docs.dev">Built with Ardo</a>
      {config.buildTime != null ? (
        <>
          {" · Built on "}
          {new Date(config.buildTime).toLocaleDateString("en-US", {
            month: "long",
            day: "numeric",
            year: "numeric",
            timeZone: "UTC",
          })}
          {config.buildHash != null ? ` (${config.buildHash})` : ""}
        </>
      ) : null}
    </>
  );
}

/** The family header with this site's sections, search and, on the documentation, the theme toggle. */
function DocsHeader() {
  const home = useHref("/");
  // The landing page has one authored scheme; the documentation keeps Ardo's.
  const landing = useLocation().pathname === home;
  return (
    <SiteHeader
      current="ferroni"
      lockup="project"
      home={home}
      nav={
        <div className="site-links">
          <SectionLinks />
        </div>
      }
      /*
       * Below 64rem Ardo hides the sidebar rail and the links above give up
       * their room: the menu holding the same three sections is then the only
       * way into the documentation. `ArdoSearch` reads its index from a
       * virtual module and falls back to the default labels, so it works
       * outside `ArdoRoot`'s provider.
       */
      actions={
        <>
          <SiteMenu label="Docs">
            <SectionLinks />
          </SiteMenu>
          <div className="site-search">
            <ArdoSearch />
          </div>
        </>
      }
      themeToggle={landing ? undefined : <ArdoThemeToggle />}
    />
  );
}

export default function Root() {
  return (
    <>
      <MarkDefs />
      <DocsHeader />

      {/*
        `fam-docs-shell` is the hook docs.css needs to turn Ardo's
        fixed-viewport application shell into a document-scrolling page: the
        family footer sits below the shell, so the page -- not the article --
        has to be what scrolls.
      */}
      <div className="fam-docs-shell">
        <ArdoRoot config={config}>
          <ArdoSidebar>
            {sections.map((section) => (
              <ArdoSidebarSection
                key={section.id}
                id={section.id}
                label={section.sidebarLabel ?? section.label}
                to={section.to}
              >
                <ArdoGeneratedSidebar section={section.id} />
              </ArdoSidebarSection>
            ))}
          </ArdoSidebar>
        </ArdoRoot>
      </div>

      {/*
        The shared family footer, a sibling of the shell rather than a child of
        Ardo's own footer. Tool names, jobs and links come from the family
        registry, never from here.
      */}
      <SiteFooter current="ferroni" legal={<FooterLegal />} />
    </>
  );
}
