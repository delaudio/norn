import "@testing-library/jest-dom/vitest";
import { vi } from "vitest";

// Keep the real Tauri `invoke` out of tests. isTauri() is already false under
// jsdom (no __TAURI_INTERNALS__), so tauriCall routes to the mock layer; this
// mock just guarantees the module never touches a real webview bridge.
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

// jsdom doesn't implement these; react-virtuoso and Radix need them.
class ResizeObserverMock {
  observe() {}
  unobserve() {}
  disconnect() {}
}
vi.stubGlobal("ResizeObserver", ResizeObserverMock);

if (!window.matchMedia) {
  vi.stubGlobal("matchMedia", (query: string) => ({
    matches: false,
    media: query,
    onchange: null,
    addEventListener: () => {},
    removeEventListener: () => {},
    addListener: () => {},
    removeListener: () => {},
    dispatchEvent: () => false,
  }));
}

// jsdom lacks constructable stylesheet support that the Pierre diff renderer
// assumes (`CSSStyleSheet.replaceSync` and `adoptedStyleSheets`).
if (
  typeof CSSStyleSheet !== "undefined" &&
  typeof CSSStyleSheet.prototype.replaceSync !== "function"
) {
  Object.defineProperty(CSSStyleSheet.prototype, "replaceSync", {
    configurable: true,
    value: () => {},
  });
}
if (!("adoptedStyleSheets" in Document.prototype)) {
  Object.defineProperty(Document.prototype, "adoptedStyleSheets", {
    configurable: true,
    get: () => [],
  });
}
