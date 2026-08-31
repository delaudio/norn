import {
  ArrowLeft,
  CheckCircle,
  CircleNotch,
  Plus,
  Trash,
  WarningCircle,
} from "@phosphor-icons/react";
import { useState } from "react";
import { ReviewTerminalPicker } from "@/components/review/ReviewTerminalPicker";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { AUTOMATIC_SYNC_INTERVAL_OPTIONS } from "@/hooks/useAutomaticSyncPolling";
import type { ConnectionUser } from "@/hooks/useCredentials";
import { cn } from "@/lib/utils";
import type {
  AiProvider,
  AutomaticSyncIntervalSeconds,
  ClaudeReviewEffort,
  ClaudeReviewModel,
  CodexReviewEffort,
  DiffViewMode,
  RepoRef,
  ReviewProvider,
  ReviewTerminal,
  ReviewTerminalOption,
} from "@/types";

type TestState =
  | { status: "idle" }
  | { status: "testing" }
  | { status: "ok"; name: string }
  | { status: "error"; message: string };

function repoRowKey(repo: RepoRef, index: number): string {
  return `${repo.provider ?? "bitbucket"}:${repo.workspace}:${repo.repo}:${repo.localPath ?? ""}:${index}`;
}

function repoWithProvider(repo: RepoRef): RepoRef {
  return { ...repo, provider: repo.provider ?? "bitbucket" };
}

export interface SettingsSaveInput {
  repos: RepoRef[];
  reviewProvider: ReviewProvider;
  defaultDiffView: DiffViewMode;
  reviewTerminal: ReviewTerminal | null;
  aiProvider: AiProvider;
  claudeModel: ClaudeReviewModel | null;
  claudeEffort: ClaudeReviewEffort | null;
  codexModel: string | null;
  codexEffort: CodexReviewEffort | null;
  jiraBaseUrl: string | null;
  automaticSyncIntervalSeconds: AutomaticSyncIntervalSeconds | null;
  menuBarSyncEnabled: boolean;
  notificationsEnabled: boolean;
  username: string;
  token: string;
  githubToken: string;
  jiraToken: string;
  notionToken: string;
}

export interface SettingsFormProps {
  repos: RepoRef[];
  reviewProvider: ReviewProvider;
  defaultDiffView: DiffViewMode;
  reviewTerminal: ReviewTerminal | null;
  aiProvider: AiProvider;
  claudeModel: ClaudeReviewModel | null;
  claudeEffort: ClaudeReviewEffort | null;
  codexModel: string | null;
  codexEffort: CodexReviewEffort | null;
  reviewTerminalOptions: ReviewTerminalOption[];
  jiraBaseUrl: string | null;
  automaticSyncIntervalSeconds: AutomaticSyncIntervalSeconds | null;
  menuBarSyncEnabled: boolean;
  notificationsEnabled: boolean;
  hasCredentials: boolean;
  hasGithubCredentials: boolean;
  hasJira: boolean;
  hasNotion: boolean;
  onTestConnection: (
    provider: ReviewProvider,
    username: string,
    token: string,
  ) => Promise<ConnectionUser>;
  onSave: (input: SettingsSaveInput) => Promise<void>;
  onCancel?: () => void;
  onSaved?: () => void;
  layout?: "inline" | "page";
}

