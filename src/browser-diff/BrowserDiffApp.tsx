import { Moon, Sun } from "@phosphor-icons/react";
import { parsePatchFiles } from "@pierre/diffs";
import { FileDiff } from "@pierre/diffs/react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { FileTree } from "@/components/diff/FileTree";
import { fileDisplayPath, fileKey, parseUnifiedDiff } from "@/lib/diff";
import type { ImageDiffMetadata, ReviewFileData } from "@/lib/imageDiff";
import {
  countReviewFileChanges,
  imageDiffKey,
  imageMimeTypeForPath,
  imagePreviewPath,
  imagePreviewSide,
  mergeImageDiffstat,
} from "@/lib/imageDiff";
import type { DiffstatEntry, DiffViewMode } from "@/types";

import { browserFileAnchor } from "./fileAnchor";

const POLL_INTERVAL_MS = 1_200;
const BROWSER_IMAGE_EXTENSIONS = [".gif", ".jpeg", ".jpg", ".png", ".webp"];

export interface BrowserDiffState {
  version: number;
  targetKind: "pullRequest" | "local";
  workspace: string;
  repo: string;
  prId: number;
  prTitle: string;
  prAuthor: string;
  sourceBranch: string;
  targetBranch: string;
  baseSha: string | null;
  diff: string | null;
  diffstat: DiffstatEntry[] | null;
  populationFailed: boolean;
}

function sessionBasePath(pathname: string): string {
  const match = pathname.match(/^(\/session\/[0-9a-f]{64})(?:\/.*)?$/);
  if (!match) throw new Error("This browser diff session URL is invalid.");
  return match[1];
}

export function browserDiffApiUrl(
  pathname: string,
  route: string,
  params: Record<string, string | number> = {},
): string {
  const url = new URL(`${sessionBasePath(pathname)}${route}`, window.location.origin);
  for (const [key, value] of Object.entries(params)) url.searchParams.set(key, String(value));
  return `${url.pathname}${url.search}`;
}

function browserPreviewPath(entry: DiffstatEntry): string | null {
  // Keep the desktop renderer's one-preview-per-entry contract. Modified and renamed images
  // display the new side; removed images display the old side.
  const path = imagePreviewPath(entry);
  if (!path) return null;
  const normalized = path.toLowerCase();
  return BROWSER_IMAGE_EXTENSIONS.some((extension) => normalized.endsWith(extension)) ? path : null;
}

function previewStates(entries: DiffstatEntry[], pathname: string) {
  return Object.fromEntries(
    entries.flatMap((entry) => {
      const path = browserPreviewPath(entry);
      if (!path) return [];
      const side = imagePreviewSide(entry);
      return [
        [
          imageDiffKey(entry),
          {
            status: "ready" as const,
            preview: {
              path,
              mimeType: imageMimeTypeForPath(path) ?? "application/octet-stream",
              dataUrl: browserDiffApiUrl(pathname, "/api/file-preview", { path, side }),
              size: 0,
            },
            error: null,
          },
        ],
      ];
    }),
  );
}

function shortRepository(state: BrowserDiffState): string {
  return state.workspace ? `${state.workspace}/${state.repo}` : state.repo;
}

function BrowserImagePanel({ file }: { file: ReviewFileData & { imageDiff: ImageDiffMetadata } }) {
  const image = file.imageDiff;
  const path = image.preview.preview?.path ?? image.path;
  const label =
    image.oldPath && image.newPath && image.oldPath !== image.newPath
      ? `${image.oldPath} → ${image.newPath}`
      : path;
  return (
    <div className="border-b border-border bg-background px-4 py-4">
      <div className="mb-1 break-all text-xs font-medium text-foreground">{label}</div>
      <div className="mb-3 flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
        <span className="rounded-md border border-border px-1.5 py-0.5">{image.mimeType}</span>
        <span>{image.previewSide === "old" ? "base image" : "new image"}</span>
      </div>
      {image.preview.status === "ready" ? (
        <div className="overflow-auto rounded-md border border-border bg-muted/20 p-3">
          <img
            src={image.preview.preview.dataUrl}
            alt={path}
            className="mx-auto max-h-[70vh] max-w-full object-contain"
            loading="lazy"
            decoding="async"
          />
        </div>
      ) : (
        <div className="rounded-md border border-dashed border-border bg-muted/30 px-4 py-8 text-center text-sm text-muted-foreground">
          Image preview is not loaded yet.
        </div>
      )}
    </div>
  );
}

