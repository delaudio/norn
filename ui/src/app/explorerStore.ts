import type {
  PrRepositoryIdentity,
  RepositoryExploreBlamePage,
  RepositoryExploreContext,
  RepositoryExploreFile,
  RepositoryExploreFindPage,
  RepositoryExploreFindParams,
  RepositoryExploreMatch,
} from "../../../src/protocol/generated/protocol";
import type { BackendClient } from "../transport/backendClient";

export type ExplorerMode = "content" | "diff" | "blame";
type Position = { offset: number; line: number; column: number };
export interface PreviewWindow extends Position {
  text: string;
  sha: string | null;
  size: number | null;
  next: number | null;
  state: string;
}
export interface ExplorerState {
  repository: PrRepositoryIdentity | null;
  context: RepositoryExploreContext | null;
  branch: string | null;
  head: string | null;
  files: RepositoryExploreFile[];
  total: number;
  offset: number;
  version: string | null;
  next: number | null;
  cursor: number;
  collapsed: string[];
  query: string;
  changedOnly: boolean;
  path: string | null;
  mode: ExplorerMode;
  focus: "tree" | "preview" | "details";
  loading: boolean;
  previewLoading: boolean;
  error: string | null;
  previewError: string | null;
  preview: PreviewWindow | null;
  previous: Position[];
  blame: RepositoryExploreBlamePage | null;
  blameCursor: number;
  previewRow: number;
  previewColumn: number;
  markdown: boolean;
  input: "filter" | "find" | null;
  inputText: string;
  search: RepositoryExploreFindPage | null;
  searchCursor: number;
  searchLoading: boolean;
}
const initial = (): ExplorerState => ({
  repository: null,
  context: null,
  branch: null,
  head: null,
  files: [],
  total: 0,
  offset: 0,
  version: null,
  next: null,
  cursor: 0,
  collapsed: [],
  query: "",
  changedOnly: false,
  path: null,
  mode: "content",
  focus: "tree",
  loading: false,
  previewLoading: false,
  error: null,
  previewError: null,
  preview: null,
  previous: [],
  blame: null,
  blameCursor: 0,
  previewRow: 0,
  previewColumn: 0,
  markdown: false,
  input: null,
  inputText: "",
  search: null,
  searchCursor: 0,
  searchLoading: false,
});
export type TreeRow = {
  kind: "folder" | "file";
  path: string;
  name: string;
  depth: number;
  file?: RepositoryExploreFile;
};
export function treeRows(state: ExplorerState): TreeRow[] {
  const rows: TreeRow[] = [];
  const seen = new Set<string>();
  const collapsed = new Set(state.collapsed);
  for (const file of state.files) {
    const parts = file.path.split("/");
    let hidden = false;
    for (let depth = 0; depth < parts.length - 1; depth++) {
      const path = parts.slice(0, depth + 1).join("/");
      if (!seen.has(path)) {
        seen.add(path);
        rows.push({ kind: "folder", path, name: parts[depth]!, depth });
      }
      if (collapsed.has(path)) {
        hidden = true;
        break;
      }
    }
    if (!hidden)
      rows.push({
        kind: "file",
        path: file.path,
        name: parts.at(-1)!,
        depth: parts.length - 1,
        file,
      });
  }
  return rows;
}
// These are display escapes only. Requests, hashes, paths and offsets retain
// their original data; repository text never emits terminal control sequences.
export function explorerText(text: string): string {
  return Array.from(text)
    .map((character) => {
      const code = character.codePointAt(0)!;
      if (character === "\n") return character;
      return code < 32 || (code >= 127 && code <= 159)
        ? `\\u${code.toString(16).padStart(4, "0")}`
        : character;
    })
    .join("");
}
export function explorerLabel(text: string): string {
  return explorerText(text).replaceAll("\n", "\\n");
}
function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : "Repository preview is unavailable.";
}

