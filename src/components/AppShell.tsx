import type * as React from "react";

import { cn } from "@/lib/utils";
import type { ReviewProvider } from "@/types";

export interface AppShellProps {
  /** When omitted the main content fills the full width (overview mode). */
  sidebar?: React.ReactNode;
  main?: React.ReactNode;
  headerRight?: React.ReactNode;
  reviewProvider?: ReviewProvider;
  onReviewProviderChange?: (provider: ReviewProvider) => void;
  footer?: React.ReactNode;
  /** Sidebar width in px. */
  sidebarWidth?: number;
  /** Optional right panel shown beside the main content (e.g. AI review). */
  rightPanel?: React.ReactNode;
  /** Right panel width in px. */
  rightPanelWidth?: number;
  /**
   * When true the right panel overlays the entire body area (absolute inset-0),
   * matching the Zed "zoom panel" pattern.
   */
  rightPanelExpanded?: boolean;
}

export function AppShell({
  sidebar,
  main,
  headerRight,
  reviewProvider = "bitbucket",
  onReviewProviderChange,
  footer,
  sidebarWidth = 340,
  rightPanel,
  rightPanelWidth = 420,
  rightPanelExpanded = false,
}: AppShellProps) {
  const hasSidebar = sidebar != null;
  const hasMain = main != null;
  const hasRight = rightPanel != null;
  const showRightInGrid = hasRight && !rightPanelExpanded;
  const sidebarColumn = `min(${sidebarWidth}px, 38vw)`;

  const columns = (() => {
    if (hasSidebar && hasMain && showRightInGrid) {
      return `${sidebarColumn} minmax(0, 1fr) ${rightPanelWidth}px`;
    }
    if (hasSidebar && hasMain) {
      return `${sidebarColumn} minmax(0, 1fr)`;
    }
    if (hasMain && showRightInGrid) {
      return `minmax(0, 1fr) ${rightPanelWidth}px`;
    }
    if (hasSidebar && showRightInGrid) {
      return `${sidebarColumn} minmax(0, 1fr)`;
    }
    return "minmax(0, 1fr)";
  })();
  const providerLabel = reviewProvider === "github" ? "GitHub" : "Bitbucket";

  const panes: React.ReactNode[] = [];
  if (hasSidebar) {
    panes.push(
      <div
        key="sidebar"
        className={cn(
          "min-h-0 overflow-hidden",
          (hasMain || showRightInGrid) && "border-r border-border",
        )}
      >
        {sidebar}
      </div>,
    );
  }
  if (hasMain) {
    panes.push(
      <main
        key="main"
        className={cn("min-h-0 overflow-hidden", showRightInGrid && "border-r border-border")}
      >
        {main}
      </main>,
    );
  }
  if (showRightInGrid) {
    panes.push(
      <div key="right" className="min-h-0 overflow-hidden bg-background">
        {rightPanel}
      </div>,
    );
  }

  return (
    <div className="flex h-full flex-col bg-background text-foreground">
      <header className="flex h-11 shrink-0 items-center justify-between border-b border-border bg-secondary px-3">
        <div className="flex items-center gap-2">
          <span className="text-sm font-semibold tracking-tight">Norn</span>
          <label className="sr-only" htmlFor="review-provider-select">
            Review provider
          </label>
          <select
            id="review-provider-select"
            className="h-7 rounded border border-transparent bg-transparent px-1 text-xs text-muted-foreground hover:border-border hover:bg-muted focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
            value={reviewProvider}
            title={`${providerLabel} review`}
            onChange={(event) => onReviewProviderChange?.(event.target.value as ReviewProvider)}
          >
            <option value="bitbucket">Bitbucket review</option>
            <option value="github">GitHub review</option>
          </select>
        </div>
        <div className="flex items-center gap-1">{headerRight}</div>
      </header>
      {panes.length > 0 ? (
        <div className="relative grid min-h-0 flex-1" style={{ gridTemplateColumns: columns }}>
          {panes}
          {hasRight && rightPanelExpanded && (
            <div
              className={cn(
                "min-h-0 overflow-hidden bg-background",
                rightPanelExpanded && "absolute inset-0 z-30 border-l-0 shadow-xl",
              )}
            >
              {rightPanel}
            </div>
          )}
        </div>
      ) : (
        <main className="min-h-0 flex-1 overflow-hidden">{main}</main>
      )}
      {footer}
    </div>
  );
}
