import { useCallback, useEffect, useState } from "react";
import { readMigratedStorageValue } from "@/lib/storageMigration";
import { tauriCall } from "@/lib/tauri";
import type { DraftComment, PrComment, ReviewProvider } from "@/types";

function storageKey(
  provider: ReviewProvider,
  workspace: string,
  repo: string,
  prId: number,
): string {
  return `norn.drafts.v1.${provider}:${workspace}/${repo}/${prId}`;
}

function legacyStorageKey(
  provider: ReviewProvider,
  workspace: string,
  repo: string,
  prId: number,
): string {
  return `lachesi.drafts.${provider}:${workspace}/${repo}/${prId}`;
}

function normalizeLegacyDrafts(raw: string): string | null {
  const parsed: unknown = JSON.parse(raw);
  if (
    !Array.isArray(parsed) ||
    !parsed.every((draft) => {
      if (draft == null || typeof draft !== "object") return false;
      const value = draft as Record<string, unknown>;
      return (
        typeof value.localId === "string" &&
        typeof value.prId === "number" &&
        typeof value.path === "string" &&
        (value.to == null || typeof value.to === "number") &&
        (value.from == null || typeof value.from === "number") &&
        typeof value.raw === "string" &&
        (value.parentId == null ||
          typeof value.parentId === "string" ||
          typeof value.parentId === "number") &&
        typeof value.createdAt === "number"
      );
    })
  ) {
    return null;
  }
  return JSON.stringify(
    parsed.map((draft) => {
      const record = draft as Record<string, unknown>;
      return {
        ...record,
        parentId: record.parentId == null ? null : String(record.parentId),
      };
    }),
  );
}

function loadDrafts(
  provider: ReviewProvider,
  workspace: string,
  repo: string,
  prId: number,
): DraftComment[] {
  try {
    const raw = readMigratedStorageValue(
      localStorage,
      storageKey(provider, workspace, repo, prId),
      legacyStorageKey(provider, workspace, repo, prId),
      normalizeLegacyDrafts,
    );
    const parsed = raw ? (JSON.parse(raw) as DraftComment[]) : [];
    return parsed.map((draft) => ({
      ...draft,
      parentId: draft.parentId == null ? null : String(draft.parentId),
    }));
  } catch {
    return [];
  }
}

export type NewDraft = Pick<
  DraftComment,
  | "path"
  | "to"
  | "from"
  | "raw"
  | "parentId"
  | "source"
  | "findingRef"
  | "publicationMode"
  | "reviewBaseSha"
  | "reviewHeadSha"
>;
export type DraftPatch = Partial<Pick<DraftComment, "raw">>;

export interface PublishResult {
  published: number;
  failed: { draft: DraftComment; error: string }[];
  errors: string[];
}

export interface PublishedDraftComment {
  id: string;
  createdOn: string;
}

export interface PublishFindingDraftsResult {
  comments: Map<string, PublishedDraftComment>;
  failures: Map<string, string>;
  errors: string[];
}

export interface PublishDraftResult {
  draft: DraftComment | null;
  comment: PublishedDraftComment | null;
  error: string | null;
}

interface UseDraftCommentsResult {
  drafts: DraftComment[];
  publishing: boolean;
  publishingDraftId: string | null;
  addDraft: (draft: NewDraft) => DraftComment | null;
  addDrafts: (drafts: NewDraft[]) => DraftComment[];
  updateDraft: (localId: string, patch: DraftPatch) => void;
  removeDraft: (localId: string) => void;
  discardAll: () => void;
  publishDraft: (localId: string) => Promise<PublishDraftResult>;
  publishAll: () => Promise<PublishResult>;
}

export interface DraftCommentLifecycleOptions {
  publishFindingDraft?: (draft: DraftComment) => Promise<PublishedDraftComment>;
  publishFindingDrafts?: (drafts: DraftComment[]) => Promise<PublishFindingDraftsResult>;
  onFindingDraftPublished?: (
    draft: DraftComment,
    comment: PublishedDraftComment,
  ) => void | Promise<void>;
  onDraftRemoved?: (draft: DraftComment) => void | Promise<void>;
  onDraftsDiscarded?: (drafts: DraftComment[]) => void | Promise<void>;
}

