/** @jsxImportSource react */
import type { Meta, StoryObj } from "@storybook/react-vite";
import type { TerminalFrameSet } from "../frame";
import framesJson from "../generated/frames.json";
import { TerminalPreview } from "../TerminalPreview";

const frames = framesJson as TerminalFrameSet;
const frameIds = Object.keys(frames);
const first = frameIds[0] ?? "";

const meta = {
  title: "Norn/Terminal frames",
  component: TerminalPreview,
  parameters: { layout: "centered", controls: { disable: true } },
  args: { frameId: first },
} satisfies Meta<typeof TerminalPreview>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Ready80x24: Story = { args: { frameId: "shell-ready-80x24" } };
export const Ready50x15: Story = { args: { frameId: "shell-ready-50x15" } };
export const Ready120x30: Story = { args: { frameId: "shell-ready-120x30" } };
export const Empty: Story = { args: { frameId: "shell-empty-80x24" } };
export const Connecting: Story = { args: { frameId: "shell-connecting-80x24" } };
export const ErrorState: Story = { args: { frameId: "shell-error-80x24" } };
export const Closed: Story = { args: { frameId: "shell-closed-80x24" } };
export const Review: Story = { args: { frameId: "shell-review-80x24" } };
export const ReviewTargets: Story = { args: { frameId: "shell-review-targets-120x30" } };
export const ImportedScopes: Story = { args: { frameId: "shell-imported-scopes-120x30" } };
export const CompactImportedScopes: Story = { args: { frameId: "shell-imported-scopes-50x15" } };
export const UnconfiguredDrafts: Story = { args: { frameId: "shell-unconfigured-drafts-120x30" } };
export const CompactUnconfiguredDrafts: Story = {
  args: { frameId: "shell-unconfigured-drafts-50x15" },
};
export const Split: Story = { args: { frameId: "shell-split-120x30" } };
export const LargeDiff: Story = { args: { frameId: "shell-large-diff-120x30" } };
export const StaleAnchor: Story = { args: { frameId: "shell-stale-anchor-80x24" } };
export const GroupedFindings: Story = { args: { frameId: "shell-grouped-findings-120x30" } };
export const Conversation: Story = { args: { frameId: "shell-conversation-120x30" } };
export const CompactConversation: Story = { args: { frameId: "shell-conversation-50x15" } };

export const FixPreview: Story = { args: { frameId: "shell-fix-preview-120x30" } };
export const CompactFixPreview: Story = { args: { frameId: "shell-fix-preview-50x15" } };

export const CommitComposer: Story = { args: { frameId: "shell-commit-composer-120x30" } };
export const CommitPreview: Story = { args: { frameId: "shell-commit-preview-120x30" } };
export const PushPreview: Story = { args: { frameId: "shell-push-preview-120x30" } };
export const CompactPushPreview: Story = { args: { frameId: "shell-push-preview-50x15" } };

export const Gallery: Story = {
  args: { frameId: first },
  render: () => (
    <div style={{ display: "flex", flexWrap: "wrap", gap: 24 }}>
      {frameIds.map((id) => (
        <div key={id}>
          <div style={{ color: "#808080", font: "12px monospace", marginBottom: 4 }}>{id}</div>
          <TerminalPreview frameId={id} />
        </div>
      ))}
    </div>
  ),
};

export const SyncPreview: Story = { args: { frameId: "shell-sync-preview-120x30" } };
export const ConflictsPreview: Story = { args: { frameId: "shell-conflicts-preview-120x30" } };
export const CompactConflictsPreview: Story = {
  args: { frameId: "shell-conflicts-preview-50x15" },
};

export const BackgroundOperation: Story = {
  args: { frameId: "shell-background-operation-120x30" },
};
export const CompactBackgroundOperation: Story = {
  args: { frameId: "shell-background-operation-50x15" },
};

