import { Moon, Sun } from "@phosphor-icons/react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { DiffViewer } from "@/components/diff/DiffViewer";
import { parseUnifiedDiff } from "@/lib/diff";
import {
  countReviewFileChanges,
  imageDiffKey,
  imageMimeTypeForPath,
  imagePreviewPath,
  imagePreviewSide,
  mergeImageDiffstat,
} from "@/lib/imageDiff";
import type { DiffstatEntry, DiffViewMode } from "@/types";

const POLL_INTERVAL_MS = 1_200;
const BROWSER_IMAGE_EXTENSIONS = [".gif", ".jpeg", ".jpg", ".png", ".webp"];

export interface BrowserDiffState {
  version: number;
  workspace: string;
  repo: string;
  prId: number;
  prTitle: string;
  prAuthor: string;
  sourceBranch: string;
  targetBranch: string;
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

export function BrowserDiffApp() {
  const [remoteState, setRemoteState] = useState<BrowserDiffState | null>(null);
  const [viewMode, setViewMode] = useState<Exclude<DiffViewMode, "conversation">>("split");
  const [error, setError] = useState<string | null>(null);
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
    // The server owns a session-scoped monotonic version that advances whenever the PR identity
    // or its content changes, so one cursor safely covers PR switches within this session.
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
          setError(
            cause instanceof Error ? cause.message : "Could not load the pull request diff.",
          );
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
      parseUnifiedDiff(remoteState.diff ?? ""),
      diffstat,
      previewStates(diffstat, window.location.pathname),
    );
  }, [remoteState]);

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

  const populationWarning = remoteState?.populationFailed
    ? "Some diff data could not be loaded. Return to Norn and open the browser diff again to retry."
    : null;
  const activeWarning = populationWarning ?? error;
  const stateError = files.length === 0 ? activeWarning : null;

  return (
    <main className="min-h-full bg-background text-foreground">
      <header className="flex min-h-16 flex-wrap items-center gap-4 border-b border-border bg-background px-5 py-3">
        <div className="flex min-w-0 flex-1 items-center gap-3">
          <span className="shrink-0 rounded-md bg-primary px-2 py-1 text-[11px] font-bold tracking-wide text-primary-foreground">
            NORN DIFF
          </span>
          <div className="min-w-0">
            <div className="flex min-w-0 items-baseline gap-2">
              <span className="shrink-0 text-sm font-semibold text-[var(--norn-accent-strong)]">
                #{remoteState?.prId ?? "…"}
              </span>
              <h1 className="truncate text-sm font-semibold">
                {remoteState?.prTitle ?? "Loading pull request…"}
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
          <button
            type="button"
            className="inline-flex size-8 items-center justify-center rounded-md border border-border bg-background text-muted-foreground hover:bg-muted hover:text-foreground"
            onClick={() => setTheme((current) => (current === "dark" ? "light" : "dark"))}
            aria-label={`Use ${theme === "dark" ? "light" : "dark"} theme`}
            title={`Use ${theme === "dark" ? "light" : "dark"} theme`}
          >
            {theme === "dark" ? <Sun size={15} /> : <Moon size={15} />}
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

      <DiffViewer
        files={files}
        viewMode={viewMode}
        onViewModeChange={(mode) => {
          if (mode !== "conversation") setViewMode(mode);
        }}
        loading={!remoteState && !stateError}
        error={stateError}
      />
    </main>
  );
}
