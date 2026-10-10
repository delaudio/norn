import { render, screen, waitFor } from "@testing-library/react";
import { StrictMode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { BrowserDiffApp, type BrowserDiffState, browserDiffApiUrl } from "./BrowserDiffApp";

// The Pierre renderer relies on browser layout/async highlighting that jsdom
// does not provide; assert the app wires the patch and mode into it instead.
vi.mock("@pierre/diffs/react", () => ({
  PatchDiff: ({ patch, options }: { patch: string; options?: { diffStyle?: string } }) => (
    <pre data-testid="pierre-patch" data-diff-style={options?.diffStyle}>
      {patch}
    </pre>
  ),
}));

const rawDiff = `diff --git a/src/example.ts b/src/example.ts
index 1111111..2222222 100644
--- a/src/example.ts
+++ b/src/example.ts
@@ -1 +1 @@
-export const value = "before";
+export const value = "after";
`;
const sessionToken = "a".repeat(64);
const sessionRoot = `/session/${sessionToken}`;

function stateResponse(title: string, version: number, overrides: Partial<BrowserDiffState> = {}) {
  return new Response(
    JSON.stringify({
      version,
      targetKind: "pullRequest",
      workspace: "workspace",
      repo: "repository",
      prId: 42,
      prTitle: title,
      prAuthor: "Reviewer",
      sourceBranch: "feature/shared-viewer",
      targetBranch: "main",
      baseSha: "1".repeat(40),
      diff: rawDiff,
      diffstat: [
        {
          status: "modified",
          linesAdded: 1,
          linesRemoved: 1,
          oldPath: "src/example.ts",
          newPath: "src/example.ts",
        },
      ],
      populationFailed: false,
      ...overrides,
    }),
    { status: 200, headers: { "Content-Type": "application/json" } },
  );
}

describe("BrowserDiffApp", () => {
  beforeEach(() => {
    window.history.replaceState({}, "", `${sessionRoot}/`);
  });

  it("builds authenticated API URLs from the current session", () => {
    expect(browserDiffApiUrl(window.location.pathname, "/api/state", { version: 4 })).toBe(
      `${sessionRoot}/api/state?version=4`,
    );
  });

  it("rejects paths without the server's authenticated session-token shape", () => {
    expect(() => browserDiffApiUrl("/session/test-token/", "/api/state")).toThrow(
      "This browser diff session URL is invalid.",
    );
  });

  it("renders provider state through the Pierre diff renderer", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(stateResponse("Use the shared diff renderer", 1)),
    );

    render(<BrowserDiffApp />);

    expect(await screen.findByText("Use the shared diff renderer")).toBeInTheDocument();
    const patch = await screen.findByTestId("pierre-patch");
    expect(patch).toHaveTextContent("src/example.ts");
    expect(patch).toHaveTextContent('export const value = "after";');
    expect(patch).toHaveAttribute("data-diff-style", "split");
  });

  it("labels local review targets without presenting a pull request id", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(
        stateResponse("Local changes on feature/local", 1, {
          targetKind: "local",
          prId: 0,
          sourceBranch: "feature/local",
          targetBranch: "origin/feature/local",
        }),
      ),
    );

    render(<BrowserDiffApp />);

    expect(await screen.findByText("LOCAL")).toBeInTheDocument();
    expect(screen.queryByText("#0")).not.toBeInTheDocument();
  });

  it("ignores a stale polling response after StrictMode aborts the first effect", async () => {
    let resolveStaleResponse!: (response: Response) => void;
    const staleResponse = new Promise<Response>((resolve) => {
      resolveStaleResponse = resolve;
    });
    vi.stubGlobal(
      "fetch",
      vi
        .fn()
        .mockImplementationOnce(() => staleResponse)
        .mockResolvedValueOnce(stateResponse("Current pull request", 2)),
    );

    render(
      <StrictMode>
        <BrowserDiffApp />
      </StrictMode>,
    );
    expect(await screen.findByText("Current pull request")).toBeInTheDocument();

    resolveStaleResponse(stateResponse("Stale pull request", 1));

    await waitFor(() => expect(screen.queryByText("Stale pull request")).not.toBeInTheDocument());
    expect(screen.getByText("Current pull request")).toBeInTheDocument();
  });

  it("uses the desktop renderer's preview-side contract for modified images", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(
        stateResponse("Modified image", 1, {
          diff: "",
          diffstat: [
            {
              status: "modified",
              linesAdded: 0,
              linesRemoved: 0,
              oldPath: "images/base.png",
              newPath: "images/changed.png",
            },
          ],
        }),
      ),
    );

    render(<BrowserDiffApp />);

    expect(await screen.findByRole("img", { name: "images/changed.png" })).toHaveAttribute(
      "src",
      `${sessionRoot}/api/file-preview?path=images%2Fchanged.png&side=new`,
    );
    expect(screen.getByText("new image")).toBeInTheDocument();
  });
});
