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
import {
  FerraGuideNavigation,
  FerraPage,
  FerraReadingLayout,
  FerraReadingTable,
  FerraSectionRail,
} from "ferramenta-family/ferra";
import touchIcon from "ferramenta-family/ferra-assets/ferroni/apple-touch-icon.png?url";
import favicon from "ferramenta-family/ferra-assets/ferroni/favicon.ico?url";
import { MonoFooter, MonoHeader, MonoPage } from "ferramenta-family/monografie";
import "ferramenta-family/ferra.css";
import "ferramenta-family/monografie.css";

import { navigation, version } from "./data/site.json";
import "ferramenta-family/ferra-ardo.css";

import "./components/site-shell.css";

export function Layout({ children }: { children: ReactNode }) {
  return (
    <ArdoRootLayout
      colorScheme="light"
      className="ferra-tokens ferra-ardo"
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
      <FerraSectionRail aria-label="Documentation sections">
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
      </FerraSectionRail>
      <FerraGuideNavigation aria-label="Documentation pages">
        <NavLink to="/" onClick={onNavigate}>
          Overview
        </NavLink>
        <p>{current.label}</p>
        {current.pages.map(({ title, href }) => (
          <NavLink key={href} to={href} onClick={onNavigate}>
            {title}
          </NavLink>
        ))}
      </FerraGuideNavigation>
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
    <FerraReadingLayout
      id="ferroni-article"
      navigation={<GuideNavigation />}
      outline={<ArdoTOC source="dom" scope="#ferroni-article" />}
    >
      <header>
        <p>Ferroni / {current?.label ?? "Documentation"}</p>
        <h1>{page?.title ?? "Documentation"}</h1>
      </header>
      {children}
    </FerraReadingLayout>
  );
}
const components = { table: FerraReadingTable, wrapper: Article };

/**
 * Every page wears the Monografie header and footer. The homepage reads on the
 * Monografie page ground; documentation keeps the Ferra reading layout.
 */
export default function Root() {
  const path = useArdoPath();
  const home = path === "/" || path === "";
  const shell = (
    <>
      <MarkDefs />
      <ArdoRoot
        className="ferra-ardo-shell"
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
    <FerraPage className="ferra-site fr-docs">{shell}</FerraPage>
  );
}
