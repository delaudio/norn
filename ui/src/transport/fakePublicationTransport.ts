// Synthetic provider for native component fixtures. Never used by the process transport.
import type {
  ApprovedComment,
  PublicationJournalEntry,
  PublicationScope,
  RemoteComment,
  WorkspaceDraft,
  WorkspaceValue,
} from "../../../src/protocol/generated/protocol";
import { FakeBackendTransport } from "./fakeTransport";

const hash = (value: unknown) =>
  new Bun.CryptoHasher("sha256").update(JSON.stringify(value)).digest("hex");
export const publicationFixtureScope: PublicationScope = {
  provider: "github",
  workspace: "synthetic",
  repo: "review",
  prId: 7,
};
export const publicationFixtureDrafts: WorkspaceDraft[] = [
  {
    localId: "draft-1",
    prId: 7,
    path: "src/example.ts",
    from: 12,
    raw: "Review the missing input 世界.\r\nOriginal line ending is retained.",
    publicationMode: "inline",
    createdAt: 1,
    source: "manual",
    reviewHeadSha: "b".repeat(40),
    reviewBaseSha: "a".repeat(40),
  },
  {
    localId: "draft-2",
    prId: 7,
    path: "",
    raw: "General reviewer note.",
    publicationMode: "general",
    createdAt: 2,
    source: "manual",
  },
];
export class FakePublicationTransport extends FakeBackendTransport {
  calls: string[] = [];
  uncertainAt: number | null = null;
  failAt: number | null = null;
  holdPrepare: Promise<void> | null = null;
  holdApply: Promise<void> | null = null;
  comments: RemoteComment[] = [
    {
      id: "123",
      parentId: null,
      authorId: "synthetic-author",
      authorName: "Synthetic reviewer",
      raw: "Please preserve the old-side anchor 世界.",
      createdAt: "2026-01-01T00:00:00Z",
      deleted: false,
      mode: "inline",
      path: "src/example.ts",
      from: 12,
      to: null,
      commitId: "b".repeat(40),
      rangeStart: null,
    },
  ];
  private publicationApprovals = new Map<
    string,
    { scope: PublicationScope; comments: ApprovedComment[]; consumed: Set<string> }
  >();
  private journal: PublicationJournalEntry[] = [];
  private applied = 0;
  constructor(drafts = publicationFixtureDrafts) {
    super({
      workspaceRecords: {
        "drafts.github:synthetic/review/7": { kind: "drafts", value: structuredClone(drafts) },
      },
    });
  }
  async request(method: string, params: Record<string, unknown>): Promise<Record<string, unknown>> {
    this.calls.push(method);
    if (!method.startsWith("publication.") && method !== "comments.list")
      return super.request(method, params);
    const scope = params.scope as PublicationScope;
    const key = `drafts.${scope.provider}:${scope.workspace}/${scope.repo}/${scope.prId}`;
    if (method === "publication.prepare") {
      if (this.holdPrepare) await this.holdPrepare;
      const record = await super.request("workspace.read", { key, offset: 0, limit: 100 });
      const drafts = (record.payload as WorkspaceValue).value as WorkspaceDraft[];
      const comments = (params.localIds as string[]).map((id): ApprovedComment => {
        const draft = drafts.find((draft) => draft.localId === id);
        if (!draft) throw new Error("Draft missing");
        return {
          scope,
          localId: id,
          raw: draft.raw,
          path: draft.path,
          to: draft.to ?? null,
          from: draft.from ?? null,
          parentId: draft.parentId ?? null,
          mode: draft.publicationMode as ApprovedComment["mode"],
          headSha: draft.reviewHeadSha ?? "b".repeat(40),
          baseSha: draft.reviewBaseSha ?? "a".repeat(40),
          originalDraftHash: hash(draft),
          ...(draft.findingRef ? { findingRef: draft.findingRef } : {}),
        };
      });
      const approvalId = hash([scope, comments, this.publicationApprovals.size]);
      this.publicationApprovals.set(approvalId, { scope, comments, consumed: new Set() });
      return {
        scope,
        key,
        approvalId,
        recordVersion: record.recordVersion,
        expiresAt: Date.now() + 300000,
        items: comments.map((comment) => ({
          comment,
          markdown: `${comment.raw}\n\n<!-- norn-draft:${hash([scope.provider, scope.workspace.toLowerCase(), scope.repo.toLowerCase(), scope.prId, comment.localId])} -->`,
        })),
      };
    }
    if (method === "publication.cancel")
      return { canceled: this.publicationApprovals.delete(String(params.approvalId)) };
    if (method === "publication.apply") {
      if (this.holdApply) await this.holdApply;
      const approval = this.publicationApprovals.get(String(params.approvalId));
      const comment = approval?.comments.find((comment) => comment.localId === params.localId);
      if (
        !approval ||
        !comment ||
        approval.consumed.has(comment.localId) ||
        JSON.stringify(scope) !== JSON.stringify(approval.scope)
      )
        throw new Error("Approval unavailable");
      approval.consumed.add(comment.localId);
      this.applied++;
      if (this.failAt === this.applied)
        throw new Error("Request timed out after send; reconcile before retrying.");
      const uncertain = this.uncertainAt === this.applied;
      const entry: PublicationJournalEntry = {
        comment,
        payloadHash: hash(comment),
        attempt: hash([comment, this.applied]),
        state: uncertain ? "uncertain" : "published",
        remoteId: uncertain ? null : String(100 + this.applied),
        startedAt: 1,
        confirmedAt: 2,
      };
      this.journal.push(entry);
      if (!uncertain) await this.removeDraft(key, comment.localId);
      return { scope, entry, localSaved: !uncertain, draftRemoved: !uncertain };
    }
    if (method === "publication.status") {
      const entries = params.after
        ? []
        : this.journal.filter(
            (entry) => JSON.stringify(entry.comment.scope) === JSON.stringify(scope),
          );
      return { scope, entries, nextAfter: null };
    }
    if (method === "publication.reconcile") {
      const entry = this.journal.find((entry) => entry.comment.localId === params.localId);
      if (!entry) throw new Error("Publication missing");
      entry.state = "published";
      entry.remoteId = "reconciled-123";
      entry.confirmedAt = 2;
      await this.removeDraft(key, entry.comment.localId);
      return { scope, entry, localSaved: true, draftRemoved: true };
    }
    if (method === "comments.list")
      return {
        scope,
        comments: this.comments,
        version: hash(this.comments),
        offset: 0,
        totalItems: this.comments.length,
        nextOffset: null,
      };
    throw new Error("Unsupported synthetic publication request");
  }
  private async removeDraft(key: string, localId: string) {
    const record = await super.request("workspace.read", { key, offset: 0, limit: 100 });
    await super.request("workspace.mutate", {
      key,
      expectedVersion: record.recordVersion,
      mutation: { kind: "removeDraft", localId },
    });
  }
}
