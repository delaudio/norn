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
import type { PullRequestSummary } from "@/types";

interface PrByRepoChartProps {
  pullRequests: PullRequestSummary[];
}

const COLORS = [
  "var(--primary)",
  "hsl(173 58% 39%)",
  "hsl(197 37% 24%)",
  "hsl(43 74% 66%)",
  "hsl(27 87% 67%)",
];

export function PrByRepoChart({ pullRequests }: PrByRepoChartProps) {
  const counts = new Map<string, number>();
  for (const pr of pullRequests) {
    const key = pr.repo;
    counts.set(key, (counts.get(key) ?? 0) + 1);
  }
  const data = [...counts.entries()]
    .sort((a, b) => b[1] - a[1])
    .map(([repo, count]) => ({ repo, count }));

  if (data.length === 0) {
    return (
      <div className="flex flex-col gap-2 rounded-lg border border-border bg-card p-4 shadow-sm">
        <div className="text-sm font-semibold">PRs by repo</div>
        <div className="flex h-[160px] items-center justify-center text-sm text-muted-foreground">
          No open PRs
        </div>
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-2 rounded-lg border border-border bg-card p-4 shadow-sm">
      <div className="text-sm font-semibold">PRs by repo</div>
      <ResponsiveContainer width="100%" height={160}>
        <BarChart data={data} layout="vertical" barSize={20}>
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
            dataKey="repo"
            width={120}
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
              fontSize: 12,
              color: "var(--popover-foreground)",
            }}
            formatter={(value) => [value, "PRs"]}
          />
          <Bar dataKey="count" radius={[0, 4, 4, 0]}>
            {data.map(({ repo }, index) => (
              <Cell key={repo} fill={COLORS[index % COLORS.length]} />
            ))}
          </Bar>
        </BarChart>
      </ResponsiveContainer>
    </div>
  );
}
