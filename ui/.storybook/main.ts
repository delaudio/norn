import type { StorybookConfig } from "@storybook/react-vite";

const config: StorybookConfig = {
  stories: ["../storybook/stories/*.stories.tsx"],
  framework: { name: "@storybook/react-vite", options: {} },
  core: { disableTelemetry: true },
  async viteFinal(config) {
    return {
      ...config,
      esbuild: { ...config.esbuild, jsx: "automatic", jsxImportSource: "react" },
      resolve: { ...config.resolve, dedupe: ["react", "react-dom"] },
    };
  },
};

export default config;
