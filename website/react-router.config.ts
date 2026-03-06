import type { Config } from "@react-router/dev/config"
import { detectGitHubBasename } from "ardo/vite"

export default {
  ssr: false,
  prerender: true,
  basename: detectGitHubBasename(),
} satisfies Config