export function BrowserDiffApp() {
  const [remoteState, setRemoteState] = useState<BrowserDiffState | null>(null);
  const [viewMode, setViewMode] = useState<Exclude<DiffViewMode, "conversation">>("split");
  const [error, setError] = useState<string | null>(null);
  const [showFiles, setShowFiles] = useState(true);
  const [selectedFileKey, setSelectedFileKey] = useState<string | null>(null);
  const [theme, setTheme] = useState<"dark" | "light">("dark");

  const loadState = useCallback(async (knownVersion: number, signal: AbortSignal) => {
    const response = await fetch(
      browserDiffApiUrl(window.location.pathname, "/api/state", { version: knownVersion }),
      { cache: "no-store", credentials: "same-origin", signal },
    );
    if (response.status === 204) return null;
    if (!response.ok) throw new Error(`The local diff server returned HTTP ${response.status}.`);
    return (await response.json()) as BrowserDiffState;
  }, []);

  useEffect(() => {
    const controller = new AbortController();
    // The server owns a session-scoped monotonic version that advances whenever the review target
    // or its content changes, so one cursor safely covers target switches within this session.
    let version = -1;
    let timeout: number | undefined;

    const poll = async () => {
      try {
        const next = await loadState(version, controller.signal);
        if (controller.signal.aborted) return;
        setError(null);
        if (next) {
          version = next.version;
          setRemoteState(next);
        }
      } catch (cause) {
        if (!controller.signal.aborted) {
          setError(cause instanceof Error ? cause.message : "Could not load the review diff.");
        }
      } finally {
        if (!controller.signal.aborted) timeout = window.setTimeout(poll, POLL_INTERVAL_MS);
      }
    };

    void poll();
    return () => {
      controller.abort();
      if (timeout !== undefined) window.clearTimeout(timeout);
    };
  }, [loadState]);

  useEffect(() => {
    document.documentElement.dataset.theme = theme;
  }, [theme]);

  const files = useMemo(() => {
    if (!remoteState) return [];
    const diffstat = remoteState.diffstat ?? [];
    return mergeImageDiffstat(
      (() => {
        try {
          return parseUnifiedDiff(remoteState.diff ?? "");
        } catch {
          return [];
        }
      })(),
      diffstat,
      previewStates(diffstat, window.location.pathname),
    );
  }, [remoteState]);

  const pierre = useMemo(() => {
    const patch = remoteState?.diff ?? "";
    if (!patch.trim()) return { files: [], error: null };
    try {
      const files = parsePatchFiles(patch, `browser-diff-${remoteState?.version}`, true).flatMap(
        (patch) => patch.files,
      );
      if (!files.length) throw new Error("Empty parsed patch");
      return { files, error: null };
    } catch {
      return {
        files: [],
        error: "The formatted diff could not be rendered. The original patch is shown below.",
      };
    }
  }, [remoteState?.diff, remoteState?.version]);

  const totals = useMemo(
    () =>
      files.reduce(
        (result, file) => {
          const count = countReviewFileChanges(file);
          result.additions += count.additions;
          result.deletions += count.deletions;
          return result;
        },
        { additions: 0, deletions: 0 },
      ),
    [files],
  );

  const imageFiles = useMemo(
    () =>
      files.filter(
        (file): file is ReviewFileData & { imageDiff: ImageDiffMetadata } => file.imageDiff != null,
      ),
    [files],
  );

  const treeFiles = useMemo(() => {
    const paths = new Set<string>();
    return files.filter((file) => {
      const path = fileDisplayPath(file);
      if (paths.has(path)) return false;
      paths.add(path);
      return true;
    });
  }, [files]);
  const firstDiffByPath = useMemo(() => {
    const first = new Map<string, (typeof pierre.files)[number]>();
    for (const file of pierre.files) if (!first.has(file.name)) first.set(file.name, file);
    return first;
  }, [pierre.files]);
  const imagePaths = new Set(imageFiles.map(fileDisplayPath));
  const selectFile = (file: ReviewFileData) => {
    setSelectedFileKey(fileKey(file));
    const target =
      document.getElementById(browserFileAnchor(fileDisplayPath(file))) ??
      document.getElementById("browser-raw-patch");
    target?.scrollIntoView({ behavior: "smooth", block: "start" });
  };

  const populationWarning = remoteState?.populationFailed
    ? "Some diff data could not be loaded. Return to Norn and open the browser diff again to retry."
    : null;
  const activeWarning = populationWarning ?? error;
  const stateError = files.length === 0 ? activeWarning : null;

  return (
    <main className="flex h-screen flex-col overflow-hidden bg-background text-foreground">
      <header className="flex min-h-16 shrink-0 flex-wrap items-center gap-4 border-b border-border bg-background px-5 py-3">
        <div className="flex min-w-0 flex-1 items-center gap-3">
          <span className="shrink-0 rounded-md bg-primary px-2 py-1 text-[11px] font-bold tracking-wide text-primary-foreground">
            NORN DIFF
          </span>
          <div className="min-w-0">
            <div className="flex min-w-0 items-baseline gap-2">
              <span className="shrink-0 text-sm font-semibold text-[var(--norn-accent-strong)]">
                {remoteState?.targetKind === "local"
                  ? "LOCAL"
                  : remoteState
                    ? `#${remoteState.prId}`
                    : "…"}
              </span>
              <h1 className="truncate text-sm font-semibold">
                {remoteState?.prTitle ?? "Loading review target…"}
              </h1>
            </div>
            <div className="mt-0.5 flex flex-wrap items-center gap-x-2 text-xs text-muted-foreground">
              <span>{remoteState ? shortRepository(remoteState) : "Loading repository…"}</span>
              {remoteState && (
                <>
                  <span aria-hidden="true">·</span>
                  <span>
                    {remoteState.sourceBranch} → {remoteState.targetBranch}
                  </span>
                  <span aria-hidden="true">·</span>
                  <span>
                    {files.length} files changed (
                    <span className="text-[var(--success)]">+{totals.additions}</span>{" "}
                    <span className="text-destructive">-{totals.deletions}</span>)
                  </span>
                </>
              )}
            </div>
          </div>
        </div>
        <div className="flex items-center gap-3">
          <div className="flex overflow-hidden rounded-md border border-border">
            {(["split", "unified"] as const).map((mode) => (
              <button
                key={mode}
                type="button"
                className={`px-2 py-1 text-xs capitalize ${
                  viewMode === mode
                    ? "bg-muted text-foreground"
                    : "bg-background text-muted-foreground hover:text-foreground"
                }`}
                onClick={() => setViewMode(mode)}
              >
                {mode}
              </button>
            ))}
          </div>
          <button
            type="button"
            className="inline-flex size-8 items-center justify-center rounded-md border border-border bg-background text-muted-foreground hover:bg-muted hover:text-foreground"
            onClick={() => setTheme((current) => (current === "dark" ? "light" : "dark"))}
            aria-label={`Use ${theme === "dark" ? "light" : "dark"} theme`}
            title={`Use ${theme === "dark" ? "light" : "dark"} theme`}
          >
            {theme === "dark" ? <Sun size={15} /> : <Moon size={15} />}
          </button>
          <button
            type="button"
            onClick={() => setShowFiles((value) => !value)}
            aria-expanded={showFiles}
            className="rounded-md border border-border px-2 py-1 text-xs"
          >
            Files
          </button>
          <span className="flex items-center gap-1.5 text-xs text-muted-foreground">
            <span className="size-2 rounded-full bg-[var(--success)] shadow-[0_0_8px_var(--success)]" />
            Live
          </span>
        </div>
      </header>

      {activeWarning && files.length > 0 && (
        <div className="border-b border-destructive/40 bg-destructive/10 px-5 py-2 text-xs text-destructive">
          {activeWarning}
        </div>
      )}

      <div className="flex min-h-0 flex-1">
        {showFiles && (
          <aside className="w-64 max-w-[40vw] shrink-0 border-r border-border">
            <FileTree files={treeFiles} activeFileKey={selectedFileKey} onSelect={selectFile} />
          </aside>
        )}
        <section className="min-h-0 min-w-0 flex-1 overflow-auto">
          {stateError ? (
            <div className="px-5 py-8 text-sm text-muted-foreground">{stateError}</div>
          ) : !remoteState ? (
            <div className="px-5 py-8 text-sm text-muted-foreground">Loading review target…</div>
          ) : (
            <>
              {pierre.files.map((fileDiff) => (
                <article
                  key={`${fileDiff.cacheKey}:${fileDiff.prevName ?? ""}:${fileDiff.name}`}
                  id={
                    !imagePaths.has(fileDiff.name) &&
                    firstDiffByPath.get(fileDiff.name) === fileDiff
                      ? browserFileAnchor(fileDiff.name)
                      : undefined
                  }
                >
                  <FileDiff
                    fileDiff={fileDiff}
                    options={{
                      theme: theme === "dark" ? "github-dark" : "github-light",
                      diffStyle: viewMode,
                    }}
                    disableWorkerPool
                  />
                </article>
              ))}
              {pierre.error && (
                <div className="px-5 py-4">
                  <p role="alert" className="mb-3 text-sm text-destructive">
                    {pierre.error}
                  </p>
                  <pre id="browser-raw-patch" className="whitespace-pre-wrap break-words text-xs">
                    {remoteState.diff}
                  </pre>
                </div>
              )}
              {imageFiles.map((file) => (
                <article
                  key={`${file.imageDiff!.oldPath}->${file.imageDiff!.newPath}`}
                  id={browserFileAnchor(fileDisplayPath(file))}
                >
                  <BrowserImagePanel file={file} />
                </article>
              ))}
              {(remoteState.diff ?? "").trim().length === 0 && imageFiles.length === 0 ? (
                <div className="px-5 py-8 text-sm text-muted-foreground">
                  {remoteState.targetKind === "local"
                    ? "No unpublished local changes."
                    : "No changes in this pull request."}
                </div>
              ) : null}
            </>
          )}
        </section>
      </div>
    </main>
  );
}