export interface SettingsDialogProps extends Omit<SettingsFormProps, "onCancel" | "onSaved"> {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

export interface SettingsPageProps extends SettingsFormProps {
  onBack: () => void;
}

function SettingsForm({
  repos: initialRepos,
  reviewProvider: initialReviewProvider,
  defaultDiffView: initialDiffView,
  reviewTerminal: initialReviewTerminal,
  aiProvider: initialAiProvider,
  claudeModel: initialClaudeModel,
  claudeEffort: initialClaudeEffort,
  codexModel: initialCodexModel,
  codexEffort: initialCodexEffort,
  reviewTerminalOptions,
  jiraBaseUrl: initialJiraBaseUrl,
  automaticSyncIntervalSeconds: initialAutomaticSyncIntervalSeconds,
  menuBarSyncEnabled: initialMenuBarSyncEnabled,
  notificationsEnabled: initialNotificationsEnabled,
  hasCredentials,
  hasGithubCredentials,
  hasJira,
  hasNotion,
  onTestConnection,
  onSave,
  onCancel,
  onSaved,
  layout = "inline",
}: SettingsFormProps) {
  const [repos, setRepos] = useState<RepoRef[]>(
    initialRepos.length > 0
      ? initialRepos.map(repoWithProvider)
      : [{ provider: initialReviewProvider, workspace: "", repo: "" }],
  );
  const [reviewProvider, setReviewProvider] = useState<ReviewProvider>(initialReviewProvider);
  const [username, setUsername] = useState("");
  const [token, setToken] = useState("");
  const [githubToken, setGithubToken] = useState("");
  const [diffView, setDiffView] = useState<DiffViewMode>(initialDiffView);
  const [reviewTerminal, setReviewTerminal] = useState<ReviewTerminal | null>(
    initialReviewTerminal,
  );
  const [aiProvider, setAiProvider] = useState<AiProvider>(initialAiProvider);
  const [claudeModel, setClaudeModel] = useState<ClaudeReviewModel | null>(initialClaudeModel);
  const [claudeEffort, setClaudeEffort] = useState<ClaudeReviewEffort | null>(initialClaudeEffort);
  const [codexModel, setCodexModel] = useState(initialCodexModel ?? "");
  const [codexEffort, setCodexEffort] = useState<CodexReviewEffort | null>(initialCodexEffort);
  const [jiraBaseUrl, setJiraBaseUrl] = useState(initialJiraBaseUrl ?? "");
  const [automaticSyncIntervalSeconds, setAutomaticSyncIntervalSeconds] =
    useState<AutomaticSyncIntervalSeconds | null>(initialAutomaticSyncIntervalSeconds);
  const [menuBarSyncEnabled, setMenuBarSyncEnabled] = useState(initialMenuBarSyncEnabled);
  const [notificationsEnabled, setNotificationsEnabled] = useState(initialNotificationsEnabled);
  const [jiraToken, setJiraToken] = useState("");
  const [notionToken, setNotionToken] = useState("");
  const [test, setTest] = useState<TestState>({ status: "idle" });
  const [saving, setSaving] = useState(false);

  const canTestBitbucket = username.trim().length > 0 && token.trim().length > 0;
  const canTestGithub = githubToken.trim().length > 0;
  const visibleRepoRows = repos
    .map((repo, index) => ({ repo, index }))
    .filter(({ repo }) => (repo.provider ?? "bitbucket") === reviewProvider);
  const repositoryPanelTitle =
    reviewProvider === "github" ? "GitHub repositories" : "Bitbucket repositories";
  const repositoryOwnerLabel = reviewProvider === "github" ? "Owner / organization" : "Workspace";

  function updateRepo(index: number, patch: Partial<RepoRef>) {
    setRepos((prev) => prev.map((r, i) => (i === index ? { ...r, ...patch } : r)));
  }

  async function handleTest() {
    if (reviewProvider === "bitbucket" && !canTestBitbucket) return;
    if (reviewProvider === "github" && !canTestGithub) return;
    setTest({ status: "testing" });
    try {
      const user =
        reviewProvider === "github"
          ? await onTestConnection("github", "", githubToken.trim())
          : await onTestConnection("bitbucket", username.trim(), token.trim());
      setTest({ status: "ok", name: user.displayName || "connected" });
    } catch (e) {
      setTest({ status: "error", message: e instanceof Error ? e.message : String(e) });
    }
  }

  async function handleSave() {
    setSaving(true);
    try {
      const cleaned: RepoRef[] = [];
      for (const repo of repos) {
        const workspace = repo.workspace.trim();
        const repoName = repo.repo.trim();
        if (!workspace || !repoName) continue;
        cleaned.push({
          provider: repo.provider ?? "bitbucket",
          workspace,
          repo: repoName,
          localPath: repo.localPath?.trim() || null,
        });
      }
      await onSave({
        repos: cleaned,
        reviewProvider,
        defaultDiffView: diffView,
        reviewTerminal,
        aiProvider,
        claudeModel,
        claudeEffort,
        codexModel: codexModel.trim() || null,
        codexEffort,
        jiraBaseUrl: jiraBaseUrl.trim() || null,
        automaticSyncIntervalSeconds,
        menuBarSyncEnabled,
        notificationsEnabled,
        username: username.trim(),
        token: token.trim(),
        githubToken: githubToken.trim(),
        jiraToken: jiraToken.trim(),
        notionToken: notionToken.trim(),
      });
      onSaved?.();
    } catch (e) {
      setTest({ status: "error", message: e instanceof Error ? e.message : String(e) });
    } finally {
      setSaving(false);
    }
  }

  const fields = (
    <div className="grid gap-4">
      <div className="grid gap-1.5 md:max-w-xs">
        <Label htmlFor="settings-review-provider">Review provider</Label>
        <select
          id="settings-review-provider"
          className="h-9 rounded-md border border-input bg-background px-3 text-sm text-foreground"
          value={reviewProvider}
          onChange={(event) => setReviewProvider(event.target.value as ReviewProvider)}
        >
          <option value="bitbucket">Bitbucket</option>
          <option value="github">GitHub</option>
        </select>
      </div>

      <div className="grid gap-1.5">
        <div className="flex items-center justify-between gap-3">
          <div>
            <Label>{repositoryPanelTitle}</Label>
            <p className="mt-1 text-xs text-muted-foreground">
              {reviewProvider === "github"
                ? "Configure GitHub owner or organization repositories. Bitbucket repositories stay in their own panel."
                : "Configure Bitbucket workspace repositories. GitHub repositories stay in their own panel."}
            </p>
          </div>
          <Button
            variant="outline"
            size="sm"
            onClick={() =>
              setRepos((prev) => [...prev, { provider: reviewProvider, workspace: "", repo: "" }])
            }
          >
            <Plus size={14} /> Add row
          </Button>
        </div>
        <div className="overflow-hidden rounded-md border border-border">
          <table className="w-full border-collapse text-left text-sm">
            <thead className="bg-muted/50 text-xs uppercase tracking-wide text-muted-foreground">
              <tr>
                <th className="w-[24%] px-3 py-2 font-medium">{repositoryOwnerLabel}</th>
                <th className="w-[24%] px-3 py-2 font-medium">Repository</th>
                <th className="px-3 py-2 font-medium">Local path</th>
                <th className="w-[92px] px-3 py-2 text-right font-medium">Actions</th>
              </tr>
            </thead>
            <tbody>
              {visibleRepoRows.map(({ repo: r, index: originalIndex }, visibleIndex) => (
                <tr key={repoRowKey(r, originalIndex)} className="border-t border-border align-top">
                  <td className="px-3 py-2">
                    <Input
                      aria-label={`${repositoryOwnerLabel} ${visibleIndex + 1}`}
                      value={r.workspace}
                      onChange={(e) => updateRepo(originalIndex, { workspace: e.target.value })}
                      placeholder={reviewProvider === "github" ? "owner or org" : "workspace"}
                    />
                  </td>
                  <td className="px-3 py-2">
                    <Input
                      aria-label={`Repository ${visibleIndex + 1}`}
                      value={r.repo}
                      onChange={(e) => updateRepo(originalIndex, { repo: e.target.value })}
                      placeholder="repository"
                    />
                  </td>
                  <td className="px-3 py-2">
                    <Input
                      aria-label={`Local path ${visibleIndex + 1}`}
                      value={r.localPath ?? ""}
                      onChange={(e) => updateRepo(originalIndex, { localPath: e.target.value })}
                      placeholder="/absolute/path/to/local/clone"
                    />
                  </td>
                  <td className="px-3 py-2 text-right">
                    <Button
                      variant="ghost"
                      size="icon"
                      aria-label={`Remove repository ${visibleIndex + 1}`}
                      onClick={() =>
                        setRepos((prev) => prev.filter((_, idx) => idx !== originalIndex))
                      }
                    >
                      <Trash size={14} />
                    </Button>
                  </td>
                </tr>
              ))}
              {visibleRepoRows.length === 0 && (
                <tr className="border-t border-border">
                  <td colSpan={4} className="px-3 py-4 text-sm text-muted-foreground">
                    No {reviewProvider === "github" ? "GitHub" : "Bitbucket"} repositories
                    configured.
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </div>
      </div>

      <div className="grid gap-3 rounded-md border border-border p-3">
        <div className="grid gap-1.5">
          <Label>Provider credentials</Label>
          <p className="text-xs text-muted-foreground">
            Bitbucket and GitHub tokens are stored separately in the local keychain.
          </p>
        </div>
        <div className="grid gap-3 md:grid-cols-2">
          <div className="grid gap-1.5">
            <Label htmlFor="settings-username">Atlassian email</Label>
            <Input
              id="settings-username"
              type="email"
              value={username}
              onChange={(e) => setUsername(e.target.value)}
              placeholder="you@example.com"
              autoComplete="off"
            />
          </div>

          <div className="grid gap-1.5">
            <Label htmlFor="settings-token">Bitbucket API token</Label>
            <Input
              id="settings-token"
              type="password"
              value={token}
              onChange={(e) => setToken(e.target.value)}
              placeholder={hasCredentials ? "•••••••• (leave blank to keep)" : "ATATT..."}
              autoComplete="off"
            />
          </div>
        </div>
        <div className="grid gap-1.5 md:max-w-md">
          <Label htmlFor="settings-github-token">GitHub token</Label>
          <Input
            id="settings-github-token"
            type="password"
            value={githubToken}
            onChange={(e) => setGithubToken(e.target.value)}
            placeholder={hasGithubCredentials ? "•••••••• (leave blank to keep)" : "github_pat_..."}
            autoComplete="off"
          />
        </div>
      </div>

      <div className="grid gap-1.5">
        <Label>Default diff view</Label>
        <div className="flex w-fit gap-1 rounded-md border border-border p-0.5">
          {(["unified", "split", "conversation"] as DiffViewMode[]).map((mode) => (
            <button
              key={mode}
              type="button"
              onClick={() => setDiffView(mode)}
              className={cn(
                "rounded px-3 py-1 text-xs font-medium capitalize transition-colors",
                diffView === mode
                  ? "bg-secondary text-secondary-foreground"
                  : "text-muted-foreground hover:bg-muted",
              )}
            >
              {mode}
            </button>
          ))}
        </div>
      </div>

      <div className="grid gap-1.5">
        <Label>Review with Claude terminal</Label>
        <ReviewTerminalPicker
          terminals={reviewTerminalOptions}
          value={reviewTerminal}
          onChange={setReviewTerminal}
          allowUnset
        />
      </div>

      <div className="grid gap-3 rounded-md border border-border p-3">
        <div className="grid gap-1.5 md:max-w-xs">
          <Label htmlFor="settings-ai-provider">AI review provider</Label>
          <select
            id="settings-ai-provider"
            className="h-9 rounded-md border border-input bg-background px-3 text-sm text-foreground"
            value={aiProvider}
            onChange={(event) => setAiProvider(event.target.value as AiProvider)}
          >
            <option value="claude">Claude</option>
            <option value="codex">Codex</option>
          </select>
        </div>
        {aiProvider === "claude" ? (
          <div className="grid gap-3 md:grid-cols-2">
            <div className="grid gap-1.5">
              <Label htmlFor="settings-claude-model">Claude review model</Label>
              <select
                id="settings-claude-model"
                className="h-9 rounded-md border border-input bg-background px-3 text-sm text-foreground"
                value={claudeModel ?? ""}
                onChange={(event) =>
                  setClaudeModel((event.target.value || null) as ClaudeReviewModel | null)
                }
              >
                <option value="">Default</option>
                <option value="sonnet">Sonnet</option>
                <option value="opus">Opus</option>
                <option value="fable">Fable</option>
              </select>
              <p className="text-xs text-muted-foreground">
                Passed to Claude Code as <span className="font-mono">--model</span> for inline AI
                reviews.
              </p>
            </div>
            <div className="grid gap-1.5">
              <Label htmlFor="settings-claude-effort">Claude review effort</Label>
              <select
                id="settings-claude-effort"
                className="h-9 rounded-md border border-input bg-background px-3 text-sm text-foreground"
                value={claudeEffort ?? ""}
                onChange={(event) =>
                  setClaudeEffort((event.target.value || null) as ClaudeReviewEffort | null)
                }
              >
                <option value="">Default</option>
                <option value="low">Low</option>
                <option value="medium">Medium</option>
                <option value="high">High</option>
                <option value="xhigh">XHigh</option>
                <option value="max">Max</option>
              </select>
              <p className="text-xs text-muted-foreground">
                Passed to Claude Code as <span className="font-mono">--effort</span> for inline AI
                reviews.
              </p>
            </div>
          </div>
        ) : (
          <div className="grid gap-3 md:grid-cols-2">
            <div className="grid gap-1.5">
              <Label htmlFor="settings-codex-model">Codex review model</Label>
              <select
                id="settings-codex-model"
                className="h-9 rounded-md border border-input bg-background px-3 text-sm text-foreground"
                value={codexModel}
                onChange={(event) => setCodexModel(event.target.value)}
              >
                <option value="">Default</option>
                <option value="gpt-5.4">gpt-5.4</option>
                <option value="gpt-5.5">gpt-5.5</option>
              </select>
              <p className="text-xs text-muted-foreground">
                Passed to Codex as <span className="font-mono">--model</span> when set.
              </p>
            </div>
            <div className="grid gap-1.5">
              <Label htmlFor="settings-codex-effort">Codex reasoning effort</Label>
              <select
                id="settings-codex-effort"
                className="h-9 rounded-md border border-input bg-background px-3 text-sm text-foreground"
                value={codexEffort ?? ""}
                onChange={(event) =>
                  setCodexEffort((event.target.value || null) as CodexReviewEffort | null)
                }
              >
                <option value="">Default</option>
                <option value="low">Low</option>
                <option value="medium">Medium</option>
                <option value="high">High</option>
              </select>
              <p className="text-xs text-muted-foreground">
                Passed to Codex as <span className="font-mono">model_reasoning_effort</span>.
              </p>
            </div>
          </div>
        )}
      </div>

      <div className="grid gap-2 rounded-md border border-border p-3">
        <div className="grid gap-1.5 md:max-w-xs">
          <Label htmlFor="settings-automatic-sync">Automatic sync</Label>
          <select
            id="settings-automatic-sync"
            className="h-9 rounded-md border border-input bg-background px-3 text-sm text-foreground"
            value={automaticSyncIntervalSeconds ?? ""}
            onChange={(event) => {
              const value = event.target.value;
              setAutomaticSyncIntervalSeconds(
                value === "" ? null : (Number(value) as AutomaticSyncIntervalSeconds),
              );
            }}
          >
            {AUTOMATIC_SYNC_INTERVAL_OPTIONS.map((option) => (
              <option key={option.value ?? "off"} value={option.value ?? ""}>
                {option.label}
              </option>
            ))}
          </select>
        </div>

        <Label>Menu bar</Label>
        <label className="flex items-start gap-2 text-sm">
          <input
            type="checkbox"
            className="mt-1"
            checked={menuBarSyncEnabled}
            onChange={(e) => setMenuBarSyncEnabled(e.target.checked)}
          />
          <span>
            Enable menu bar sync
            <span className="block text-xs text-muted-foreground">
              Shows the latest loaded pull requests in the macOS menu bar and lets you refresh them
              from there.
            </span>
          </span>
        </label>
        <label className="flex items-start gap-2 text-sm">
          <input
            type="checkbox"
            className="mt-1"
            checked={notificationsEnabled}
            onChange={(e) => setNotificationsEnabled(e.target.checked)}
          />
          <span>
            Enable native notifications
            <span className="block text-xs text-muted-foreground">
              Notifies about new or updated pull requests after the initial snapshot.
            </span>
          </span>
        </label>
      </div>

      <div className="grid gap-1.5">
        <Label htmlFor="settings-jira">Jira site URL</Label>
        <Input
          id="settings-jira"
          value={jiraBaseUrl}
          onChange={(e) => setJiraBaseUrl(e.target.value)}
          placeholder="https://example.atlassian.net"
        />
        <p className="text-xs text-muted-foreground">
          Used to link PR issue keys (APP-1234) and add Jira context to AI reviews. Leave empty to
          disable.
        </p>
      </div>

      <div className="grid grid-cols-1 gap-3 md:grid-cols-2">
        <div className="grid gap-1.5">
          <Label htmlFor="settings-jira-token">Jira API token</Label>
          <Input
            id="settings-jira-token"
            type="password"
            value={jiraToken}
            onChange={(e) => setJiraToken(e.target.value)}
            placeholder={hasJira ? "•••••••• (leave blank to keep)" : "optional — inlines ticket"}
            autoComplete="off"
          />
        </div>
        <div className="grid gap-1.5">
          <Label htmlFor="settings-notion-token">Notion token</Label>
          <Input
            id="settings-notion-token"
            type="password"
            value={notionToken}
            onChange={(e) => setNotionToken(e.target.value)}
            placeholder={hasNotion ? "•••••••• (leave blank to keep)" : "optional — inlines docs"}
            autoComplete="off"
          />
        </div>
      </div>

      <div className="flex items-center gap-2">
        <Button
          variant="secondary"
          size="sm"
          onClick={handleTest}
          disabled={
            test.status === "testing" ||
            (reviewProvider === "github" ? !canTestGithub : !canTestBitbucket)
          }
        >
          {test.status === "testing" && <CircleNotch size={14} className="animate-spin" />}
          Test {reviewProvider === "github" ? "GitHub" : "Bitbucket"} connection
        </Button>
        <TestStatus test={test} />
      </div>
    </div>
  );

  const actions = (
    <div
      className={cn(
        "flex justify-end gap-2",
        layout === "page"
          ? "shrink-0 border-t border-border bg-background px-4 py-3"
          : "mt-5 border-t border-border pt-4",
      )}
    >
      <div
        className={cn("flex gap-2", layout === "page" && "mx-auto w-full max-w-7xl justify-end")}
      >
        {onCancel && (
          <Button variant="ghost" onClick={onCancel}>
            Cancel
          </Button>
        )}
        <Button onClick={handleSave} disabled={saving}>
          {saving && <CircleNotch size={14} className="animate-spin" />}
          Save
        </Button>
      </div>
    </div>
  );

  if (layout === "page") {
    return (
      <div className="flex h-full min-h-0 flex-col">
        <div className="min-h-0 flex-1 overflow-y-auto px-4 py-4">
          <div className="mx-auto w-full max-w-7xl">{fields}</div>
        </div>
        {actions}
      </div>
    );
  }

  return (
    <>
      {fields}
      {actions}
    </>
  );
}

export function SettingsPage({ onBack, ...props }: SettingsPageProps) {
  const formKey = [
    props.reviewProvider,
    props.repos
      .map(
        (repo) =>
          `${repo.provider ?? "bitbucket"}:${repo.workspace}/${repo.repo}/${repo.localPath ?? ""}`,
      )
      .join("|"),
    props.defaultDiffView,
    props.reviewTerminal ?? "",
    props.aiProvider,
    props.claudeModel ?? "",
    props.claudeEffort ?? "",
    props.codexModel ?? "",
    props.codexEffort ?? "",
    props.jiraBaseUrl ?? "",
    props.automaticSyncIntervalSeconds ?? "sync-off",
    props.menuBarSyncEnabled ? "menu-on" : "menu-off",
    props.notificationsEnabled ? "notifications-on" : "notifications-off",
  ].join("::");

  return (
    <div className="flex h-full flex-col overflow-hidden bg-background">
      <div className="flex shrink-0 items-center gap-2 border-b border-border px-4 py-2.5">
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
          <span className="text-sm font-semibold">Settings</span>
          <span className="ml-2 text-xs text-muted-foreground">
            Providers, repositories, credentials, and review defaults
          </span>
        </div>
      </div>
      <div className="min-h-0 flex-1">
        <SettingsForm key={formKey} {...props} layout="page" onCancel={onBack} onSaved={onBack} />
      </div>
    </div>
  );
}

export function SettingsDialog({ open, onOpenChange, ...props }: SettingsDialogProps) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-h-[85vh] overflow-auto">
        <DialogHeader>
          <DialogTitle>Settings</DialogTitle>
          <DialogDescription>
            Track Bitbucket Cloud and GitHub repositories with separate local credentials.
          </DialogDescription>
        </DialogHeader>
        {open && (
          <SettingsForm
            {...props}
            onCancel={() => onOpenChange(false)}
            onSaved={() => onOpenChange(false)}
          />
        )}
        <DialogFooter className="hidden" />
      </DialogContent>
    </Dialog>
  );
}

function TestStatus({ test }: { test: TestState }) {
  if (test.status === "ok") {
    return (
      <span className="flex items-center gap-1 text-xs text-[var(--success)]">
        <CheckCircle size={14} weight="fill" /> Connected as {test.name}
      </span>
    );
  }
  if (test.status === "error") {
    return (
      <span className="flex items-center gap-1 text-xs text-destructive" title={test.message}>
        <WarningCircle size={14} weight="fill" /> Connection failed
      </span>
    );
  }
  return null;
}