export class ExplorerStore {
  private state = initial();
  private listeners = new Set<() => void>();
  private client: BackendClient | null = null;
  private generation = 0;
  private listing = 0;
  private reading = 0;
  private searching = 0;
  getSnapshot = () => this.state;
  subscribe = (listener: () => void) => {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  };
  private update(values: Partial<ExplorerState>) {
    this.state = { ...this.state, ...values };
    for (const listener of this.listeners) listener();
  }
  dispose() {
    this.generation++;
    this.listing++;
    this.reading++;
    this.searching++;
    this.client = null;
  }
  async load(client: BackendClient, repository: PrRepositoryIdentity | null, available = true) {
    this.dispose();
    this.client = client;
    const generation = this.generation;
    this.state = initial();
    this.update({ repository, loading: true });
    if (!available) {
      this.update({
        loading: false,
        error:
          "Repository files are unavailable with this backend. Install a matching Norn workspace and backend.",
      });
      return;
    }
    if (!repository) {
      this.update({
        loading: false,
        error: "Select a configured repository to explore its local files.",
      });
      return;
    }
    try {
      const info = await client.repositoryExploreInfo({ repository });
      if (generation !== this.generation) return;
      this.update({ context: info.context, branch: info.branch, head: info.headSha });
      await this.pageFiles(0);
    } catch (error) {
      if (generation === this.generation)
        this.update({ loading: false, error: errorMessage(error) });
    }
  }
  async refresh() {
    if (this.client) await this.load(this.client, this.state.repository);
  }
  async pageFiles(direction: number) {
    const client = this.client,
      context = this.state.context;
    if (!client || !context) return;
    const offset =
      direction > 0 ? this.state.next : direction < 0 ? Math.max(0, this.state.offset - 32) : 0;
    if (offset === null) return;
    const generation = this.generation,
      listing = ++this.listing;
    this.reading++;
    this.searching++;
    const params = {
      context,
      offset,
      limit: 32,
      query: this.state.query,
      changedOnly: this.state.changedOnly,
      version: direction === 0 ? null : this.state.version,
    };
    this.update({
      loading: true,
      error: null,
      path: null,
      preview: null,
      blame: null,
      previewLoading: false,
      search: null,
      searchLoading: false,
    });
    try {
      const page = await client.repositoryExploreFiles(params);
      if (generation !== this.generation || listing !== this.listing) return;
      this.update({
        files: page.files,
        total: page.total,
        offset,
        version: page.version,
        next: page.nextOffset,
        loading: false,
        cursor: 0,
        focus: "tree",
      });
    } catch (error) {
      if (generation === this.generation && listing === this.listing)
        this.update({
          loading: false,
          files: [],
          total: 0,
          offset: 0,
          version: null,
          next: null,
          error: errorMessage(error),
        });
    }
  }
  move(delta: number) {
    if (this.state.focus === "preview") {
      if (this.state.mode === "blame")
        this.update({
          blameCursor: Math.max(
            0,
            Math.min((this.state.blame?.lines.length ?? 1) - 1, this.state.blameCursor + delta),
          ),
          previewRow: 0,
        });
      else
        this.update({
          previewRow: Math.max(
            0,
            Math.min(
              Math.max(0, (this.state.preview?.text.split("\n").length ?? 1) - 1),
              this.state.previewRow + delta,
            ),
          ),
        });
      return;
    }
    this.update({
      cursor: Math.max(
        0,
        Math.min(Math.max(0, treeRows(this.state).length - 1), this.state.cursor + delta),
      ),
    });
  }
  cycleFocus() {
    this.update({
      focus:
        this.state.focus === "tree"
          ? "preview"
          : this.state.focus === "preview" && this.state.mode === "blame"
            ? "details"
            : "tree",
    });
  }
  moveHorizontal(delta: number) {
    const longest = Math.max(
      0,
      ...Array.from(
        this.state.preview?.text.split("\n") ?? [],
        (line) => Array.from(explorerText(line)).length,
      ),
    );
    this.update({
      previewColumn: Math.max(0, Math.min(longest, this.state.previewColumn + delta)),
    });
  }
  async choose() {
    if (this.state.loading) return;
    const row = treeRows(this.state)[this.state.cursor];
    if (!row) return;
    if (row.kind === "folder") {
      const closed = new Set(this.state.collapsed);
      if (closed.has(row.path)) closed.delete(row.path);
      else closed.add(row.path);
      this.update({ collapsed: [...closed] });
      return;
    }
    this.searching++;
    this.update({
      path: row.path,
      focus: "preview",
      search: null,
      searchLoading: false,
      preview: null,
      blame: null,
      previous: [],
      previewRow: 0,
      previewError: null,
    });
    await this.setMode("content");
  }
  async setMode(mode: ExplorerMode) {
    this.searching++;
    this.update({
      mode,
      preview: null,
      blame: null,
      previous: [],
      search: null,
      searchLoading: false,
      markdown: false,
      previewRow: 0,
      blameCursor: 0,
      previewColumn: 0,
      focus: this.state.focus === "details" ? "preview" : this.state.focus,
    });
    await this.read({ offset: 0, line: 1, column: 0 }, null);
  }
  private async read(position: Position, sha: string | null, previous: Position[] = []) {
    const client = this.client,
      context = this.state.context,
      path = this.state.path,
      mode = this.state.mode;
    if (!client || !context || !path) return;
    const generation = this.generation,
      reading = ++this.reading;
    this.update({ previewLoading: true, previewError: null });
    try {
      if (mode === "blame") {
        const content = await client.repositoryExploreContent({
          context,
          path,
          offset: 0,
          maxBytes: 4,
          contentSha256: sha,
        });
        if (generation !== this.generation || reading !== this.reading) return;
        if (content.state !== "text" || !content.contentSha256)
          throw new Error("Blame is available for text files only.");
        const page = await client.repositoryExploreBlame({
          context,
          path,
          offset: position.offset,
          limit: 32,
          contentSha256: content.contentSha256,
        });
        if (generation !== this.generation || reading !== this.reading) return;
        this.update({
          blame: page,
          previewLoading: false,
          previewRow: 0,
          blameCursor: 0,
          previous,
        });
      } else {
        const params = {
          context,
          path,
          offset: position.offset,
          maxBytes: 32768,
          contentSha256: sha,
        };
        const page =
          mode === "content"
            ? await client.repositoryExploreContent(params)
            : await client.repositoryExploreDiff(params);
        if (generation !== this.generation || reading !== this.reading) return;
        const preview = {
          ...position,
          text: page.text,
          sha: page.contentSha256,
          size: "size" in page ? page.size : page.totalBytes,
          next: page.nextOffset,
          state: "state" in page ? page.state : "diff",
        };
        this.update({ preview, previewLoading: false, previous, previewRow: 0, previewColumn: 0 });
      }
    } catch (error) {
      if (generation === this.generation && reading === this.reading)
        this.update({
          previewLoading: false,
          preview: null,
          blame: null,
          previewError: errorMessage(error),
        });
    }
  }
  async pagePreview(direction: number) {
    if (this.state.previewLoading) return;
    if (this.state.mode === "blame") {
      const page = this.state.blame;
      if (!page) return;
      const offset = direction > 0 ? page.nextOffset : Math.max(0, page.request.offset - 32);
      if (offset === null) return;
      await this.read({ offset, line: offset + 1, column: 0 }, page.request.contentSha256);
      return;
    }
    const preview = this.state.preview;
    if (!preview) return;
    if (direction < 0) {
      const previous = [...this.state.previous];
      const position = previous.pop();
      if (!position) return;
      await this.read(position, preview.sha, previous);
      return;
    }
    if (preview.next === null) return;
    const parts = preview.text.split("\n");
    const position = {
      offset: preview.next,
      line: preview.line + parts.length - 1,
      column: parts.length === 1 ? preview.column + parts[0]!.length : parts.at(-1)!.length,
    };
    await this.read(
      position,
      preview.sha,
      [
        ...this.state.previous,
        { offset: preview.offset, line: preview.line, column: preview.column },
      ].slice(-256),
    );
  }
  async pageViewport(direction: number, rows: number) {
    if (this.state.mode === "blame" || !this.state.preview) {
      await this.pagePreview(direction);
      return;
    }
    const lastStart = Math.max(0, this.state.preview.text.split("\n").length - rows);
    if (direction > 0 && this.state.previewRow < lastStart)
      this.update({ previewRow: Math.min(lastStart, this.state.previewRow + rows) });
    else if (direction < 0 && this.state.previewRow > 0)
      this.update({ previewRow: Math.max(0, this.state.previewRow - rows) });
    else await this.pagePreview(direction);
  }
  async firstPreview() {
    await this.read(
      { offset: 0, line: 1, column: 0 },
      this.state.preview?.sha ?? this.state.blame?.request.contentSha256 ?? null,
    );
  }
  toggleMarkdown() {
    if (
      this.state.mode === "content" &&
      this.state.preview?.state === "text" &&
      /\.(md|markdown)$/i.test(this.state.path ?? "")
    )
      this.update({ markdown: !this.state.markdown });
  }
  async toggleChanged() {
    this.update({ changedOnly: !this.state.changedOnly });
    await this.pageFiles(0);
  }
  openInput(kind: "filter" | "find") {
    if (
      kind === "find" &&
      (this.state.mode === "blame" ||
        !this.state.preview?.sha ||
        !["text", "diff"].includes(this.state.preview.state))
    ) {
      this.update({ previewError: "Load a text or diff preview before searching." });
      return;
    }
    this.update({
      input: kind,
      inputText: kind === "filter" ? this.state.query : (this.state.search?.request.query ?? ""),
    });
  }
  cancelInput() {
    this.update({ input: null, inputText: "" });
  }
  appendInput(text: string) {
    const value = this.state.inputText + text;
    if (
      Buffer.byteLength(value) <= 256 &&
      !Array.from(text).some((ch) => {
        const n = ch.charCodeAt(0);
        return n < 32 || (n >= 127 && n <= 159);
      })
    )
      this.update({ inputText: value });
  }
  backspaceInput() {
    this.update({ inputText: Array.from(this.state.inputText).slice(0, -1).join("") });
  }
  clearInput() {
    this.update({ inputText: "" });
  }
  async applyInput() {
    const kind = this.state.input,
      text = this.state.inputText;
    this.cancelInput();
    if (kind === "find" && !text) {
      this.searching++;
      this.update({ search: null, searchCursor: 0, searchLoading: false });
      return;
    }
    if (kind === "filter") {
      this.update({ query: text });
      await this.pageFiles(0);
      return;
    }
    const preview = this.state.preview,
      context = this.state.context,
      path = this.state.path;
    if (
      kind !== "find" ||
      !text ||
      !preview?.sha ||
      !context ||
      !path ||
      this.state.mode === "blame"
    )
      return;
    await this.find(
      {
        context,
        path,
        source: this.state.mode,
        query: text,
        offset: 0,
        limit: 32,
        contentSha256: preview.sha,
      },
      0,
    );
  }
  private async find(params: RepositoryExploreFindParams, cursor: number) {
    const client = this.client;
    if (!client) return;
    const generation = this.generation,
      searching = ++this.searching;
    this.update({ searchLoading: true, previewError: null });
    try {
      const search = await client.repositoryExploreFind(params);
      if (generation !== this.generation || searching !== this.searching) return;
      this.update({
        search,
        searchCursor: Math.max(0, Math.min(search.matches.length - 1, cursor)),
        searchLoading: false,
      });
      await this.jumpMatch();
    } catch (error) {
      if (generation === this.generation && searching === this.searching)
        this.update({ searchLoading: false, search: null, previewError: errorMessage(error) });
    }
  }
  selectedMatch(): RepositoryExploreMatch | null {
    return this.state.search?.matches[this.state.searchCursor] ?? null;
  }
  private async jumpMatch() {
    const match = this.selectedMatch(),
      search = this.state.search;
    if (!match || !search) return;
    this.update({ focus: "preview", markdown: false });
    await this.read(
      { offset: match.startByte, line: match.line, column: match.columnUtf16 },
      search.request.contentSha256,
    );
  }
  async moveMatch(delta: number) {
    const search = this.state.search;
    if (!search?.total || this.state.searchLoading) return;
    const absolute =
      (search.request.offset + this.state.searchCursor + delta + search.total) % search.total;
    const offset = Math.floor(absolute / 32) * 32;
    if (offset !== search.request.offset) {
      await this.find({ ...search.request, offset }, absolute - offset);
      return;
    }
    this.update({ searchCursor: absolute - offset });
    await this.jumpMatch();
  }
}
