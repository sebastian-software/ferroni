import { type ReactNode, useState } from "ardo/react";
import { ArdoActiveLink as NavLink, useArdoPageData, useArdoPath } from "ardo/runtime";
import {
  ArdoErrorBoundary,
  ArdoHeaderSearch,
  ArdoMobileNavigation,
  ArdoRoot,
  ArdoRootLayout,
  ArdoTOC,
} from "ardo/ui";
import { Mark, MarkDefs } from "ferramenta-family";
import touchIcon from "ferramenta-family/ferra-assets/ferroni/apple-touch-icon.png?url";
import favicon from "ferramenta-family/ferra-assets/ferroni/favicon.ico?url";
import {
  MonoFooter,
  MonoGuideNavigation,
  MonoHeader,
  MonoPage,
  MonoReadingLayout,
  MonoReadingTable,
  MonoSectionRail,
} from "ferramenta-family/monografie";
import "ferramenta-family/monografie.css";
import "ferramenta-family/monografie-docs.css";
import "ferramenta-family/monografie-ardo.css";

// Two fits for the documentation pages, under fr-docs: the header's row and code comments.
import "./components/docs-pages.css";
import { navigation, version } from "./data/site.json";

export function Layout({ children }: { children: ReactNode }) {
  return (
    <ArdoRootLayout
      colorScheme="light"
      className="mono-ardo"
      icon={favicon}
      appleTouchIcon={touchIcon}
      iconBasePath={false}
    >
      {children}
    </ArdoRootLayout>
  );
}
export const ErrorBoundary = ArdoErrorBoundary;

function GuideNavigation({ onNavigate }: { onNavigate?: () => void }) {
  const path = useArdoPath();
  const current = navigation.find((section) => path.startsWith(`/${section.id}/`)) ?? navigation[0];
  return (
    <>
      <MonoSectionRail aria-label="Documentation sections">
        {navigation.map((section) => (
          <NavLink
            key={section.id}
            to={section.href}
            data-active={current.id === section.id}
            onClick={onNavigate}
          >
            <Mark name={section.icon} />
            {section.label}
          </NavLink>
        ))}
      </MonoSectionRail>
      <MonoGuideNavigation aria-label="Documentation pages">
        <NavLink to="/" onClick={onNavigate}>
          Overview
        </NavLink>
        <p>{current.label}</p>
        {current.pages.map(({ title, href }) => (
          <NavLink key={href} to={href} onClick={onNavigate}>
            {title}
          </NavLink>
        ))}
      </MonoGuideNavigation>
    </>
  );
}

/** The documentation menu and search, which stay in the header on documentation pages. */
function DocsActions() {
  const [open, setOpen] = useState(false);
  return (
    <>
      <ArdoMobileNavigation title="Ferroni documentation" open={open} onOpenChange={setOpen}>
        <GuideNavigation
          onNavigate={() => {
            setOpen(false);
          }}
        />
      </ArdoMobileNavigation>
      <ArdoHeaderSearch
        variant="icon"
        onActivate={() => {
          setOpen(false);
        }}
      />
    </>
  );
}

function Header({ docs }: { docs: boolean }) {
  return (
    <MonoHeader
      product="ferroni"
      home="/"
      release={version}
      links={[
        <NavLink key="guide" to="/guide/getting-started">
          Guide
        </NavLink>,
        <NavLink key="scanner" to="/guide/scanner">
          Scanner
        </NavLink>,
        <a key="speed" href="/#fr-speed">
          Benchmarks
        </a>,
        <NavLink key="compatibility" to="/guide/compatibility">
          Compatibility
        </NavLink>,
        <a key="api" href="https://docs.rs/ferroni">
          API
        </a>,
        <a key="github" href="https://github.com/sebastian-software/ferroni">
          GitHub
        </a>,
      ]}
      actions={docs ? <DocsActions /> : undefined}
    />
  );
}

function Footer() {
  return (
    <MonoFooter product="ferroni" home="/">
      <a href="https://github.com/sebastian-software/ferroni">GitHub</a>
      <a href="https://crates.io/crates/ferroni">crates.io</a>
      <a href="https://docs.rs/ferroni">docs.rs</a>
    </MonoFooter>
  );
}

function Article({ children }: { children: ReactNode }) {
  const page = useArdoPageData();
  const path = useArdoPath();
  const current = navigation.find((section) => path.startsWith(`/${section.id}/`));
  return (
    <MonoReadingLayout
      id="ferroni-article"
      navigation={<GuideNavigation />}
      outline={<ArdoTOC source="dom" scope="#ferroni-article" />}
    >
      <header>
        <p>Ferroni / {current?.label ?? "Documentation"}</p>
        <h1>{page?.title ?? "Documentation"}</h1>
      </header>
      {children}
    </MonoReadingLayout>
  );
}
const components = { table: MonoReadingTable, wrapper: Article };

/**
 * Every page wears the Monografie header and footer and reads on the Monografie
 * page ground; documentation adds the reading layout.
 */
export default function Root() {
  const path = useArdoPath();
  const home = path === "/" || path === "";
  const shell = (
    <>
      <MarkDefs />
      <ArdoRoot
        className="mono-ardo-shell"
        scrollMode="document"
        header={<Header docs={!home} />}
        sidebar={false}
        footer={false}
        components={components}
      />
      <Footer />
    </>
  );
  return home ? (
    <MonoPage className="fr-home">{shell}</MonoPage>
  ) : (
    <MonoPage className="mono-docs fr-docs">{shell}</MonoPage>
  );
}