function materializeDraft(prId: number, draft: NewDraft, index: number): DraftComment {
  return {
    ...draft,
    prId,
    localId: `${Date.now()}-${index}-${Math.random().toString(36).slice(2, 8)}`,
    createdAt: Date.now(),
    source: draft.source ?? "manual",
    findingRef: draft.findingRef ?? null,
    publicationMode: draft.publicationMode ?? null,
    reviewBaseSha: draft.reviewBaseSha ?? null,
    reviewHeadSha: draft.reviewHeadSha ?? null,
  };
}

async function publishDraftToServer(
  provider: ReviewProvider,
  workspace: string,
  repo: string,
  prId: number,
  draft: DraftComment,
  publishFindingDraft?: (draft: DraftComment) => Promise<PublishedDraftComment>,
  canTrackFindingPublication = false,
): Promise<PublishedDraftComment> {
  if (draft.findingRef) {
    if (!canTrackFindingPublication) {
      throw new Error(
        "Structured finding tracking is unavailable; refresh the review before publishing.",
      );
    }
    if (!publishFindingDraft) {
      throw new Error("Structured finding publication is unavailable; refresh the review.");
    }
    return publishFindingDraft(draft);
  }

  if (draft.parentId != null) {
    const comment = await tauriCall<PrComment>("create_general_comment", {
      provider,
      workspace,
      repo,
      id: prId,
      raw: draft.raw,
      parentId: draft.parentId,
    });
    return { id: String(comment.id), createdOn: comment.createdOn };
  }

  const comment = await tauriCall<PrComment>("create_inline_comment", {
    provider,
    workspace,
    repo,
    id: prId,
    req: {
      path: draft.path,
      to: draft.to,
      from: draft.from,
      raw: draft.raw,
      parentId: null,
    },
  });
  return { id: String(comment.id), createdOn: comment.createdOn };
}

/**
 * GitHub-style "pending review": comments are staged locally (persisted per
 * repo + PR) and published in a batch to the owning repo.
 */
