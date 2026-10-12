import type {
  ProviderApprovalPreview,
  ProviderApprovalResult,
  ProviderCurrentUser,
  ProviderPrDetail,
  ProviderPrListParams,
  ProviderPrSummary,
  PrRepositoryIdentity,
} from "../../../src/protocol/generated/protocol";
import type { BackendClient, RepositorySummary } from "../transport/backendClient";
import { validPublicationDTO } from "../transport/publicationValidation";
import type { PickerEntry } from "./store";

export const prFilters = ["OPEN", "DRAFT", "MERGED", "ALL"] as const;
export type ProviderFilter = (typeof prFilters)[number];
export type ProviderFocus = "repository" | "state" | "author" | "query" | "list";
const querySegmenter = new Intl.Segmenter();
const PAGE_SIZE = 30;
const MAX_GROUPS = 100;
const MAX_RESULTS = 5000;
const MAX_BYTES = 8 * 1024 * 1024;
export const repositoryKey = (repo: PrRepositoryIdentity) =>
  JSON.stringify([repo.provider, repo.workspace, repo.repo]);
export const providerPrKey = (repo: PrRepositoryIdentity, id: number) =>
  JSON.stringify([repo.provider, repo.workspace, repo.repo, id]);

export interface ProviderGroup {
  repository: PrRepositoryIdentity;
  values: ProviderPrSummary[];
  page: number;
  nextPage: number | null;
  loading: boolean;
  error: string | null;
  collapsed: boolean;
}
export type ProviderRow =
  | { kind: "repository"; key: string; group: ProviderGroup }
  | { kind: "pr"; key: string; group: ProviderGroup; pr: ProviderPrSummary }
  | { kind: "page"; key: string; group: ProviderGroup };
export interface ProviderAuthor {
  key: string;
  provider: PrRepositoryIdentity["provider"];
  id: string;
  label: string;
  me: boolean;
}
export interface ProviderSnapshot {
  repositories: PrRepositoryIdentity[];
  repository: string | null;
  filter: ProviderFilter;
  author: ProviderAuthor | null;
  query: string;
  text: string;
  editing: boolean;
  focus: ProviderFocus;
  groups: ProviderGroup[];
  users: ProviderCurrentUser[];
  userErrors: string[];
  selected: string | null;
  detail: ProviderPrDetail | null;
  detailKey: string | null;
  detailOpen: boolean;
  detailLoading: boolean;
  browserOpening: boolean;
  approvalOpen: boolean;
  approvalBusy: boolean;
  approvalSending: boolean;
  approvalPreview: ProviderApprovalPreview | null;
  approvalResult: ProviderApprovalResult | null;
  approvalError: string | null;
  detailError: string | null;
  error: string | null;
  notice: string | null;
}
const initial = (): ProviderSnapshot => ({
  repositories: [],
  repository: null,
  filter: "OPEN",
  author: null,
  query: "",
  text: "",
  editing: false,
  focus: "list",
  groups: [],
  users: [],
  userErrors: [],
  selected: null,
  detail: null,
  detailKey: null,
  detailOpen: false,
  detailLoading: false,
  browserOpening: false,
  approvalOpen: false,
  approvalBusy: false,
  approvalSending: false,
  approvalPreview: null,
  approvalResult: null,
  approvalError: null,
  detailError: null,
  error: null,
  notice: null,
});
const message = (error: unknown) =>
  error instanceof Error ? error.message : "Provider data is unavailable.";
type Task = { generation: number; kind: "list" | "user" | "detail"; run: () => Promise<void> };

