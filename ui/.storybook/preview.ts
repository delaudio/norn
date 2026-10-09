import type { Preview } from "@storybook/react-vite";
import "@fontsource/jetbrains-mono/400.css";
import "@fontsource/jetbrains-mono/700.css";
import "../storybook/preview.css";

// Frame viewers only: stories render captured native frames, not a live OpenTUI
// runtime. Layout/color regressions are enforced by `bun run frames:check`.
const preview: Preview = {
  parameters: {
    layout: "centered",
    controls: { disable: true },
    backgrounds: { default: "dark" },
  },
  async afterEach({ canvasElement }) {
    await document.fonts.ready;
    if (!canvasElement.querySelector("[data-visual-snapshot-root]")) {
      throw new Error("Story did not render a native terminal frame");
    }
  },
};

export default preview;
