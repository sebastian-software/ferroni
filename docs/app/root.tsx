import type { ReactNode } from "ardo/react";

import { ArdoActiveLink as NavLink } from "ardo/runtime";
import {
  MonoArdoErrorBoundary,
  MonoArdoLayout,
  MonoArdoSite,
  type MonoDocsSection,
} from "ferramenta-family/ardo";
import touchIcon from "ferramenta-family/ferra-assets/ferroni/apple-touch-icon.png?url";
import favicon from "ferramenta-family/ferra-assets/ferroni/favicon.ico?url";

import { navigation, version } from "./data/site.json";
import "ferramenta-family/monografie.css";
import "ferramenta-family/monografie-docs.css";
import "ferramenta-family/monografie-ardo.css";

export function Layout({ children }: { children: ReactNode }) {
  return (
    <MonoArdoLayout icon={favicon} appleTouchIcon={touchIcon}>
      {children}
    </MonoArdoLayout>
  );
}
export const ErrorBoundary = MonoArdoErrorBoundary;

const sections: MonoDocsSection[] = navigation.map((section) => ({
  id: section.id,
  label: section.label,
  to: section.href,
  icon: section.icon,
  pages: section.pages.map(({ title, href }) => ({ label: title, to: href })),
}));

const links = [
  <NavLink key="guide" to="/guide/getting-started">
    Guide
  </NavLink>,
  <NavLink key="scanner" to="/guide/scanner">
    Scanner
  </NavLink>,
  <NavLink key="compatibility" to="/guide/compatibility">
    Compatibility
  </NavLink>,
  <a key="api" href="https://docs.rs/ferroni">
    API
  </a>,
];

/** Every page on Monografie's Ardo shell; the documentation adds its menu, search and reading layout. */
export default function Root() {
  return (
    <MonoArdoSite
      product="ferroni"
      name="Ferroni"
      release={version}
      links={links}
      repository="https://github.com/sebastian-software/ferroni"
      sections={sections}
      homeClassName="fr-home"
      docsClassName="fr-docs"
      footer={
        <>
          <a href="https://github.com/sebastian-software/ferroni">GitHub</a>
          <a href="https://crates.io/crates/ferroni">crates.io</a>
          <a href="https://docs.rs/ferroni">docs.rs</a>
        </>
      }
    />
  );
}
