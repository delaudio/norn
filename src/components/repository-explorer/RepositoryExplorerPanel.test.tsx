import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { RepositoryExplorerPanel } from "./RepositoryExplorerPanel";

describe("RepositoryExplorerPanel", () => {
  it("loads repository files, filters the tree, and opens a file", async () => {
    const user = userEvent.setup();
    const onSelectFile = vi.fn();

    render(
      <RepositoryExplorerPanel
        workspace="example-workspace"
        repo="frontend-app"
        onSelectFile={onSelectFile}
      />,
    );

    const fileTree = screen.getByLabelText("Repository files");
    await waitFor(() => {
      expect(within(fileTree).getByRole("button", { name: /App\.tsx/ })).toBeInTheDocument();
    });
    expect(screen.getByText("9 files")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "4 changed" })).toBeInTheDocument();
    expect(within(fileTree).getByTitle("Modified")).toBeInTheDocument();
    await waitFor(() => {
      expect(screen.getByText(/showLocalDraft/)).toBeInTheDocument();
    });

    await user.click(screen.getByRole("button", { name: "4 changed" }));

    expect(within(fileTree).getByRole("button", { name: /localDraft\.ts/ })).toBeInTheDocument();
    expect(
      within(fileTree).queryByRole("button", { name: /RecordTable\.tsx/ }),
    ).not.toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "4 changed" }));

    await user.type(screen.getByRole("searchbox", { name: "Search repository files" }), "format");

    expect(screen.getByRole("button", { name: /format\.ts/ })).toBeInTheDocument();
    expect(within(fileTree).queryByRole("button", { name: /App\.tsx/ })).not.toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: /format\.ts/ }));

    expect(onSelectFile).toHaveBeenCalledWith("src/lib/format.ts", null);
    await waitFor(() => {
      expect(screen.getByText(/formatCurrency/)).toBeInTheDocument();
    });

    await user.click(screen.getByRole("button", { name: "Open file in external editor" }));
    expect(screen.queryByText(/Mock file not found/)).not.toBeInTheDocument();
  });

  it("collapses and expands folders from the tree", async () => {
    const user = userEvent.setup();
    render(<RepositoryExplorerPanel workspace="example-workspace" repo="frontend-app" />);

    const fileTree = screen.getByLabelText("Repository files");
    await waitFor(() => {
      expect(within(fileTree).getByRole("button", { name: /App\.tsx/ })).toBeInTheDocument();
    });

    await user.click(within(fileTree).getByRole("button", { name: /^src$/ }));

    expect(within(fileTree).queryByRole("button", { name: /App\.tsx/ })).not.toBeInTheDocument();

    await user.click(within(fileTree).getByRole("button", { name: /^src$/ }));

    expect(within(fileTree).getByRole("button", { name: /App\.tsx/ })).toBeInTheDocument();
  });

  it("toggles all visible folders between collapsed and expanded", async () => {
    const user = userEvent.setup();
    render(<RepositoryExplorerPanel workspace="example-workspace" repo="frontend-app" />);

    const fileTree = screen.getByLabelText("Repository files");
    await waitFor(() => {
      expect(within(fileTree).getByRole("button", { name: /App\.tsx/ })).toBeInTheDocument();
    });

    await user.click(screen.getByRole("button", { name: "Collapse all" }));

    expect(within(fileTree).queryByRole("button", { name: /App\.tsx/ })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Expand all" })).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Expand all" }));

    expect(within(fileTree).getByRole("button", { name: /App\.tsx/ })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Collapse all" })).toBeInTheDocument();
  });

  it("loads blame details when a file line is selected", async () => {
    const user = userEvent.setup();
    const onSelectFile = vi.fn();
    render(
      <RepositoryExplorerPanel
        workspace="example-workspace"
        repo="frontend-app"
        onSelectFile={onSelectFile}
      />,
    );

    await user.click(await screen.findByRole("button", { name: "File" }));
    const lineButton = await screen.findByRole("button", { name: "Select line 4" });
    await user.click(lineButton);

    expect(onSelectFile).toHaveBeenCalledWith("src/App.tsx", 4);
    await waitFor(() => {
      expect(screen.getByText("Grace Hopper")).toBeInTheDocument();
    });
    expect(screen.getByText("6f52c9a1")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "6f52c9a1" })).toHaveAttribute(
      "href",
      "https://bitbucket.org/example-workspace/frontend-app/commits/6f52c9a1cf5cd075762f13d0b0f8bf8d0f4f3f7d",
    );
    expect(screen.getByText(/Update fixture file/)).toBeInTheDocument();
    expect(screen.getByText(/Refresh the mock repository content/)).toBeInTheDocument();

    await user.click(lineButton);

    expect(onSelectFile).toHaveBeenLastCalledWith("src/App.tsx", null);
    expect(screen.queryByText("Grace Hopper")).not.toBeInTheDocument();
  });

  it("opens file search with Cmd+F and navigates matches", async () => {
    const user = userEvent.setup();
    const { container } = render(
      <RepositoryExplorerPanel workspace="example-workspace" repo="frontend-app" />,
    );

    await screen.findByText(/showLocalDraft/);
    await user.click(await screen.findByRole("button", { name: "File" }));
    await screen.findByRole("button", { name: "Select line 4" });

    fireEvent.keyDown(window, { key: "f", metaKey: true });
    const findInput = await screen.findByRole("searchbox", { name: "Find in current file" });

    await waitFor(() => {
      expect(findInput).toHaveFocus();
    });

    await user.type(findInput, "RecordTable");

    await waitFor(() => {
      expect(screen.getByText("1/3")).toBeInTheDocument();
      expect(container.querySelectorAll(".repo-find-match")).toHaveLength(3);
    });
    expect(container.querySelectorAll(".repo-find-match--active")).toHaveLength(1);

    await user.click(screen.getByRole("button", { name: "Next match" }));

    expect(screen.getByText("2/3")).toBeInTheDocument();

    fireEvent.keyDown(window, { key: "Escape" });

    expect(
      screen.queryByRole("searchbox", { name: "Find in current file" }),
    ).not.toBeInTheDocument();
    expect(container.querySelectorAll(".repo-find-match")).toHaveLength(0);
  });

  it("renders Markdown files with a source and preview toggle", async () => {
    const user = userEvent.setup();
    render(<RepositoryExplorerPanel workspace="example-workspace" repo="frontend-app" />);

    await screen.findByText(/showLocalDraft/);
    expect(screen.queryByRole("button", { name: "Preview" })).not.toBeInTheDocument();

    await user.click(await screen.findByRole("button", { name: /README\.md/ }));

    expect(await screen.findByRole("button", { name: "Source" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Preview" })).toBeInTheDocument();
    expect(
      await screen.findByText("Mock repository fixture for Norn browser development."),
    ).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Preview" }));

    expect(screen.getByRole("heading", { name: "Frontend app", level: 1 })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Usage", level: 2 })).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "local app" })).toHaveAttribute(
      "href",
      "https://example.com",
    );
    expect(screen.getByText("pnpm dev")).toBeInTheDocument();
    expect(screen.getByText("export const preview = true;")).toBeInTheDocument();
    expect(screen.getByRole("navigation", { name: "Markdown headings" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Frontend app" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Usage" })).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Collapse headings map" }));

    expect(screen.queryByRole("navigation", { name: "Markdown headings" })).not.toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Expand headings map" }));

    expect(screen.getByRole("navigation", { name: "Markdown headings" })).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Source" }));

    expect(
      screen.getByText("Mock repository fixture for Norn browser development."),
    ).toBeInTheDocument();
  });
});
