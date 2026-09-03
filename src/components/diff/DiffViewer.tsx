import "react-diff-view/style/index.css";
import "./diff-theme.css";
import { CaretDown } from "@phosphor-icons/react";
import { type ReactNode, useEffect, useMemo, useState } from "react";
import type { ChangeEventArgs } from "react-diff-view";
import { fileAnchorId, fileKey } from "@/lib/diff";
import { countReviewFileChanges, type ReviewFileData } from "@/lib/imageDiff";
import type { DiffViewMode } from "@/types";
import { DiffViewToggle } from "./DiffViewToggle";
import { FileDiff } from "./FileDiff";
import { FileTree, type FileTreeFolderCommand } from "./FileTree";

type RenderableDiffViewMode = Exclude<DiffViewMode, "conversation">;

function startsCollapsed(file: ReviewFileData): boolean {
  const { additions, deletions } = countReviewFileChanges(file);
  return additions + deletions > 500;
}

function initialCollapsedFileKeys(files: ReviewFileData[]): Set<string> {
  return new Set(files.filter(startsCollapsed).map(fileKey));
}

export interface DiffViewerProps {
  files: ReviewFileData[];
  viewMode: RenderableDiffViewMode;
  onViewModeChange: (mode: DiffViewMode) => void;
  loading?: boolean;
  error?: string | null;
  emptyMessage?: string;
  /** Per-file changeKey → widget map (comments / composer), keyed by fileKey. */
  widgetsByFile?: Record<string, Record<string, ReactNode>>;
  /** Per-file file-level comment block, keyed by fileKey. */
  fileWidgets?: Record<string, ReactNode>;
  viewedFileKeys?: Set<string>;
  onToggleFileViewed?: (file: ReviewFileData) => void;
  /** Called when a line gutter is clicked, to open a comment composer. */
  onGutterClick?: (file: ReviewFileData, args: ChangeEventArgs) => void;
  /** Called when the AI gutter action is clicked for a diff line. */
  onAskLine?: (file: ReviewFileData, args: ChangeEventArgs) => void;
}