export function useDraftComments(
  provider: ReviewProvider | null,
  workspace: string | null,
  repo: string | null,
  prId: number | null,
  options: DraftCommentLifecycleOptions = {},
): UseDraftCommentsResult {
  const [drafts, setDrafts] = useState<DraftComment[]>([]);
  const [publishing, setPublishing] = useState(false);
  const [publishingDraftId, setPublishingDraftId] = useState<string | null>(null);
  const {
    publishFindingDraft,
    publishFindingDrafts,
    onFindingDraftPublished,
    onDraftRemoved,
    onDraftsDiscarded,
  } = options;

  const active = provider != null && workspace != null && repo != null && prId != null;

  useEffect(() => {
    setDrafts(active ? loadDrafts(provider, workspace, repo, prId) : []);
  }, [active, provider, workspace, repo, prId]);

  useEffect(() => {
    if (!active) return;
    try {
      localStorage.setItem(storageKey(provider, workspace, repo, prId), JSON.stringify(drafts));
    } catch {
      // ignore storage failures
    }
  }, [drafts, active, provider, workspace, repo, prId]);

  const addDraft = useCallback(
    (draft: NewDraft) => {
      if (prId == null) return null;
      const nextDraft = materializeDraft(prId, draft, drafts.length);
      setDrafts((prev) => [...prev, nextDraft]);
      return nextDraft;
    },
    [prId, drafts.length],
  );

  const addDrafts = useCallback(
    (nextDrafts: NewDraft[]) => {
      if (prId == null || nextDrafts.length === 0) return [];
      const materialized = nextDrafts.map((draft, index) =>
        materializeDraft(prId, draft, drafts.length + index),
      );
      setDrafts((prev) => [...prev, ...materialized]);
      return materialized;
    },
    [prId, drafts.length],
  );

  const removeDraft = useCallback(
    (localId: string) => {
      const draft = drafts.find((candidate) => candidate.localId === localId) ?? null;
      setDrafts((prev) => prev.filter((d) => d.localId !== localId));
      if (draft && onDraftRemoved) {
        void Promise.resolve(onDraftRemoved(draft));
      }
    },
    [drafts, onDraftRemoved],
  );

  const updateDraft = useCallback((localId: string, patch: DraftPatch) => {
    setDrafts((prev) =>
      prev.map((draft) => (draft.localId === localId ? { ...draft, ...patch } : draft)),
    );
  }, []);

  const discardAll = useCallback(() => {
    const discarded = [...drafts];
    setDrafts([]);
    if (discarded.length > 0 && onDraftsDiscarded) {
      void Promise.resolve(onDraftsDiscarded(discarded));
    }
  }, [drafts, onDraftsDiscarded]);

  const publishDraft = useCallback(
    async (localId: string): Promise<PublishDraftResult> => {
      if (!active) {
        return { draft: null, comment: null, error: "No active pull request selected." };
      }

      const draft = drafts.find((candidate) => candidate.localId === localId) ?? null;
      if (!draft) {
        return { draft: null, comment: null, error: "Draft comment not found." };
      }

      setPublishing(true);
      setPublishingDraftId(localId);
      try {
        const comment = await publishDraftToServer(
          provider,
          workspace,
          repo,
          prId,
          draft,
          publishFindingDraft,
          onFindingDraftPublished != null,
        );
        if (draft.findingRef && onFindingDraftPublished) {
          await onFindingDraftPublished(draft, comment);
        }
        setDrafts((prev) => prev.filter((candidate) => candidate.localId !== localId));
        return { draft, comment, error: null };
      } catch (e) {
        return {
          draft,
          comment: null,
          error: e instanceof Error ? e.message : String(e),
        };
      } finally {
        setPublishingDraftId(null);
        setPublishing(false);
      }
    },
    [active, provider, workspace, repo, prId, drafts, publishFindingDraft, onFindingDraftPublished],
  );

  const publishAll = useCallback(async (): Promise<PublishResult> => {
    if (!active) return { published: 0, failed: [], errors: [] };
    setPublishing(true);
    const failed: PublishResult["failed"] = [];
    const errors: string[] = [];
    let published = 0;
    const structuredDrafts = drafts.filter((draft) => draft.findingRef != null);
    if (structuredDrafts.length > 0 && publishFindingDrafts) {
      setPublishingDraftId(structuredDrafts[0]?.localId ?? null);
      let batch: PublishFindingDraftsResult | null = null;
      try {
        batch = await publishFindingDrafts(structuredDrafts);
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        failed.push(...structuredDrafts.map((draft) => ({ draft, error: message })));
      }
      if (batch) {
        errors.push(...batch.errors);
        for (const draft of structuredDrafts) {
          const batchFailure = batch.failures.get(draft.localId);
          if (batchFailure) {
            failed.push({ draft, error: batchFailure });
            continue;
          }
          const comment = batch.comments.get(draft.localId);
          if (!comment) {
            failed.push({
              draft,
              error: "Structured finding reconciliation returned no provider comment.",
            });
            continue;
          }
          try {
            if (onFindingDraftPublished) {
              await onFindingDraftPublished(draft, comment);
            }
            published += 1;
            setDrafts((prev) => prev.filter((candidate) => candidate.localId !== draft.localId));
          } catch (error) {
            failed.push({
              draft,
              error: error instanceof Error ? error.message : String(error),
            });
          }
        }
      }
    }
    const individuallyPublishedDrafts = publishFindingDrafts
      ? drafts.filter((draft) => draft.findingRef == null)
      : drafts;
    for (const draft of individuallyPublishedDrafts) {
      setPublishingDraftId(draft.localId);
      try {
        const comment = await publishDraftToServer(
          provider,
          workspace,
          repo,
          prId,
          draft,
          publishFindingDraft,
          onFindingDraftPublished != null,
        );
        if (draft.findingRef && onFindingDraftPublished) {
          await onFindingDraftPublished(draft, comment);
        }
        published += 1;
        setDrafts((prev) => prev.filter((d) => d.localId !== draft.localId));
      } catch (e) {
        failed.push({ draft, error: e instanceof Error ? e.message : String(e) });
      }
    }
    setPublishingDraftId(null);
    setPublishing(false);
    return { published, failed, errors };
  }, [
    active,
    provider,
    workspace,
    repo,
    prId,
    drafts,
    publishFindingDraft,
    publishFindingDrafts,
    onFindingDraftPublished,
  ]);

  return {
    drafts,
    publishing,
    publishingDraftId,
    addDraft,
    addDrafts,
    updateDraft,
    removeDraft,
    discardAll,
    publishDraft,
    publishAll,
  };
}
