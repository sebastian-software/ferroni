import { RootLayout, ArdoRoot, Footer } from "ardo/ui"
import { FamilyLinks } from "@ferramenta/ardo-config"
import config from "virtual:ardo/config"
import sidebar from "virtual:ardo/sidebar"
import type { MetaFunction } from "react-router"
import "ardo/ui/styles.css"
import "@ferramenta/ardo-config/theme.css"

export const meta: MetaFunction = () => [{ title: config.title }]

export function Layout({ children }: { children: React.ReactNode }) {
  return <RootLayout>{children}</RootLayout>
}

function FerroniFooter() {
  const version = config.project?.version
  const buildDate = config.buildTime
    ? new Date(config.buildTime).toLocaleDateString("en-US", {
        month: "long",
        day: "numeric",
        year: "numeric",
      })
    : undefined

  return (
    <Footer>
      <p>
        ferroni{version ? ` v${version}` : ""} ·{" "}
        <a href="https://ardo-docs.dev">Built with Ardo</a>
      </p>
      <FamilyLinks current="ferroni" />
      <p>Released under the BSD-2-Clause License.</p>
      {buildDate ? (
        <p>
          Built on {buildDate}
          {config.buildHash ? ` (${config.buildHash})` : ""}
        </p>
      ) : null}
    </Footer>
  )
}

export default function Root() {
  return <ArdoRoot config={config} sidebar={sidebar} footer={<FerroniFooter />} />
}
