import { withArdoGitHubPages } from "ardo/vite";

// The site is served from the root of its own domain (ferroni.dev), not from
// the repository path GitHub Pages would otherwise detect.
export default withArdoGitHubPages(
  {
    ssr: false,
    prerender: true,
  },
  { basename: "/" },
);