export function DiffViewer({
  files,
  viewMode,
  onViewModeChange,
  loading,
  error,
  emptyMessage = "No changes in this pull request.",
  widgetsByFile,
  fileWidgets,
  viewedFileKeys,
  onToggleFileViewed,
  onGutterClick,
  onAskLine,
}: DiffViewerProps) {
  const [showFiles, setShowFiles] = useState(true);
  const [activeFileKey, setActiveFileKey] = useState<string | null>(() =>
    files[0] ? fileKey(files[0]) : null,
  );
  const [collapsedFileKeys, setCollapsedFileKeys] = useState<Set<string>>(() =>
    initialCollapsedFileKeys(files),
  );
  const [foldersCollapsed, setFoldersCollapsed] = useState(false);
  const [folderCommand, setFolderCommand] = useState<FileTreeFolderCommand | undefined>();
  const fileKeysByAnchorId = useMemo(
    () => new Map(files.map((file) => [fileAnchorId(file), fileKey(file)])),
    [files],
  );

  useEffect(() => {
    setActiveFileKey(files[0] ? fileKey(files[0]) : null);
  }, [files]);

  useEffect(() => {
    setCollapsedFileKeys((previous) => {
      const currentFileKeys = new Set(files.map(fileKey));
      const next = new Set([...previous].filter((key) => currentFileKeys.has(key)));
      for (const file of files) {
        const key = fileKey(file);
        if (startsCollapsed(file) && !previous.has(key)) next.add(key);
      }
      return next;
    });
  }, [files]);

  useEffect(() => {
    if (typeof IntersectionObserver === "undefined") return;

    const observer = new IntersectionObserver(
      (entries) => {
        const closestVisibleEntry = entries
          .filter((entry) => entry.isIntersecting)
          .sort(
            (a, b) => Math.abs(a.boundingClientRect.top) - Math.abs(b.boundingClientRect.top),
          )[0];
        if (!closestVisibleEntry) return;
        const key = fileKeysByAnchorId.get(closestVisibleEntry.target.id);
        if (key) setActiveFileKey(key);
      },
      { rootMargin: "-80px 0px -70% 0px", threshold: 0 },
    );

    for (const anchorId of fileKeysByAnchorId.keys()) {
      const element = document.getElementById(anchorId);
      if (element) observer.observe(element);
    }

    return () => observer.disconnect();
  }, [fileKeysByAnchorId]);

  const scrollToFile = (file: ReviewFileData) => {
    setActiveFileKey(fileKey(file));
    document
      .getElementById(fileAnchorId(file))
      ?.scrollIntoView({ behavior: "smooth", block: "start" });
  };

  const handleToggleFileCollapsed = (file: ReviewFileData) => {
    const key = fileKey(file);
    setCollapsedFileKeys((previous) => {
      const next = new Set(previous);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });
  };

  const handleToggleFileViewed = (file: ReviewFileData) => {
    const key = fileKey(file);
    const currentlyViewed = viewedFileKeys?.has(key) ?? false;
    onToggleFileViewed?.(file);
    setCollapsedFileKeys((previous) => {
      const next = new Set(previous);
      if (currentlyViewed) next.delete(key);
      else next.add(key);
      return next;
    });
  };

  const handleToggleAllFolders = () => {
    setFoldersCollapsed((currentlyCollapsed) => {
      const nextCollapsed = !currentlyCollapsed;
      setFolderCommand((previous) => ({
        id: (previous?.id ?? 0) + 1,
        mode: nextCollapsed ? "collapse" : "expand",
      }));
      return nextCollapsed;
    });
  };

  if (loading) {
    return <div className="p-6 text-sm text-muted-foreground">Loading diff…</div>;
  }
  if (error) {
    return <div className="p-6 text-sm text-destructive">{error}</div>;
  }
  if (files.length === 0) {
    return <div className="p-6 text-sm text-muted-foreground">{emptyMessage}</div>;
  }

  const totals = files.reduce(
    (acc, file) => {
      const { additions, deletions } = countReviewFileChanges(file);
      acc.additions += additions;
      acc.deletions += deletions;
      return acc;
    },
    { additions: 0, deletions: 0 },
  );
  const viewedCount = files.filter((file) => viewedFileKeys?.has(fileKey(file))).length;

  return (
    <div>
      <div className="sticky top-0 z-20 border-b border-border bg-background">
        <div className="flex items-center gap-3 border-t border-border px-4 py-2 text-xs">
          <button
            type="button"
            onClick={() => setShowFiles((s) => !s)}
            className="flex items-center gap-1 font-medium hover:text-primary"
            aria-expanded={showFiles}
            title="Show changed files"
          >
            <CaretDown
              size={12}
              className={showFiles ? "" : "-rotate-90"}
              style={{ transition: "transform 0.15s" }}
            />
            {files.length} file{files.length === 1 ? "" : "s"} changed
          </button>
          <span className="text-[var(--success)]">+{totals.additions}</span>
          <span className="text-destructive">-{totals.deletions}</span>
          {viewedFileKeys && (
            <span className="ml-2 text-muted-foreground">
              {viewedCount} / {files.length} viewed
            </span>
          )}
          <div className="ml-auto">
            <DiffViewToggle value={viewMode} onChange={onViewModeChange} />
          </div>
        </div>
      </div>
      <div className="flex min-w-0 items-start">
        {showFiles && (
          <aside className="sticky top-9 z-10 h-[calc(100vh-2.25rem)] w-80 shrink-0 border-r border-border bg-secondary">
            <div className="flex h-8 items-center border-b border-border px-3 text-xs font-semibold">
              <span className="min-w-0 flex-1 truncate">Changed files</span>
              <button
                type="button"
                onClick={handleToggleAllFolders}
                className="inline-flex size-6 shrink-0 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-muted hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:pointer-events-none disabled:opacity-50 [&_svg]:shrink-0"
                title={foldersCollapsed ? "Expand all" : "Collapse all"}
                aria-label={foldersCollapsed ? "Expand all folders" : "Collapse all folders"}
              >
                <svg
                  xmlns="http://www.w3.org/2000/svg"
                  width="16"
                  height="16"
                  fill="currentColor"
                  viewBox="0 0 256 256"
                  aria-hidden="true"
                  className={foldersCollapsed ? "rotate-180" : ""}
                >
                  <path d="M213.66,194.34a8,8,0,0,1-11.32,11.32L128,131.31,53.66,205.66a8,8,0,0,1-11.32-11.32l80-80a8,8,0,0,1,11.32,0Zm-160-68.68L128,51.31l74.34,74.35a8,8,0,0,0,11.32-11.32l-80-80a8,8,0,0,0-11.32,0l-80,80a8,8,0,0,0,11.32,11.32Z" />
                </svg>
              </button>
            </div>
            <div className="h-[calc(100%-2rem)]">
              <FileTree
                files={files}
                activeFileKey={activeFileKey}
                viewedFileKeys={viewedFileKeys}
                folderCommand={folderCommand}
                onSelect={scrollToFile}
                onToggleViewed={handleToggleFileViewed}
              />
            </div>
          </aside>
        )}
        <div className="min-w-0 flex-1">
          {files.map((file) => (
            <FileDiff
              key={fileKey(file)}
              file={file}
              viewType={viewMode}
              viewed={viewedFileKeys?.has(fileKey(file)) ?? false}
              collapsed={collapsedFileKeys.has(fileKey(file))}
              widgets={widgetsByFile?.[fileKey(file)]}
              fileComments={fileWidgets?.[fileKey(file)]}
              onGutterClick={onGutterClick}
              onAskLine={onAskLine}
              onToggleViewed={handleToggleFileViewed}
              onToggleCollapsed={handleToggleFileCollapsed}
            />
          ))}
        </div>
      </div>
    </div>
  );
}
