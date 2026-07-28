import {
  ArrowLeft,
  ArrowsClockwise,
  ChartBarHorizontal,
  WarningCircle,
} from "@phosphor-icons/react";
import { useEffect, useMemo, useState } from "react";
import {
  Bar,
  BarChart,
  CartesianGrid,
  Cell,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import { Button } from "@/components/ui/button";
import { useReviewEffectiveness } from "@/hooks/useReviewEffectiveness";
import type {
  RepoRef,
  ReviewEffectivenessReport,
  ReviewMetricCount,
  ReviewProvider,
} from "@/types";
import { repoKey } from "@/types";

export type ReviewEffectivenessRange = 7 | 30 | 90 | null;

const RANGE_OPTIONS: ReadonlyArray<{
  value: ReviewEffectivenessRange;
  label: string;
  scopeLabel: string;
}> = [
  { value: 7, label: "7d", scopeLabel: "Last 7 days" },
  { value: 30, label: "30d", scopeLabel: "Last 30 days" },
  { value: 90, label: "90d", scopeLabel: "Last 90 days" },
  { value: null, label: "All", scopeLabel: "All time" },
];

const CHART_COLORS = [
  "var(--accent-foreground)",
  "var(--success)",
  "var(--warning)",
  "var(--destructive)",
  "var(--muted-foreground)",
];

const NUMBER_FORMATTER = new Intl.NumberFormat();

function formatNumber(value: number): string {
  return NUMBER_FORMATTER.format(value);
}

function formatRate(basisPoints: number | null): string {
  if (basisPoints == null) return "N/A";
  return `${(basisPoints / 100).toLocaleString(undefined, {
    maximumFractionDigits: 1,
  })}%`;
}

function formatDuration(milliseconds: number | null): string {
  if (milliseconds == null) return "N/A";
  const minutes = milliseconds / 60_000;
  if (minutes < 1) return "< 1m";
  if (minutes < 60) return `${Math.round(minutes)}m`;
  const hours = minutes / 60;
  if (hours < 24) {
    return `${hours.toLocaleString(undefined, { maximumFractionDigits: 1 })}h`;
  }
  return `${(hours / 24).toLocaleString(undefined, { maximumFractionDigits: 1 })}d`;
}

function rangeScopeLabel(days: ReviewEffectivenessRange): string {
  return RANGE_OPTIONS.find((option) => option.value === days)?.scopeLabel ?? "All time";
}

function repositoryLabel(repository: RepoRef | null): string {
  return repository == null ? "All repositories" : `${repository.workspace}/${repository.repo}`;
}

function MetricCard({ label, value, detail }: { label: string; value: string; detail: string }) {
  return (
    <div className="flex min-h-28 flex-col justify-between gap-2 rounded-md border border-border bg-card px-4 py-3 shadow-sm">
      <div className="text-xs font-medium text-muted-foreground">{label}</div>
      <div className="text-2xl font-semibold tabular-nums">{value}</div>
      <div className="text-xs text-muted-foreground">{detail}</div>
    </div>
  );
}

function BreakdownChart({
  title,
  scopeLabel,
  values,
}: {
  title: string;
  scopeLabel: string;
  values: ReviewMetricCount[];
}) {
  const titleId = `${title.replace(/ /g, "-").toLowerCase()}-title`;
  const data = values
    .filter((item) => item.count > 0)
    .sort((left, right) => right.count - left.count || left.key.localeCompare(right.key));
  const accessibleSummary = data.map((item) => `${item.key}: ${item.count}`).join(", ");

  return (
    <section
      className="flex min-h-64 flex-col gap-3 rounded-md border border-border bg-card p-4 shadow-sm"
      aria-labelledby={titleId}
    >
      <div>
        <h2 id={titleId} className="text-sm font-semibold">
          {title}
        </h2>
        <p className="mt-0.5 text-xs text-muted-foreground">{scopeLabel}</p>
      </div>
      {data.length === 0 ? (
        <div className="flex flex-1 items-center justify-center text-sm text-muted-foreground">
          No findings in this scope.
        </div>
      ) : (
        <div
          className="h-48 min-w-0"
          role="img"
          aria-label={`${title} for ${scopeLabel}: ${accessibleSummary}`}
        >
          <ResponsiveContainer width="100%" height="100%">
            <BarChart data={data} layout="vertical" barSize={18}>
              <CartesianGrid strokeDasharray="3 3" stroke="var(--border)" horizontal={false} />
              <XAxis
                type="number"
                allowDecimals={false}
                tick={{ fontSize: 11, fill: "var(--muted-foreground)" }}
                axisLine={false}
                tickLine={false}
              />
              <YAxis
                type="category"
                dataKey="key"
                width={112}
                tick={{ fontSize: 11, fill: "var(--muted-foreground)" }}
                axisLine={false}
                tickLine={false}
              />
              <Tooltip
                cursor={{ fill: "var(--muted)" }}
                contentStyle={{
                  background: "var(--popover)",
                  border: "1px solid var(--border)",
                  borderRadius: "6px",
                  color: "var(--popover-foreground)",
                  fontSize: 12,
                }}
                formatter={(value) => [value, "Findings"]}
              />
              <Bar dataKey="count" minPointSize={4} radius={[0, 4, 4, 0]} isAnimationActive={false}>
                {data.map((item, index) => (
                  <Cell key={item.key} fill={CHART_COLORS[index % CHART_COLORS.length]} />
                ))}
              </Bar>
            </BarChart>
          </ResponsiveContainer>
        </div>
      )}
    </section>
  );
}

export interface ReviewEffectivenessDashboardViewProps {
  report: ReviewEffectivenessReport | null;
  loading: boolean;
  error: string | null;
  repositories: RepoRef[];
  selectedRepository: RepoRef | null;
  days: ReviewEffectivenessRange;
  onRepositoryChange: (repository: RepoRef | null) => void;
  onDaysChange: (days: ReviewEffectivenessRange) => void;
  onRetry: () => void;
  onBack: () => void;
}

export function ReviewEffectivenessDashboardView({
  report,
  loading,
  error,
  repositories,
  selectedRepository,
  days,
  onRepositoryChange,
  onDaysChange,
  onRetry,
  onBack,
}: ReviewEffectivenessDashboardViewProps) {
  const scopeLabel = `${repositoryLabel(selectedRepository)} · ${rangeScopeLabel(days)}`;
  const summary = report?.summary ?? null;

  return (
    <div className="flex h-full flex-col overflow-hidden">
      <header className="flex shrink-0 flex-wrap items-center gap-3 border-b border-border px-4 py-2.5">
        <Button variant="ghost" size="sm" onClick={onBack} className="gap-1.5">
          <ArrowLeft size={14} />
          PR list
        </Button>
        <div className="min-w-48 flex-1">
          <h1 className="text-sm font-semibold">Review effectiveness</h1>
          <p className="text-xs text-muted-foreground">{scopeLabel}</p>
        </div>

        <fieldset className="flex items-center rounded-md border border-border p-0.5">
          <legend className="sr-only">Period</legend>
          {RANGE_OPTIONS.map((option) => (
            <Button
              key={option.label}
              type="button"
              variant={option.value === days ? "secondary" : "ghost"}
              size="sm"
              className="h-7 min-w-10 px-2 text-xs"
              aria-pressed={option.value === days}
              onClick={() => onDaysChange(option.value)}
            >
              {option.label}
            </Button>
          ))}
        </fieldset>

        <label className="flex items-center gap-2 text-xs text-muted-foreground">
          Repository
          <select
            aria-label="Repository"
            value={selectedRepository == null ? "" : repoKey(selectedRepository)}
            onChange={(event) => {
              const next =
                repositories.find((repo) => repoKey(repo) === event.target.value) ?? null;
              onRepositoryChange(next);
            }}
            className="max-w-64 rounded-md border border-input bg-background px-2 py-1.5 text-xs text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
          >
            <option value="">All repositories</option>
            {repositories.map((repository) => (
              <option key={repoKey(repository)} value={repoKey(repository)}>
                {repository.workspace}/{repository.repo}
              </option>
            ))}
          </select>
        </label>

        <Button
          variant="ghost"
          size="icon"
          onClick={onRetry}
          aria-label="Refresh metrics"
          title="Refresh metrics"
          disabled={loading}
        >
          <ArrowsClockwise size={16} className={loading ? "animate-spin" : undefined} />
        </Button>
      </header>

      <main className="flex-1 overflow-y-auto px-4 py-4">
        {loading && report == null ? (
          <div
            className="flex h-48 items-center justify-center gap-2 text-sm text-muted-foreground"
            role="status"
          >
            <ArrowsClockwise size={16} className="animate-spin" />
            Loading review metrics...
          </div>
        ) : error ? (
          <div className="flex h-48 flex-col items-center justify-center gap-3" role="alert">
            <div className="text-sm font-medium">Review metrics could not be loaded.</div>
            <div className="max-w-xl text-center text-xs text-muted-foreground">{error}</div>
            <Button variant="secondary" size="sm" onClick={onRetry}>
              <ArrowsClockwise size={14} />
              Retry
            </Button>
          </div>
        ) : summary == null || (summary.reviewCount === 0 && summary.findingCount === 0) ? (
          <div className="flex h-48 flex-col items-center justify-center gap-2 text-center">
            <ChartBarHorizontal size={24} className="text-muted-foreground" />
            <div className="text-sm font-medium">No review metrics in this scope.</div>
            <div className="text-xs text-muted-foreground">{scopeLabel}</div>
          </div>
        ) : (
          <div className="flex flex-col gap-4">
            {summary.feedback.findingsWithoutFeedback > 0 && (
              <div
                className="flex items-start gap-2 rounded-md border px-3 py-2.5"
                role="status"
                style={{
                  background: "color-mix(in srgb, var(--warning) 10%, transparent)",
                  borderColor: "var(--warning)",
                }}
              >
                <WarningCircle size={18} className="mt-0.5 shrink-0" color="var(--warning)" />
                <div>
                  <div className="text-sm font-medium">Feedback coverage is incomplete</div>
                  <div className="text-xs text-muted-foreground">
                    {formatNumber(summary.feedback.findingsWithoutFeedback)} of{" "}
                    {formatNumber(summary.feedback.eligibleFindings)} eligible findings have no
                    reviewer outcome. Acceptance and false-positive rates keep them in their
                    denominators.
                  </div>
                </div>
              </div>
            )}

            <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 xl:grid-cols-5">
              <MetricCard
                label="Reviews"
                value={formatNumber(summary.reviewCount)}
                detail={scopeLabel}
              />
              <MetricCard
                label="Findings"
                value={formatNumber(summary.findingCount)}
                detail={`${formatRate(summary.feedback.coverageRate.basisPoints)} feedback coverage`}
              />
              <MetricCard
                label="Acceptance"
                value={formatRate(summary.feedback.acceptanceRate.basisPoints)}
                detail={`${formatNumber(summary.feedback.acceptedFindings)} accepted findings`}
              />
              <MetricCard
                label="False positives"
                value={formatRate(summary.feedback.falsePositiveRate.basisPoints)}
                detail={`${formatNumber(summary.feedback.falsePositiveFindings)} marked false positive`}
              />
              <MetricCard
                label="Median first review"
                value={formatDuration(summary.timeToFirstReview.medianMs)}
                detail={`${formatNumber(summary.timeToFirstReview.sampleCount)} pull request samples`}
              />
            </div>

            <div className="grid grid-cols-1 gap-4 lg:grid-cols-2">
              <BreakdownChart
                title="Findings by severity"
                scopeLabel={scopeLabel}
                values={summary.findingsBySeverity}
              />
              <BreakdownChart
                title="Findings by category"
                scopeLabel={scopeLabel}
                values={summary.findingsByCategory}
              />
            </div>
          </div>
        )}
      </main>
    </div>
  );
}

interface ReviewEffectivenessDashboardProps {
  repositories: RepoRef[];
  provider: ReviewProvider;
  onBack: () => void;
}

export function ReviewEffectivenessDashboard({
  repositories,
  provider,
  onBack,
}: ReviewEffectivenessDashboardProps) {
  const [days, setDays] = useState<ReviewEffectivenessRange>(30);
  const [selectedRepositoryKey, setSelectedRepositoryKey] = useState<string | null>(null);
  const selectedRepository = useMemo(
    () => repositories.find((repository) => repoKey(repository) === selectedRepositoryKey) ?? null,
    [repositories, selectedRepositoryKey],
  );

  useEffect(() => {
    if (selectedRepositoryKey != null && selectedRepository == null) {
      setSelectedRepositoryKey(null);
    }
  }, [selectedRepository, selectedRepositoryKey]);

  const metrics = useReviewEffectiveness({
    enabled: true,
    provider,
    repository: selectedRepository,
    days,
  });

  return (
    <ReviewEffectivenessDashboardView
      report={metrics.report}
      loading={metrics.loading}
      error={metrics.error}
      repositories={repositories}
      selectedRepository={selectedRepository}
      days={days}
      onRepositoryChange={(repository) =>
        setSelectedRepositoryKey(repository == null ? null : repoKey(repository))
      }
      onDaysChange={setDays}
      onRetry={() => void metrics.refresh()}
      onBack={onBack}
    />
  );
}
