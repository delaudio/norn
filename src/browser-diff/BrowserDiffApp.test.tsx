import type { FileDiffMetadata } from "@pierre/diffs";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { StrictMode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { BrowserDiffApp, type BrowserDiffState, browserDiffApiUrl } from "./BrowserDiffApp";

// The Pierre renderer relies on browser layout/async highlighting that jsdom
// does not provide. Only mock rendering: use the real Pierre patch parser to
// verify the single-file metadata boundary, including complete multi-file PRs.
vi.mock("@pierre/diffs/react", () => ({
  FileDiff: ({
    fileDiff,
    options,
  }: {
    fileDiff: FileDiffMetadata;
    options?: { diffStyle?: string };
  }) => (
    <pre
      data-testid="pierre-patch"
      data-file-name={fileDiff.name}
      data-cache-key={fileDiff.cacheKey}
      data-diff-style={options?.diffStyle}
    >
      {[fileDiff.name, ...fileDiff.deletionLines, ...fileDiff.additionLines].join("\n")}
    </pre>
  ),
}));

import { browserFileAnchor } from "./fileAnchor";

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
    const target = document.getElementById(browserFileAnchor("images/changed.png"))!;
    target.scrollIntoView = vi.fn();
    fireEvent.click(screen.getByRole("button", { name: /changed.png/ }));
    expect(target.scrollIntoView).toHaveBeenCalled();
    expect(target.querySelector("img")).not.toBeNull();
  });
  it("renders every file in a 75-file provider patch through Pierre's real parser", async () => {
    const patch = Array.from({ length: 75 }, (_, i) =>
      rawDiff.split("src/example.ts").join(`src/file-${i}.ts`),
    ).join("");
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(stateResponse("Large PR", 1, { diff: patch })),
    );
    render(<BrowserDiffApp />);
    await screen.findByText("Large PR");
    const files = await screen.findAllByTestId("pierre-patch");
    expect(files).toHaveLength(75);
    expect(files.map((file) => file.dataset.fileName)).toEqual(
      Array.from({ length: 75 }, (_, i) => `src/file-${i}.ts`),
    );
    for (const file of files) expect(file).toHaveAttribute("data-diff-style", "split");
  });

  it("keeps the original patch visible when the formatted parser cannot read it", async () => {
    vi.stubGlobal(
      "fetch",
      vi
        .fn()
        .mockResolvedValue(
          stateResponse("Unrecognized patch", 1, { diff: "unsupported patch body" }),
        ),
    );
    render(<BrowserDiffApp />);
    expect(await screen.findByRole("alert")).toHaveTextContent("original patch");
    expect(screen.getByText("unsupported patch body")).toBeInTheDocument();
    expect(screen.queryByTestId("pierre-patch")).not.toBeInTheDocument();
  });
  it("keeps repeated file paths from distinct local layers separate", async () => {
    vi.stubGlobal(
      "fetch",
      vi
        .fn()
        .mockResolvedValue(
          stateResponse("Layered local diff", 1, { targetKind: "local", diff: rawDiff + rawDiff }),
        ),
    );
    render(<BrowserDiffApp />);
    const files = await screen.findAllByTestId("pierre-patch");
    expect(files).toHaveLength(2);
    expect(files[0]).toHaveAttribute("data-file-name", "src/example.ts");
    expect(files[1]).toHaveAttribute("data-file-name", "src/example.ts");
    expect(files[0].dataset.cacheKey).not.toBe(files[1].dataset.cacheKey);
    expect(
      Array.from(document.querySelectorAll("article[id]")).filter(
        (node) => node.id === browserFileAnchor("src/example.ts"),
      ),
    ).toHaveLength(1);
    expect(screen.getAllByRole("button", { name: /example.ts/ })).toHaveLength(1);
  });
  it("shows a searchable file tree whose clicks scroll to the corresponding diff", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(
        stateResponse("Tree navigation", 1, {
          diff: rawDiff + rawDiff.split("src/example.ts").join("docs/readme.md"),
        }),
      ),
    );
    render(<BrowserDiffApp />);
    await screen.findByText("Tree navigation");
    expect(screen.getByRole("navigation", { name: "Changed files" })).toBeInTheDocument();
    const target = document.getElementById(browserFileAnchor("docs/readme.md"))!;
    target.scrollIntoView = vi.fn();
    const button = screen.getByRole("button", { name: /readme.md/ });
    fireEvent.click(button);
    expect(target.scrollIntoView).toHaveBeenCalledWith({ behavior: "smooth", block: "start" });
    expect(button).toHaveAttribute("aria-current", "true");
    fireEvent.change(screen.getByRole("searchbox"), { target: { value: "readme" } });
    expect(screen.queryByRole("button", { name: /example.ts/ })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: /readme.md/ })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Files" }));
    expect(screen.queryByRole("navigation", { name: "Changed files" })).not.toBeInTheDocument();
  });
});