/** Transient provider browsing. Rust owns credentials/HTTP; no browser read writes. */
export class ProviderStore {
  private state = initial();
  private generation = 0;
  private detailGeneration = 0;
  private approvalGeneration = 0;
  private active = 0;
  private queue: Task[] = [];
  private bytes = new Map<string, number>();
  private authorCache = new Map<string, ProviderAuthor>();
  private listeners = new Set<() => void>();
  private rowGroups: ProviderGroup[] | null = null;
  private rowCache: ProviderRow[] = [];
  private rowIndexes = new Map<string, number>();
  notice(text: string) {
    this.update({ notice: text });
  }
  getSnapshot = () => this.state;
  subscribe = (listener: () => void) => {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  };
  private update(patch: Partial<ProviderSnapshot>) {
    this.state = { ...this.state, ...patch };
    for (const listener of this.listeners) listener();
  }
  dispose() {
    this.generation++;
    this.detailGeneration++;
    this.queue = [];
    this.bytes.clear();
    this.authorCache.clear();
    this.update(initial());
  }
  rows(): ProviderRow[] {
    if (this.rowGroups === this.state.groups) return this.rowCache;
    this.rowGroups = this.state.groups;
    this.rowCache = this.state.groups.flatMap((group) => {
      const key = repositoryKey(group.repository);
      const rows: ProviderRow[] = [{ kind: "repository", key: `repo:${key}`, group }];
      if (!group.collapsed) {
        rows.push(
          ...group.values.map((pr) => ({
            kind: "pr" as const,
            key: providerPrKey(group.repository, pr.id),
            group,
            pr,
          })),
        );
        if (group.nextPage !== null) rows.push({ kind: "page", key: `page:${key}`, group });
      }
      return rows;
    });
    this.rowIndexes = new Map(this.rowCache.map((row, i) => [row.key, i]));
    return this.rowCache;
  }
  selectedIndex() {
    this.rows();
    return this.rowIndexes.get(this.state.selected ?? "") ?? 0;
  }
  selectedRow() {
    return this.rows()[this.selectedIndex()];
  }
  authors(): ProviderAuthor[] {
    const values = new Map(this.authorCache);
    const me = new Set(
      this.state.users.map((user) => JSON.stringify([user.repository.provider, user.accountId])),
    );
    for (const group of this.state.groups)
      for (const pr of group.values) {
        if (!pr.authorAccountId) continue;
        const key = JSON.stringify([group.repository.provider, pr.authorAccountId]);
        values.set(key, {
          key,
          provider: group.repository.provider,
          id: pr.authorAccountId,
          label: pr.authorDisplayName || pr.authorAccountId,
          me: me.has(key),
        });
      }
    for (const user of this.state.users) {
      const key = JSON.stringify([user.repository.provider, user.accountId]);
      values.set(key, {
        key,
        provider: user.repository.provider,
        id: user.accountId,
        label: user.displayName || user.accountId,
        me: true,
      });
    }
    if (this.state.author) values.set(this.state.author.key, this.state.author);
    return [...values.values()].sort(
      (a, b) =>
        a.provider.localeCompare(b.provider) ||
        a.label.localeCompare(b.label) ||
        a.id.localeCompare(b.id),
    );
  }
  load(
    client: BackendClient,
    repositories: RepositorySummary[],
    picker: PickerEntry[],
    available = true,
  ) {
    this.dispose();
    if (!available) {
      this.update({
        error:
          "This backend does not support provider PR browsing. Install the matching UI/backend pair.",
      });
      return;
    }
    const scopes = new Map<string, PrRepositoryIdentity>();
    let invalid = 0;
    for (const source of [
      ...repositories,
      ...picker.filter((entry) => entry.prId === null || entry.prId < 0x80000000),
    ]) {
      const repository = {
        provider: source.provider,
        workspace: source.workspace,
        repo: source.repo,
      };
      if (!validPublicationDTO("PrRepositoryIdentity", repository)) {
        invalid++;
        continue;
      }
      scopes.set(repositoryKey(repository), repository);
    }
    const identities = [...scopes.values()];
    if (identities.length > 10000 || Buffer.byteLength(JSON.stringify(identities)) > MAX_BYTES) {
      this.update({
        error:
          "Repository inventory exceeds the browser limit; narrow the configured/saved scopes.",
      });
      return;
    }
    this.update({
      repositories: identities,
      notice: invalid
        ? `${invalid} repository identities cannot be browsed; inspect Settings or their saved scope.`
        : null,
    });
    this.refresh(client, false);
  }
  private enqueue(task: Task, priority = false) {
    if (priority) this.queue.unshift(task);
    else this.queue.push(task);
    this.pump();
  }
  private pump() {
    while (this.active < 2 && this.queue.length) {
      const task = this.queue.shift()!;
      if (task.generation !== this.generation) continue;
      this.active++;
      void task
        .run()
        .catch(() => {
          /* Each read reports only its current generation. */
        })
        .finally(() => {
          this.active--;
          this.pump();
        });
    }
  }
  refresh(client: BackendClient, preservePages = true) {
    this.clearApproval();
    const previous = new Map(
      this.state.groups.map((group) => [repositoryKey(group.repository), group]),
    );
    const selected = preservePages ? this.state.selected : null;
    const generation = ++this.generation;
    this.detailGeneration++;
    this.queue = [];
    this.bytes.clear();
    const repositories = this.state.repositories.filter(
      (repo) =>
        (this.state.repository === null || repositoryKey(repo) === this.state.repository) &&
        (!this.state.author || repo.provider === this.state.author.provider),
    );
    this.update({
      groups: [],
      selected,
      detail: null,
      detailKey: null,
      detailOpen: false,
      detailLoading: false,
      detailError: null,
      error: null,
    });
    if (repositories.length > MAX_GROUPS) {
      this.update({
        error: `All-repository browsing is limited to ${MAX_GROUPS} repositories. Select a repository to browse every scope.`,
      });
      return;
    }
    if (!repositories.length) {
      this.update({
        notice:
          "No repositories match this selection. Configure repositories in Settings or select All authors.",
      });
      return;
    }
    const groups = repositories.map((repository) => ({
      repository,
      values: [],
      page: 0,
      nextPage: null,
      loading: true,
      error: null,
      collapsed: previous.get(repositoryKey(repository))?.collapsed ?? false,
    }));
    this.update({ groups });
    const providers = new Set<string>();
    for (const repository of repositories) {
      if (
        !providers.has(repository.provider) &&
        !this.state.users.some((user) => user.repository.provider === repository.provider)
      ) {
        providers.add(repository.provider);
        this.enqueue({
          generation,
          kind: "user",
          run: async () => {
            try {
              const user = await client.providerUser(
                repository,
                () => generation === this.generation,
              );
              if (generation === this.generation)
                this.update({
                  userErrors: this.state.userErrors.filter(
                    (error) => !error.startsWith(`${repository.provider}:`),
                  ),
                  users: [
                    ...this.state.users.filter(
                      (u) => u.repository.provider !== repository.provider,
                    ),
                    user,
                  ],
                });
            } catch (error) {
              if (generation === this.generation)
                this.update({
                  userErrors: [
                    ...this.state.userErrors,
                    `${repository.provider}: ${message(error)}`,
                  ].slice(-2),
                });
            }
          },
        });
      }
      const desired = preservePages
        ? Math.max(1, previous.get(repositoryKey(repository))?.page ?? 1)
        : 1;
      this.schedulePage(client, repository, 1, desired, generation);
    }
  }
  private schedulePage(
    client: BackendClient,
    repository: PrRepositoryIdentity,
    page: number,
    desired: number,
    generation: number,
  ) {
    const filter = this.state.filter;
    const query = this.state.query;
    const authorId = this.state.author?.id ?? null;
    this.enqueue({
      generation,
      kind: "list",
      run: async () => {
        const key = repositoryKey(repository);
        try {
          const request: ProviderPrListParams = {
            repository,
            filter,
            page,
            limit: PAGE_SIZE,
            query: query || null,
            authorId,
          };
          const result = await client.providerPrs(request, () => generation === this.generation);
          if (generation !== this.generation) return;
          const group = this.state.groups.find((group) => repositoryKey(group.repository) === key);
          if (!group) return;
          const values = new Map(group.values.map((pr) => [pr.id, pr]));
          for (const pr of result.values) values.set(pr.id, pr);
          const merged = [...values.values()];
          const bytes = Buffer.byteLength(JSON.stringify(merged));
          const totalBytes = [...this.bytes.entries()].reduce(
            (sum, [k, n]) => sum + (k === key ? 0 : n),
            bytes,
          );
          const count = this.state.groups.reduce(
            (sum, g) => sum + (repositoryKey(g.repository) === key ? 0 : g.values.length),
            merged.length,
          );
          if (totalBytes > MAX_BYTES || count > MAX_RESULTS)
            throw new Error(
              "Loaded PR data reached its workspace limit. Narrow repository/author/query filters before loading more.",
            );
          for (const pr of result.values) {
            if (!pr.authorAccountId) continue;
            const authorKey = JSON.stringify([repository.provider, pr.authorAccountId]);
            if (this.authorCache.has(authorKey) || this.authorCache.size < MAX_RESULTS)
              this.authorCache.set(authorKey, {
                key: authorKey,
                provider: repository.provider,
                id: pr.authorAccountId,
                label: pr.authorDisplayName || pr.authorAccountId,
                me: false,
              });
          }
          this.bytes.set(key, bytes);
          const more = page < desired && result.nextPage !== null;
          this.update({
            groups: this.state.groups.map((g) =>
              repositoryKey(g.repository) === key
                ? {
                    ...g,
                    values: merged,
                    page,
                    nextPage: result.nextPage,
                    loading: more,
                    error: null,
                  }
                : g,
            ),
          });
          if (more) this.schedulePage(client, repository, result.nextPage!, desired, generation);
          this.ensureSelection();
        } catch (error) {
          if (generation === this.generation) {
            this.update({
              groups: this.state.groups.map((g) =>
                repositoryKey(g.repository) === key
                  ? { ...g, loading: false, error: message(error) }
                  : g,
              ),
            });
            this.ensureSelection();
          }
        }
      },
    });
  }
  private ensureSelection() {
    const rows = this.rows();
    if (!this.state.selected)
      this.update({ selected: rows.find((row) => row.kind === "pr")?.key ?? rows[0]?.key ?? null });
    else if (
      !this.state.groups.some((group) => group.loading) &&
      !rows.some((row) => row.key === this.state.selected)
    ) {
      this.update({
        selected: rows.find((row) => row.kind === "pr")?.key ?? rows[0]?.key ?? null,
        notice:
          "Previously selected PR is no longer in the loaded results. The workspace target was not changed.",
      });
    }
  }
  move(delta: number) {
    this.clearApproval();
    const rows = this.rows();
    if (!rows.length) return;
    const index = (this.selectedIndex() + delta + rows.length) % rows.length;
    this.update({
      selected: rows[index]!.key,
      ...(this.state.browserOpening ? { notice: null } : {}),
    });
  }
  cycleFocus() {
    const fields: ProviderFocus[] = ["repository", "state", "author", "query", "list"];
    this.update({ focus: fields[(fields.indexOf(this.state.focus) + 1) % fields.length]! });
  }
  cycleRepository(client: BackendClient, delta: number) {
    const choices = [null, ...this.state.repositories.map(repositoryKey)];
    const next =
      choices[(choices.indexOf(this.state.repository) + delta + choices.length) % choices.length]!;
    const repo = this.state.repositories.find((repo) => repositoryKey(repo) === next);
    this.update({
      repository: next,
      author: repo && this.state.author?.provider !== repo.provider ? null : this.state.author,
    });
    this.refresh(client, false);
  }
  cycleFilter(client: BackendClient, delta: number) {
    this.update({
      filter:
        prFilters[
          (prFilters.indexOf(this.state.filter) + delta + prFilters.length) % prFilters.length
        ]!,
    });
    this.refresh(client, false);
  }
  cycleAuthor(client: BackendClient, delta: number) {
    const repository = this.state.repositories.find(
      (repo) => repositoryKey(repo) === this.state.repository,
    );
    const choices = [
      null,
      ...this.authors().filter((author) => !repository || author.provider === repository.provider),
    ];
    const index = choices.findIndex((author) => author?.key === this.state.author?.key);
    this.update({ author: choices[(index + delta + choices.length) % choices.length]! });
    this.refresh(client, false);
  }
  beginQuery() {
    this.update({ editing: true, text: this.state.query, focus: "query" });
  }
  append(text: string) {
    if (/\p{Cc}/u.test(text)) {
      this.update({ notice: "Query cannot contain terminal controls or newlines." });
      return;
    }
    if (Buffer.byteLength(this.state.text + text) > 256) {
      this.update({ notice: "Query is limited to 256 UTF-8 bytes." });
      return;
    }
    this.update({ text: this.state.text + text });
  }
  backspace() {
    const parts = [...querySegmenter.segment(this.state.text)];
    this.update({
      text: parts
        .slice(0, -1)
        .map((part) => part.segment)
        .join(""),
    });
  }
  clearQueryText() {
    this.update({ text: "" });
  }
  cancelQuery() {
    this.update({ editing: false, text: this.state.query, focus: "list" });
  }
  applyQuery(client: BackendClient) {
    this.update({ editing: false, query: this.state.text, focus: "list" });
    this.refresh(client, false);
  }
  toggleGroup() {
    const row = this.selectedRow();
    if (!row) return;
    const key = repositoryKey(row.group.repository);
    this.update({
      groups: this.state.groups.map((group) =>
        repositoryKey(group.repository) === key ? { ...group, collapsed: !group.collapsed } : group,
      ),
      selected: `repo:${key}`,
    });
  }
  loadMore(client: BackendClient) {
    const group = this.selectedRow()?.group;
    if (!group || group.loading || group.nextPage === null) return;
    const key = repositoryKey(group.repository);
    this.update({
      groups: this.state.groups.map((g) =>
        repositoryKey(g.repository) === key ? { ...g, loading: true } : g,
      ),
    });
    this.schedulePage(client, group.repository, group.nextPage, group.nextPage, this.generation);
  }
  private clearApproval() {
    this.approvalGeneration++;
    this.update({
      approvalOpen: false,
      approvalBusy: false,
      approvalSending: false,
      approvalPreview: null,
      approvalResult: null,
      approvalError: null,
    });
  }
  closeApproval() {
    if (this.state.approvalSending) {
      this.update({
        approvalError:
          "Approval is being sent. Inspect its result; closing cannot undo a provider write.",
      });
      return;
    }
    this.clearApproval();
  }
  async previewApproval(client: BackendClient) {
    const row = this.selectedRow();
    if (row?.kind !== "pr" || this.state.approvalBusy) return;
    const generation = this.generation;
    const approval = ++this.approvalGeneration;
    const current = () =>
      generation === this.generation &&
      approval === this.approvalGeneration &&
      this.state.selected === row.key;
    this.update({
      approvalOpen: true,
      approvalBusy: true,
      approvalSending: false,
      approvalPreview: null,
      approvalResult: null,
      approvalError: null,
    });
    try {
      const preview = await client.providerApprovalPreview(
        { repository: row.group.repository, prId: row.pr.id },
        current,
      );
      if (current()) this.update({ approvalPreview: preview });
    } catch (error) {
      if (current()) this.update({ approvalError: message(error) });
    } finally {
      if (current()) this.update({ approvalBusy: false });
    }
  }
  async confirmApproval(client: BackendClient) {
    const preview = this.state.approvalPreview;
    if (
      !this.state.approvalOpen ||
      this.state.approvalBusy ||
      !preview?.approvalId ||
      !preview.expiresAt
    )
      return;
    if (Date.now() / 1000 >= preview.expiresAt) {
      this.update({
        approvalPreview: { ...preview, approvalId: null, expiresAt: null },
        approvalError: "Preview expired. Close and refresh before confirming.",
      });
      return;
    }
    const generation = this.generation,
      approval = this.approvalGeneration,
      selected = this.state.selected;
    const current = () =>
      generation === this.generation &&
      approval === this.approvalGeneration &&
      selected === this.state.selected;
    // Consume in UI before awaiting, independently of native single-use enforcement.
    this.update({
      approvalPreview: { ...preview, approvalId: null, expiresAt: null },
      approvalBusy: true,
      approvalSending: true,
      approvalError: null,
    });
    try {
      const result = await client.providerApprovalConfirm(
        { request: preview.request, approvalId: preview.approvalId },
        current,
      );
      if (current()) this.update({ approvalResult: result });
    } catch (error) {
      if (current())
        this.update({ approvalError: `${message(error)} Check status before sending again.` });
    } finally {
      if (current()) this.update({ approvalBusy: false, approvalSending: false });
    }
  }
  async approvalStatus(client: BackendClient, reconcile: boolean) {
    const row = this.selectedRow();
    if (row?.kind !== "pr" || !this.state.approvalOpen || this.state.approvalBusy) return;
    const generation = this.generation,
      approval = this.approvalGeneration;
    const current = () =>
      generation === this.generation &&
      approval === this.approvalGeneration &&
      row.key === this.state.selected;
    this.update({ approvalBusy: true, approvalPreview: null, approvalError: null });
    try {
      const result = await client.providerApprovalStatus(
        { repository: row.group.repository, prId: row.pr.id },
        reconcile,
        current,
      );
      if (current()) this.update({ approvalResult: result });
    } catch (error) {
      if (current()) this.update({ approvalError: message(error) });
    } finally {
      if (current()) this.update({ approvalBusy: false });
    }
  }