export const Settings: Story = { args: { frameId: "shell-settings-120x40" } };
export const CompactSettings: Story = { args: { frameId: "shell-settings-50x15" } };
export const MaskedCredential: Story = { args: { frameId: "shell-settings-secret-120x40" } };
export const CompactMaskedCredential: Story = { args: { frameId: "shell-settings-secret-50x15" } };
export const SettingsError: Story = { args: { frameId: "shell-settings-error-80x24" } };
export const LightSettings: Story = { args: { frameId: "shell-settings-light-120x40" } };

export const DataPreview: Story = { args: { frameId: "shell-data-preview-120x30" } };
export const CompactDataPreview: Story = { args: { frameId: "shell-data-preview-50x15" } };
export const DataReadError: Story = { args: { frameId: "shell-data-error-80x24" } };
export const DataImported: Story = { args: { frameId: "shell-data-imported-80x24" } };
export const SavedRepositoryPrompt: Story = { args: { frameId: "shell-workspace-saved-120x30" } };
export const UnsavedRepositoryPrompt: Story = { args: { frameId: "shell-workspace-dirty-80x24" } };
export const PromptWriteConflict: Story = { args: { frameId: "shell-workspace-conflict-50x15" } };
export const ClosedPromptState: Story = { args: { frameId: "shell-workspace-disposed-80x24" } };
export const EditingDiffSearch: Story = { args: { frameId: "shell-search-editing-120x30" } };
export const NoDiffSearchMatch: Story = { args: { frameId: "shell-search-none-80x24" } };
export const SplitDiffSearch: Story = { args: { frameId: "shell-search-split-80x24" } };
export const CompactSplitDiffSearch: Story = { args: { frameId: "shell-search-split-50x15" } };

export const ImageComparison: Story = { args: { frameId: "shell-images-modified-120x30" } };
export const CompactImageComparison: Story = { args: { frameId: "shell-images-modified-50x20" } };
export const AddedImage: Story = { args: { frameId: "shell-images-added-80x24" } };
export const DeletedImage: Story = { args: { frameId: "shell-images-deleted-80x24" } };
export const RenamedImage: Story = { args: { frameId: "shell-images-renamed-100x24" } };
export const UnsupportedImage: Story = { args: { frameId: "shell-images-unsupported-80x24" } };

export const WrappedAnchor: Story = { args: { frameId: "shell-wrapped-anchor-120x30" } };
export const CompactWrappedAnchor: Story = { args: { frameId: "shell-wrapped-anchor-80x24" } };

export const ImportedThemePreview: Story = {
  args: { frameId: "shell-settings-imported-theme-120x40" },
};
export const CompactImportedThemePreview: Story = {
  args: { frameId: "shell-settings-imported-theme-50x20" },
};
export const SavedReferences: Story = { args: { frameId: "shell-references-saved-120x40" } };
export const CompactReferences: Story = { args: { frameId: "shell-references-saved-50x20" } };
export const ReferenceEditor: Story = { args: { frameId: "shell-references-editing-120x40" } };
export const ReferenceConflict: Story = { args: { frameId: "shell-references-conflict-80x30" } };
export const ReferenceRemoval: Story = { args: { frameId: "shell-references-remove-80x30" } };
export const NoPullRequestReferences: Story = { args: { frameId: "shell-references-local-50x15" } };

export const SavedDrafts: Story = { args: { frameId: "shell-drafts-saved-120x40" } };
export const CompactDrafts: Story = { args: { frameId: "shell-drafts-saved-50x20" } };
export const DraftEditor: Story = { args: { frameId: "shell-drafts-editing-120x40" } };
export const DraftConflict: Story = { args: { frameId: "shell-drafts-conflict-80x30" } };
export const DraftRemoval: Story = { args: { frameId: "shell-drafts-remove-80x30" } };
export const NoPullRequestDrafts: Story = { args: { frameId: "shell-drafts-local-50x15" } };

