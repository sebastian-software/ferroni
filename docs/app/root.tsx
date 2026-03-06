import { RootLayout, ArdoRoot } from "ardo/ui"
import config from "virtual:ardo/config"
import sidebar from "virtual:ardo/sidebar"
import type { MetaFunction } from "react-router"
import "ardo/ui/styles.css"

export const meta: MetaFunction = () => [{ title: config.title }]

export function Layout({ children }: { children: React.ReactNode }) {
  return <RootLayout>{children}</RootLayout>
}

export default function Root() {
  return <ArdoRoot config={config} sidebar={sidebar} />
}
