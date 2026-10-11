// Owned synthetic canonical data; no repository config, provider or saved run.
import type { WorkspaceValue } from "../../../src/protocol/generated/protocol";

export const importedScopeRecords: Record<string, WorkspaceValue> = {
  "drafts.github:imported/repo/7": {
    kind: "drafts",
    value: [
      {
        localId: "github-draft",
        prId: 7,
        path: "github.rs",
        raw: "Imported GitHub comment 世界",
        createdAt: 1,
        source: "manual",
        publicationMode: "file",
      },
    ],
  },
  "drafts.bitbucket:imported/repo/7": {
    kind: "drafts",
    value: [
      {
        localId: "bitbucket-draft",
        prId: 7,
        path: "bitbucket.rs",
        raw: "Imported Bitbucket comment ✓",
        createdAt: 1,
        source: "manual",
        publicationMode: "file",
      },
    ],
  },
  "references.github:imported/repo/7": {
    kind: "references",
    value: [
      {
        id: "github-note",
        type: "note",
        source: "manual",
        title: "Imported design note",
        body: "Original reviewer context",
        createdAt: 1,
        updatedAt: 1,
      },
    ],
  },
};