  async openBrowser(client: BackendClient): Promise<void> {
    const row = this.selectedRow();
    if (row?.kind !== "pr" || this.state.browserOpening) return;
    const generation = this.generation;
    const current = () => generation === this.generation && this.state.selected === row.key;
    this.update({ browserOpening: true, notice: "Requesting the provider PR page…" });
    try {
      const result = await client.providerBrowser(
        { repository: row.group.repository, prId: row.pr.id },
        current,
      );
      if (current())
        this.update({
          notice: result.launchError
            ? `Provider browser unavailable: ${result.launchError}`
            : result.launchRequested
              ? "Provider PR page requested. Check your browser."
              : "Provider browser launching is disabled; use copy URL instead.",
        });
    } catch (error) {
      if (current()) this.update({ notice: message(error) });
    } finally {
      if (generation === this.generation) this.update({ browserOpening: false });
    }
  }

  showDetail(client: BackendClient) {
    const row = this.selectedRow();
    if (row?.kind !== "pr") return;
    const generation = this.generation;
    const detailGeneration = ++this.detailGeneration;
    this.queue = this.queue.filter((task) => task.kind !== "detail");
    this.update({
      detailOpen: true,
      detailKey: row.key,
      detail: null,
      detailLoading: true,
      detailError: null,
    });
    this.enqueue(
      {
        generation,
        kind: "detail",
        run: async () => {
          const current = () =>
            generation === this.generation &&
            detailGeneration === this.detailGeneration &&
            this.state.detailOpen;
          try {
            const detail = await client.providerPrDetail(
              { repository: row.group.repository, prId: row.pr.id },
              current,
            );
            if (current()) this.update({ detail, detailLoading: false });
          } catch (error) {
            if (current()) this.update({ detailLoading: false, detailError: message(error) });
          }
        },
      },
      true,
    );
  }
  closeDetail() {
    this.detailGeneration++;
    this.queue = this.queue.filter((task) => task.kind !== "detail");
    this.update({ detailOpen: false, detailLoading: false });
  }
}
