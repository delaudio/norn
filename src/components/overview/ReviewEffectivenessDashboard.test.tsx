import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { mockReviewEffectivenessReport } from "@/mock-tauri/fixtures";
import type { RepoRef, ReviewEffectivenessReport } from "@/types";
import {
  ReviewEffectivenessDashboardView,
  type ReviewEffectivenessDashboardViewProps,
} from "./ReviewEffectivenessDashboard";

vi.mock("recharts", () => ({
  Bar: ({ children }: { children?: React.ReactNode }) => <div>{children}</div>,
  BarChart: ({ children }: { children?: React.ReactNode }) => <div>{children}</div>,
  CartesianGrid: () => null,
  Cell: () => null,
  ResponsiveContainer: ({ children }: { children?: React.ReactNode }) => <div>{children}</div>,
  Tooltip: () => null,
  XAxis: () => null,
  YAxis: () => null,
}));

const REPOSITORIES: RepoRef[] = [
  { provider: "bitbucket", workspace: "example-workspace", repo: "frontend-app" },
  { provider: "bitbucket", workspace: "example-workspace", repo: "backend-api" },
];

function emptyReport(): ReviewEffectivenessReport {
  return {
    ...mockReviewEffectivenessReport,
    summary: {
      reviewCount: 0,
      findingCount: 0,
      findingsBySeverity: [],
      findingsByCategory: [],
      feedback: {
        eligibleFindings: 0,
        findingsWithFeedback: 0,
        findingsWithoutFeedback: 0,
        acceptedFindings: 0,
        falsePositiveFindings: 0,
        fixedFindings: 0,
        dismissedFindings: 0,
        reopenedFindings: 0,
        coverageRate: { numerator: 0, denominator: 0, basisPoints: null },
        acceptanceRate: { numerator: 0, denominator: 0, basisPoints: null },
        falsePositiveRate: { numerator: 0, denominator: 0, basisPoints: null },
        fixedRate: { numerator: 0, denominator: 0, basisPoints: null },
      },
      timeToFirstReview: {
        sampleCount: 0,
        totalMs: 0,
        averageMs: null,
        medianMs: null,
        minimumMs: null,
        maximumMs: null,
      },
    },
    repositories: [],
  };
}

function renderView(overrides: Partial<ReviewEffectivenessDashboardViewProps> = {}) {
  const props: ReviewEffectivenessDashboardViewProps = {
    report: mockReviewEffectivenessReport,
    loading: false,
    error: null,
    repositories: REPOSITORIES,
    selectedRepository: null,
    days: 30,
    onRepositoryChange: vi.fn(),
    onDaysChange: vi.fn(),
    onRetry: vi.fn(),
    onBack: vi.fn(),
    ...overrides,
  };
  return { ...render(<ReviewEffectivenessDashboardView {...props} />), props };
}

describe("ReviewEffectivenessDashboardView", () => {
  it("exposes an accessible loading state", () => {
    renderView({ report: null, loading: true });

    expect(screen.getByRole("status")).toHaveTextContent("Loading review metrics");
  });

  it("shows the error and lets the reviewer retry", async () => {
    const user = userEvent.setup();
    const onRetry = vi.fn();
    renderView({ report: null, error: "database unavailable", onRetry });

    expect(screen.getByRole("alert")).toHaveTextContent("database unavailable");
    await user.click(screen.getByRole("button", { name: "Retry" }));
    expect(onRetry).toHaveBeenCalledOnce();
  });

  it("shows an empty state for the selected scope", () => {
    renderView({ report: emptyReport(), days: 7 });

    expect(screen.getByText("No review metrics in this scope.")).toBeInTheDocument();
    expect(screen.getAllByText("All repositories · Last 7 days").length).toBeGreaterThan(0);
  });

  it("renders contract metrics, filter scope, charts, and incomplete feedback", () => {
    renderView();

    expect(screen.getByRole("heading", { name: "Review effectiveness" })).toBeInTheDocument();
    expect(screen.getByText("Feedback coverage is incomplete")).toBeInTheDocument();
    expect(screen.getByText("48")).toBeInTheDocument();
    expect(screen.getByText("126")).toBeInTheDocument();
    expect(screen.getByText("57.1%")).toBeInTheDocument();
    expect(screen.getByText("7.1%")).toBeInTheDocument();
    expect(screen.getByText("58m")).toBeInTheDocument();
    expect(
      screen.getByRole("img", {
        name: /Findings by severity for All repositories · Last 30 days/,
      }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("img", {
        name: /Findings by category for All repositories · Last 30 days/,
      }),
    ).toBeInTheDocument();
    expect(screen.queryByText(/developer ranking/i)).not.toBeInTheDocument();
  });

  it("reports period and repository changes without deriving new metrics", async () => {
    const user = userEvent.setup();
    const onDaysChange = vi.fn();
    const onRepositoryChange = vi.fn();
    renderView({ onDaysChange, onRepositoryChange });

    await user.click(screen.getByRole("button", { name: "7d" }));
    await user.selectOptions(screen.getByRole("combobox", { name: "Repository" }), [
      "example-workspace/backend-api",
    ]);

    expect(onDaysChange).toHaveBeenCalledWith(7);
    expect(onRepositoryChange).toHaveBeenCalledWith(REPOSITORIES[1]);
  });
});
