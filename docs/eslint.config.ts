import { getEslintConfig } from "eslint-config-setup";

const config = await getEslintConfig({ node: true, react: true, oxlint: true });

config.unshift({
  ignores: [
    "**/dist/**",
    "coverage/**",
    "build/**",
    ".react-router/**",
    "app/routes.ts",
    "node_modules/**",
    "pnpm-lock.yaml",
    "**/*.json",
    "**/*.md",
  ],
});

export default config;
