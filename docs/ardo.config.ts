import { defineArdoConfig } from "ardo/config";

export default defineArdoConfig({
  title: "Ferroni",
  description: "Oniguruma-compatible regular expressions and a TextMate scanner, in native Rust.",
  siteUrl: "https://ferroni.dev",
  icons: false,
  markdown: { headingAttributes: true },
  linkCheck: { level: "error" },
  redirects: [
    {
      from: "/adr/readme-theme-composition",
      to: "https://github.com/sebastian-software/ferroni/blob/main/docs/readme-theme.md",
    },
  ],
});
