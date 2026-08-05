import { CaretDown, CaretRight } from "@phosphor-icons/react";
import { useState } from "react";
import type { PrGroup } from "@/hooks/usePullRequests";
import { readMigratedStorageValue } from "@/lib/storageMigration";
import { type PrListFilter, type PullRequestSummary, type RepoRef, repoKey } from "@/types";
import { AuthorFilter, type AuthorOption } from "./AuthorFilter";
import { PrListItem } from "./PrListItem";
import { PrStateTabs } from "./PrStateTabs";
import { RepoHeader } from "./RepoHeader";
import { RepositoryFilter, type RepositoryOption } from "./RepositoryFilter";

const COLLAPSED_KEY = "norn.collapsedRepos.v1";
const LEGACY_COLLAPSED_KEY = "lachesi.collapsedRepos";

function normalizeLegacyCollapsed(raw: string): string | null {
  const parsed: unknown = JSON.parse(raw);
  return Array.isArray(parsed) && parsed.every((item) => typeof item === "string")
    ? JSON.stringify(parsed)
    : null;
}

function loadCollapsed(): Set<string> {
  try {
    const value = readMigratedStorageValue(
      localStorage,
      COLLAPSED_KEY,
      LEGACY_COLLAPSED_KEY,
      normalizeLegacyCollapsed,
    );
    return new Set(JSON.parse(value ?? "[]") as string[]);
  } catch {
    return new Set();
  }
}

function persistCollapsed(next: Set<string>) {
  try {
    localStorage.setItem(COLLAPSED_KEY, JSON.stringify([...next]));
  } catch {
    // ignore storage failures
  }
}

export interface PrSidebarActive {
  workspace: string;
  repo: string;
  prId: number;
}

export interface PrSidebarProps {
  groups: PrGroup[];
  filter: PrListFilter;
  loading?: boolean;
  refreshing?: boolean;
  active: PrSidebarActive | null;
  authors: AuthorOption[];
  authorFilter: string | null;
  repositories: RepositoryOption[];
  repositoryFilter: string | null;
  onFilterChange: (filter: PrListFilter) => void;
  onAuthorFilterChange: (key: string | null) => void;
  onRepositoryFilterChange: (key: string | null) => void;
  onSelect: (pr: PullRequestSummary) => void;
  onLoadMore: (repo: RepoRef) => void;
  onRefresh: () => void;
  onOpenSettings: () => void;
  onOpenOverview?: () => void;
  onOpenClosedAnalytics?: () => void;
}

export function PrSidebar({
  groups,
  filter,
  loading,
  refreshing,
  active,
  authors,
  authorFilter,
  repositories,
  repositoryFilter,
  onFilterChange,
  onAuthorFilterChange,
  onRepositoryFilterChange,
  onSelect,
  onLoadMore,
  onRefresh,
  onOpenSettings,
  onOpenOverview,
  onOpenClosedAnalytics,
}: PrSidebarProps) {
  const totalPrs = groups.reduce((n, g) => n + g.pullRequests.length, 0);
  const draftCount = groups.reduce((n, g) => n + g.pullRequests.filter((pr) => pr.draft).length, 0);
  const [collapsed, setCollapsed] = useState<Set<string>>(loadCollapsed);

  const toggle = (key: string) => {
    setCollapsed((prev) => {
      const next = new Set(prev);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      persistCollapsed(next);
      return next;
    });
  };

  const allKeys = groups.map((g) => repoKey(g.repo));
  const allCollapsed = allKeys.length > 0 && allKeys.every((k) => collapsed.has(k));
  const toggleAll = () => {
    const next = allCollapsed ? new Set<string>() : new Set(allKeys);
    setCollapsed(next);
    persistCollapsed(next);
  };

  return (
    <aside className="flex h-full flex-col bg-sidebar text-sidebar-foreground">
      <RepoHeader
        repoCount={groups.length}
        refreshing={refreshing}
        onToggleCollapseAll={toggleAll}
        allCollapsed={allCollapsed}
        onRefresh={onRefresh}
        onOpenSettings={onOpenSettings}
        onOpenOverview={onOpenOverview}
        onOpenClosedAnalytics={onOpenClosedAnalytics}
      />
      <PrStateTabs value={filter} onChange={onFilterChange} counts={{ DRAFT: draftCount }} />
      <RepositoryFilter
        repositories={repositories}
        value={repositoryFilter}
        onChange={onRepositoryFilterChange}
      />
      <AuthorFilter authors={authors} value={authorFilter} onChange={onAuthorFilterChange} />
      <div className="min-h-0 flex-1 overflow-auto">
        {groups.length === 0 ? (
          <div className="p-4 text-sm text-muted-foreground">
            No repositories configured. Open settings to add one.
          </div>
        ) : loading && totalPrs === 0 ? (
          <div className="p-4 text-sm text-muted-foreground">Loading pull requests…</div>
        ) : (
          groups.map((group) => {
            const key = repoKey(group.repo);
            const isCollapsed = collapsed.has(key);
            return (
              <section key={key}>
                <button
                  type="button"
                  onClick={() => toggle(key)}
                  aria-expanded={!isCollapsed}
                  className="sticky top-0 z-10 flex w-full items-center gap-1.5 bg-secondary px-3 py-1.5 text-xs font-medium text-muted-foreground hover:text-foreground"
                >
                  {isCollapsed ? <CaretRight size={12} /> : <CaretDown size={12} />}
                  <span className="truncate">
                    {group.repo.workspace}/
                    <span className="text-foreground">{group.repo.repo}</span>
                  </span>
                  <span className="ml-auto">{group.pullRequests.length}</span>
                </button>
                {!isCollapsed &&
                  (group.error ? (
                    <p className="px-3 py-2 text-xs text-destructive">{group.error}</p>
                  ) : (
                    <>
                      {group.pullRequests.length === 0 && !group.hasNext ? (
                        <p className="px-3 py-2 text-xs text-muted-foreground">No pull requests.</p>
                      ) : (
                        group.pullRequests.map((pr) => (
                          <PrListItem
                            key={`${key}#${pr.id}`}
                            pr={pr}
                            active={
                              active != null &&
                              active.prId === pr.id &&
                              active.workspace === pr.workspace &&
                              active.repo === pr.repo
                            }
                            onSelect={onSelect}
                          />
                        ))
                      )}
                      {group.hasNext && (
                        <button
                          type="button"
                          onClick={() => onLoadMore(group.repo)}
                          disabled={group.loadingMore}
                          className="w-full border-b border-border px-3 py-2 text-center text-xs font-medium text-accent-foreground hover:bg-muted disabled:opacity-60"
                        >
                          {group.loadingMore ? "Loading…" : "Load more"}
                        </button>
                      )}
                    </>
                  ))}
              </section>
            );
          })
        )}
      </div>
    </aside>
  );
}
