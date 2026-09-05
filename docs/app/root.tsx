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
} from "ardo/ui"
import { FamilyLinks } from "@ferramenta/ardo-config"
import config from "virtual:ardo/config"
import type { MetaFunction } from "react-router"
import "ardo/ui/styles.css"
import "@ferramenta/ardo-config/theme.css"

export const meta: MetaFunction = () => [{ title: config.title }]

export function Layout({ children }: { children: React.ReactNode }) {
  return <ArdoRootLayout>{children}</ArdoRootLayout>
}

export default function Root() {
  return (
    <ArdoRoot config={config}>
      <ArdoHeader>
        <ArdoNav>
          <ArdoNavLink to="/guide/getting-started">Guide</ArdoNavLink>
          <ArdoNavLink to="/perf/benchmark-results">Performance</ArdoNavLink>
          <ArdoNavLink to="/adr/001-one-to-one-parity-with-c-original">
            ADRs
          </ArdoNavLink>
        </ArdoNav>
      </ArdoHeader>

      <ArdoSidebar>
        <ArdoSidebarSection
          id="guide"
          label="Guide"
          to="/guide/getting-started"
        >
          <ArdoGeneratedSidebar section="guide" />
        </ArdoSidebarSection>
        <ArdoSidebarSection
          id="perf"
          label="Performance"
          to="/perf/benchmark-results"
        >
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

      <ArdoFooter>
        <p>
          ferroni{config.project?.version ? ` v${config.project.version}` : ""} ·{" "}
          <a href="https://ardo-docs.dev">Built with Ardo</a>
        </p>
        <FamilyLinks current="ferroni" />
        <p>Released under the BSD-2-Clause License.</p>
        {config.buildTime ? (
          <p>
            Built on{" "}
            {new Date(config.buildTime).toLocaleDateString("en-US", {
              month: "long",
              day: "numeric",
              year: "numeric",
              timeZone: "UTC",
            })}
            {config.buildHash ? ` (${config.buildHash})` : ""}
          </p>
        ) : null}
      </ArdoFooter>
    </ArdoRoot>
  )
}
