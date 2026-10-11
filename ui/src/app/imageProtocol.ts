import {
  type ImageRenderProtocol,
  resolveImageRenderProtocol,
  type TerminalCapabilities,
} from "@opentui/core";

export function imageProtocol(
  capabilities: TerminalCapabilities | null,
  hasResolution: boolean,
): Exclude<ImageRenderProtocol, "auto"> {
  if (capabilities?.multiplexer === "tmux") return "blocks";
  // Physical iTerm2 3.6.11 / OpenTUI 0.5.17 smoke received a positive Kitty
  // query but displayed blank image placements. The same fixture renders with
  // Sixel. Prefer that observed working path when available; preserve explicit
  // user protocol overrides and the normal SDK selection on other terminals.
  if (
    (capabilities?.image_protocol ?? "auto") === "auto" &&
    capabilities?.terminal.name.toLowerCase().includes("iterm")
  )
    return capabilities.sixel && hasResolution ? "sixel" : "blocks";
  return resolveImageRenderProtocol("auto", capabilities, hasResolution);
}