export const ChooseFindingDraftMode: Story = { args: { frameId: "shell-staging-choose-120x30" } };
export const FindingDraftPreview: Story = { args: { frameId: "shell-staging-preview-120x40" } };
export const CompactFindingDraftPreview: Story = {
  args: { frameId: "shell-staging-preview-50x20" },
};
export const GeneralFindingDraft: Story = { args: { frameId: "shell-staging-general-80x24" } };
export const UnanchoredFindingDraft: Story = {
  args: { frameId: "shell-staging-unanchored-50x20" },
};
export const StagedFindingDraft: Story = { args: { frameId: "shell-staging-staged-80x30" } };
export const AlreadyStagedFindingDraft: Story = {
  args: { frameId: "shell-staging-duplicate-80x30" },
};
export const FindingDraftConflict: Story = { args: { frameId: "shell-staging-conflict-80x30" } };

export const PublicationSelection: Story = { args: { frameId: "shell-publication-drafts-120x40" } };
export const CompactPublicationSelection: Story = {
  args: { frameId: "shell-publication-drafts-50x20" },
};
export const ExactPublicationPreview: Story = {
  args: { frameId: "shell-publication-preview-120x40" },
};
export const CompactPublicationPreview: Story = {
  args: { frameId: "shell-publication-preview-50x20" },
};
export const PartialPublication: Story = { args: { frameId: "shell-publication-partial-120x40" } };
export const PublicationJournal: Story = { args: { frameId: "shell-publication-status-80x30" } };
export const RemoteCommentThreads: Story = {
  args: { frameId: "shell-publication-threads-120x30" },
};
export const CompactRemoteCommentThreads: Story = {
  args: { frameId: "shell-publication-threads-50x20" },
};

export const ProviderList: Story = { args: { frameId: "shell-provider-list-120x30" } };
export const ProviderListCompact: Story = { args: { frameId: "shell-provider-list-50x15" } };
export const ProviderDetail: Story = { args: { frameId: "shell-provider-detail-120x30" } };
export const ProviderDetailCompact: Story = { args: { frameId: "shell-provider-detail-50x15" } };
export const ProviderEmpty: Story = { args: { frameId: "shell-provider-empty-120x30" } };
export const ProviderEmptyCompact: Story = { args: { frameId: "shell-provider-empty-50x15" } };
export const ProviderError: Story = { args: { frameId: "shell-provider-error-120x30" } };
export const ProviderErrorCompact: Story = { args: { frameId: "shell-provider-error-50x15" } };
export const ProviderLoading: Story = { args: { frameId: "shell-provider-loading-120x30" } };
export const ProviderLoadingCompact: Story = { args: { frameId: "shell-provider-loading-50x15" } };
export const ProviderIdentity: Story = { args: { frameId: "shell-provider-identity-120x30" } };
export const ProviderIdentityCompact: Story = {
  args: { frameId: "shell-provider-identity-50x15" },
};

export const ProviderPreviewGithub120x30: Story = {
  args: { frameId: "shell-provider-preview-github-120x30" },
};
export const ProviderPreviewGithub50x15: Story = {
  args: { frameId: "shell-provider-preview-github-50x15" },
};
export const ProviderPreviewBitbucket120x30: Story = {
  args: { frameId: "shell-provider-preview-bitbucket-120x30" },
};
export const ProviderPreviewBitbucket50x15: Story = {
  args: { frameId: "shell-provider-preview-bitbucket-50x15" },
};
export const ProviderPreviewSaved120x30: Story = {
  args: { frameId: "shell-provider-preview-saved-120x30" },
};
export const ProviderPreviewSaved50x15: Story = {
  args: { frameId: "shell-provider-preview-saved-50x15" },
};
export const ProviderPreviewError120x30: Story = {
  args: { frameId: "shell-provider-preview-error-120x30" },
};
export const ProviderPreviewError50x15: Story = {
  args: { frameId: "shell-provider-preview-error-50x15" },
};
export const ProviderPreviewImage120x30: Story = {
  args: { frameId: "shell-provider-preview-image-120x30" },
};
export const ProviderPreviewImage50x15: Story = {
  args: { frameId: "shell-provider-preview-image-50x15" },
};
