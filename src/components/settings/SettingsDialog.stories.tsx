import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { Button } from "@/components/ui/button";
import type { RepoRef, ReviewProvider, ReviewTerminalOption } from "@/types";
import { SettingsDialog } from "./SettingsDialog";

const REVIEW_TERMINAL_OPTIONS: ReviewTerminalOption[] = [
  { id: "wezterm", label: "WezTerm", available: true },
  { id: "iterm", label: "iTerm2", available: true },
  { id: "terminal", label: "Terminal", available: true },
];

const okConnection = async (_provider: ReviewProvider, _u: string, _t: string) => {
  await new Promise((r) => setTimeout(r, 400));
  return { displayName: "Alex Reviewer" };
};

const failConnection = async () => {
  await new Promise((r) => setTimeout(r, 400));
  throw new Error("Bitbucket API error 401 Unauthorized");
};

const SAMPLE_REPOS: RepoRef[] = [
  { provider: "bitbucket", workspace: "example-workspace", repo: "frontend-app" },
  { provider: "bitbucket", workspace: "example-workspace", repo: "backend-api" },
  { provider: "github", workspace: "delaudio", repo: "norn" },
];

const meta = {
  title: "Settings/SettingsDialog",
  component: SettingsDialog,
  parameters: { layout: "centered" },
  args: {
    open: true,
    onOpenChange: () => {},
    repos: SAMPLE_REPOS,
    reviewProvider: "bitbucket",
    defaultDiffView: "unified",
    reviewTerminal: "wezterm",
    aiProvider: "claude",
    claudeModel: "sonnet",
    claudeEffort: "high",
    codexModel: null,
    codexEffort: null,
    reviewTerminalOptions: REVIEW_TERMINAL_OPTIONS,
    jiraBaseUrl: "https://example.atlassian.net",
    automaticSyncIntervalSeconds: null,
    menuBarSyncEnabled: true,
    notificationsEnabled: false,
    hasCredentials: false,
    hasGithubCredentials: false,
    hasJira: false,
    hasNotion: false,
    onTestConnection: okConnection,
    onSave: async () => {
      await new Promise((r) => setTimeout(r, 200));
    },
  },
} satisfies Meta<typeof SettingsDialog>;

export default meta;
type Story = StoryObj<typeof meta>;

function Harness({
  testConnection,
  repos,
  hasCredentials,
}: {
  testConnection: (
    provider: ReviewProvider,
    u: string,
    t: string,
  ) => Promise<{ displayName: string }>;
  repos: RepoRef[];
  hasCredentials: boolean;
}) {
  const [open, setOpen] = useState(true);
  return (
    <div>
      <Button onClick={() => setOpen(true)}>Open settings</Button>
      <SettingsDialog
        open={open}
        onOpenChange={setOpen}
        repos={repos}
        reviewProvider="bitbucket"
        defaultDiffView="unified"
        reviewTerminal={null}
        aiProvider="claude"
        claudeModel={null}
        claudeEffort={null}
        codexModel={null}
        codexEffort={null}
        reviewTerminalOptions={REVIEW_TERMINAL_OPTIONS}
        jiraBaseUrl="https://example.atlassian.net"
        automaticSyncIntervalSeconds={null}
        menuBarSyncEnabled={true}
        notificationsEnabled={false}
        hasCredentials={hasCredentials}
        hasGithubCredentials={false}
        hasJira={false}
        hasNotion={false}
        onTestConnection={testConnection}
        onSave={async () => {
          await new Promise((r) => setTimeout(r, 200));
        }}
      />
    </div>
  );
}

export const MultipleRepos: Story = {
  render: () => <Harness testConnection={okConnection} repos={SAMPLE_REPOS} hasCredentials />,
};

export const FirstRun: Story = {
  render: () => <Harness testConnection={okConnection} repos={[]} hasCredentials={false} />,
};

export const ConnectionFails: Story = {
  render: () => (
    <Harness testConnection={failConnection} repos={SAMPLE_REPOS} hasCredentials={false} />
  ),
};
