import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { PrDetailPanel } from "@/components/pr-detail/PrDetailPanel";
import { PrSidebar } from "@/components/pr-sidebar/PrSidebar";
import { ThemeToggle } from "@/components/ThemeToggle";
import type { NewDraft } from "@/hooks/useDraftComments";
import type { PrGroup } from "@/hooks/usePullRequests";
import { clonePullRequests } from "@/storybook/bitbucket.fixtures";
import { type PrListFilter, repoKey } from "@/types";
import { AppShell } from "./AppShell";

const meta = {
  title: "App/AppShell",
  component: AppShell,
  parameters: { layout: "fullscreen" },
} satisfies Meta<typeof AppShell>;

export default meta;
type Story = StoryObj<typeof meta>;

function buildGroups(): PrGroup[] {
  const base = clonePullRequests();
  return [
    {
      repo: { workspace: "example-workspace", repo: "frontend-app" },
      pullRequests: base.map((p) => ({ ...p, repo: "frontend-app" })),
      page: 1,
      hasNext: true,
      loadingMore: false,
      error: null,
    },
    {
      repo: { workspace: "example-workspace", repo: "backend-api" },
      pullRequests: base.slice(0, 2).map((p) => ({
        ...p,
        id: p.id + 1000,
        repo: "backend-api",
      })),
      page: 1,
      hasNext: false,
      loadingMore: false,
      error: null,
    },
  ];
}

function AppShellHarness() {
  const groups = buildGroups();
  const [theme, setTheme] = useState<"light" | "dark">("dark");
  const [filter, setFilter] = useState<PrListFilter>("OPEN");
  const [authorFilter, setAuthorFilter] = useState<string | null>(null);
  const [repositoryFilter, setRepositoryFilter] = useState<string | null>(null);
  const [active, setActive] = useState<{ workspace: string; repo: string; prId: number } | null>(
    null,
  );

  const authors = Array.from(
    new Map(
      groups
        .flatMap((g) => g.pullRequests)
        .map((pr) => [
          pr.authorDisplayName,
          {
            key: pr.authorDisplayName,
            label: pr.authorDisplayName,
            isMe: pr.authorDisplayName === "Alex Reviewer",
          },
        ]),
    ).values(),
  );
  const repositories = groups.map((group) => ({
    key: repoKey(group.repo),
    label: `${group.repo.workspace}/${group.repo.repo}`,
    count: group.pullRequests.length,
  }));
  const displayed = groups
    .filter((group) => repositoryFilter == null || repoKey(group.repo) === repositoryFilter)
    .map((group) => ({
      ...group,
      pullRequests:
        authorFilter == null
          ? group.pullRequests
          : group.pullRequests.filter((pr) => pr.authorDisplayName === authorFilter),
    }));

  return (
    <div style={{ height: "100vh" }}>
      <AppShell
        headerRight={
          <ThemeToggle
            theme={theme}
            onToggle={() => setTheme((t) => (t === "dark" ? "light" : "dark"))}
          />
        }
        sidebar={
          <PrSidebar
            groups={displayed}
            filter={filter}
            active={active}
            authors={authors}
            authorFilter={authorFilter}
            repositories={repositories}
            repositoryFilter={repositoryFilter}
            onFilterChange={setFilter}
            onAuthorFilterChange={setAuthorFilter}
            onRepositoryFilterChange={setRepositoryFilter}
            onSelect={(pr) => setActive({ workspace: pr.workspace, repo: pr.repo, prId: pr.id })}
            onLoadMore={() => {}}
            onRefresh={() => {}}
            onOpenSettings={() => {}}
          />
        }
        main={
          <PrDetailPanel
            provider="bitbucket"
            workspace={active?.workspace ?? null}
            repo={active?.repo ?? null}
            prId={active?.prId ?? null}
            currentUserDisplayName="Alex Reviewer"
            jiraBaseUrl="https://example.atlassian.net"
            jiraContextEnabled={false}
            availablePullRequests={[]}
            availableRepositories={[]}
            reviewReferences={[]}
            addReviewReference={() => {}}
            updateReviewReference={() => {}}
            removeReviewReference={() => {}}
            drafts={[]}
            publishing={false}
            publishingDraftId={null}
            addDraft={(_draft: NewDraft) => {}}
            updateDraft={() => {}}
            removeDraft={() => {}}
            discardAll={() => {}}
            publishDraft={async () => ({ draft: null, comment: null, error: null })}
            publishAll={async () => ({ published: 0, failed: [], errors: [] })}
          />
        }
      />
    </div>
  );
}

export const Default: Story = {
  args: { sidebar: null, main: null },
  render: () => <AppShellHarness />,
};
