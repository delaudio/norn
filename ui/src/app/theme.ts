// Norn terminal palette (dark), aligned with the desktop app tokens.

const dark = {
  background: "#0a0a0a",
  panel: "#141414",
  element: "#1e1e1e",
  border: "#3c3c3c",
  text: "#eeeeee",
  muted: "#808080",
  primary: "#7aa2f7",
  secondary: "#9ece6a",
  warning: "#e0af68",
  error: "#f7768e",
} as const;

const light: Record<keyof typeof dark, string> = {
  background: "#fafafa",
  panel: "#eeeeee",
  element: "#e0e0e0",
  border: "#b0b0b0",
  text: "#202020",
  muted: "#606060",
  primary: "#3054a5",
  secondary: "#386b27",
  warning: "#8d5b13",
  error: "#aa2338",
};
export let theme: Record<keyof typeof dark, string> = dark;
export function setTheme(mode: "light" | "dark"): void {
  theme = mode === "light" ? light : dark;
}
