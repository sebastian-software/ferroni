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
  FerraFooter,
  FerraGuideNavigation,
  FerraHeader,
  FerraPage,
  FerraReadingLayout,
  FerraReadingTable,
  FerraSectionRail,
} from "ferramenta-family/ferra";
import touchIcon from "ferramenta-family/ferra-assets/ferroni/apple-touch-icon.png?url";
import favicon from "ferramenta-family/ferra-assets/ferroni/favicon.ico?url";
import { WORKSHOP } from "ferramenta-family/registry";
import "ferramenta-family/ferra.css";

import { navigation, version } from "./data/site.json";
import "ferramenta-family/ferra-ardo.css";

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

function Header() {
  const [open, setOpen] = useState(false);
  return (
    <FerraHeader
      product="ferroni"
      home="/"
      nav={
        <>
          <NavLink to="/guide/getting-started">Guide</NavLink>
          <NavLink to="/perf/regex-workloads">Performance</NavLink>
          <NavLink to="/adr/001-one-to-one-parity-with-c-original">Architecture</NavLink>
          <a href="https://ferramenta.dev">Ferramenta</a>
        </>
      }
      actions={
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
      }
    />
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

export default function Root() {
  return (
    <FerraPage className="ferra-site">
      <MarkDefs />
      <ArdoRoot
        className="ferra-ardo-shell"
        scrollMode="document"
        header={<Header />}
        sidebar={false}
        footer={false}
        components={components}
      />
      <FerraFooter product="ferroni" home="/">
        <p>
          Ferroni v{version} · BSD-2-Clause License ·{" "}
          <a href="https://ardo-docs.dev">Built with Ardo</a> ·{" "}
          <a href={WORKSHOP.openSource}>{WORKSHOP.company}</a> ·{" "}
          <a href="https://ferramenta.dev">Ferramenta family</a>
        </p>
      </FerraFooter>
    </FerraPage>
  );
}
