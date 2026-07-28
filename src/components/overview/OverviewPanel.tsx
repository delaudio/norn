import { ArrowLeft, ArrowsClockwise, ChartLineUp, Pulse } from "@phosphor-icons/react";
import { useState } from "react";
import type { AuthorOption } from "@/components/pr-sidebar/AuthorFilter";
import { Button } from "@/components/ui/button";
import { authorKey } from "@/hooks/useCurrentUser";
import type { PrGroup } from "@/hooks/usePullRequests";
import type { PullRequestSummary } from "@/types";
import { PrAgeChart } from "./PrAgeChart";
import { PrByRepoChart } from "./PrByRepoChart";
import { PrTable } from "./PrTable";
import { StatsCards } from "./StatsCards";

export interface OverviewPanelProps {
  /** All loaded PR groups (OPEN). */
  groups: PrGroup[];
  loading: boolean;
  onRefresh: () => void;
  onBack: () => void;
  onOpenClosedAnalytics: () => void;
  onOpenReviewEffectiveness: () => void;
  onSelectPr: (pr: PullRequestSummary) => void;
  /** The currently authenticated user, used to pin "me" first in the filter. */
  currentUser: { displayName: string; accountId?: string | null } | null;
}

export function OverviewPanel({
  groups,
  loading,
  onRefresh,
  onBack,
  onOpenClosedAnalytics,
  onOpenReviewEffectiveness,
  onSelectPr,
  currentUser,
}: OverviewPanelProps) {
  const [authorFilter, setAuthorFilter] = useState<string | null>(null);

  const meKey = currentUser ? authorKey(currentUser.accountId, currentUser.displayName) : null;

  // Flatten all PRs from all groups.
  const allPrs: PullRequestSummary[] = groups.flatMap((g) => g.pullRequests);

  // Build author list with "me" pinned first.
  const authorMap = new Map<string, AuthorOption>();
  for (const pr of allPrs) {
    const key = authorKey(pr.authorAccountId, pr.authorDisplayName);
    if (!authorMap.has(key)) {
      authorMap.set(key, {
        key,
        label: pr.authorDisplayName,
        isMe: meKey != null && key === meKey,
      });
    }
  }
  const authors = [...authorMap.values()].sort((a, b) =>
    a.isMe ? -1 : b.isMe ? 1 : a.label.localeCompare(b.label),
  );

  const visiblePrs =
    authorFilter == null
      ? allPrs
      : allPrs.filter((pr) => authorKey(pr.authorAccountId, pr.authorDisplayName) === authorFilter);

  return (
    <div className="flex h-full flex-col overflow-hidden">
      {/* Internal header */}
      <div className="flex shrink-0 flex-wrap items-center gap-2 border-b border-border px-4 py-2.5">
        <Button
          variant="ghost"
          size="sm"
          onClick={onBack}
          className="gap-1.5"
          aria-label="Back to PR list"
        >
          <ArrowLeft size={14} />
          PR list
        </Button>
        <div className="min-w-0 flex-1">
          <span className="text-sm font-semibold">Overview</span>
          {allPrs.length > 0 && (
            <span className="ml-2 text-xs text-muted-foreground">
              {allPrs.length} open PR{allPrs.length !== 1 ? "s" : ""}
            </span>
          )}
        </div>

        <Button variant="secondary" size="sm" onClick={onOpenReviewEffectiveness}>
          <Pulse size={14} />
          Review effectiveness
        </Button>

        <Button variant="secondary" size="sm" onClick={onOpenClosedAnalytics}>
          <ChartLineUp size={14} />
          Closed analytics
        </Button>

        {/* Author filter */}
        {authors.length > 0 && (
          <select
            aria-label="Filter by author"
            value={authorFilter ?? ""}
            onChange={(e) => setAuthorFilter(e.target.value || null)}
            className="rounded-md border border-input bg-background px-2 py-1 text-xs focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
          >
            <option value="">All authors</option>
            {authors.map((a) => (
              <option key={a.key} value={a.key}>
                {a.isMe ? `${a.label} (me)` : a.label}
              </option>
            ))}
          </select>
        )}

        <Button
          variant="ghost"
          size="icon"
          onClick={onRefresh}
          aria-label="Refresh"
          title="Refresh"
          disabled={loading}
        >
          <ArrowsClockwise size={16} className={loading ? "animate-spin" : undefined} />
        </Button>
      </div>

      {/* Body */}
      <div className="flex-1 overflow-y-auto px-4 py-4">
        {loading && allPrs.length === 0 ? (
          <div className="flex h-32 items-center justify-center text-sm text-muted-foreground">
            Loading…
          </div>
        ) : (
          <div className="flex flex-col gap-5">
            <StatsCards pullRequests={visiblePrs} />

            <div className="grid grid-cols-1 gap-4 lg:grid-cols-2">
              <PrAgeChart pullRequests={visiblePrs} />
              <PrByRepoChart pullRequests={visiblePrs} />
            </div>

            <div className="flex flex-col gap-2">
              <div className="text-sm font-semibold">
                Open PRs{" "}
                <span className="text-xs font-normal text-muted-foreground">oldest first</span>
              </div>
              <PrTable
                pullRequests={visiblePrs}
                currentUserAccountId={currentUser?.accountId ?? null}
                onSelectPr={onSelectPr}
              />
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
