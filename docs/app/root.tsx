import { MarkDefs, SiteFooter, SiteHeader } from "ferramenta-family";
import {
  ArdoGeneratedSidebar,
  ArdoRoot,
  ArdoRootLayout,
  ArdoSearch,
  ArdoSidebar,
  ArdoSidebarSection,
  ArdoThemeToggle,
} from "ardo/ui";
import config from "virtual:ardo/config";
import { useRef } from "react";
import { NavLink, type MetaFunction } from "react-router";
import "ardo/ui/styles.css";
import "ferramenta-family/tokens.css";
import "ferramenta-family/fonts.css";
import "ferramenta-family/theme.css";
import "./site.css";
// Last on purpose (see the package README): the shared chrome has to win the
// ties the Ardo and site styles around it would otherwise take.
import "ferramenta-family/chrome.css";

export const meta: MetaFunction = () => [{ title: config.title }];

export function Layout({ children }: { children: React.ReactNode }) {
  return <ArdoRootLayout>{children}</ArdoRootLayout>;
}

/*
 * The family chrome replaces Ardo's own header and footer, so Ardo must not
 * render either: `chrome` is read from every route match, and no route below
 * this one overrides it. The sidebar is not part of that switch -- the docs
 * rail and its generated navigation stay exactly as they were.
 */
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

/** The site's own navigation, in the header's `nav` slot. */
function DocsNav() {
  return (
    <div className="ferroni-nav">
      {sections.map((section) => (
        <NavLink key={section.id} to={section.to}>
          {section.label}
        </NavLink>
      ))}
    </div>
  );
}

/*
 * The controls a documentation site keeps in the bar, in the header's
 * `actions` slot: full-text search, and -- below 1024px, where Ardo hides the
 * sidebar rail and the links above give up their room -- a menu holding the
 * same three sections, which is then the only way into the documentation.
 * `ArdoSearch` reads its index from a virtual module and falls back to the
 * default labels, so it works outside `ArdoRoot`'s provider.
 */
function DocsTools() {
  const menu = useRef<HTMLDetailsElement>(null);

  return (
    <>
      <details className="ferroni-sections" ref={menu}>
        {/* No aria-label: the visible word is the accessible name, so a voice
            command for what is on screen reaches the control. */}
        <summary>Docs</summary>
        <div className="ferroni-sections-flyout">
          {sections.map((section) => (
            <NavLink
              key={section.id}
              to={section.to}
              /*
               * Client-side navigation keeps the page mounted, so the menu has
               * to close itself when one of its links is taken.
               */
              onClick={() => menu.current?.removeAttribute("open")}
            >
              {section.label}
            </NavLink>
          ))}
        </div>
      </details>
      <div className="ferroni-search">
        <ArdoSearch />
      </div>
    </>
  );
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
    <>
      <MarkDefs />
      <SiteHeader
        current="ferroni"
        nav={<DocsNav />}
        actions={<DocsTools />}
        themeToggle={<ArdoThemeToggle />}
      />

      {/*
        `ferroni-shell` is the hook site.css needs to turn Ardo's
        fixed-viewport application shell into a document-scrolling page: the
        family footer sits below the shell, so the page -- not the article --
        has to be what scrolls.
      */}
      <div className="ferroni-shell">
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
