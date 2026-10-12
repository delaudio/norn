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

export const ReviewProfile: Story = { args: { frameId: "shell-review-profile-120x30" } };
export const CompactReviewProfile: Story = { args: { frameId: "shell-review-profile-50x15" } };

export const ProviderBrowserDisabled: Story = {
  args: { frameId: "shell-provider-browser-disabled-120x30" },
};
export const CompactProviderBrowserDisabled: Story = {
  args: { frameId: "shell-provider-browser-disabled-50x15" },
};

export const ProviderApproval: Story = { args: { frameId: "shell-provider-approval-120x30" } };
export const CompactProviderApproval: Story = {
  args: { frameId: "shell-provider-approval-50x22" },
};
export const UncertainProviderApproval: Story = {
  args: { frameId: "shell-provider-approval-uncertain-120x30" },
};
export const CompactUncertainProviderApproval: Story = {
  args: { frameId: "shell-provider-approval-uncertain-50x22" },
};
export const BitbucketApproval: Story = {
  args: { frameId: "shell-provider-approval-bitbucket-120x30" },
};
export const CompactBitbucketApproval: Story = {
  args: { frameId: "shell-provider-approval-bitbucket-50x22" },
};
export const UncertainBitbucketApproval: Story = {
  args: { frameId: "shell-provider-approval-bitbucket-uncertain-120x30" },
};
export const CompactUncertainBitbucketApproval: Story = {
  args: { frameId: "shell-provider-approval-bitbucket-uncertain-50x22" },
};

export const ReviewProfiles: Story = { args: { frameId: "shell-profiles-catalog-120x30" } };
export const CompactReviewProfiles: Story = { args: { frameId: "shell-profiles-catalog-50x18" } };
export const BaseReviewProfile: Story = { args: { frameId: "shell-profiles-base-50x18" } };
export const InvalidReviewProfilePolicy: Story = {
  args: { frameId: "shell-profiles-error-50x18" },
};
export const LoadingReviewProfiles: Story = { args: { frameId: "shell-profiles-loading-50x18" } };
export const SavedLocalSnapshots: Story = { args: { frameId: "shell-saved-local-list-120x30" } };
export const CompactSavedLocalSnapshots: Story = {
  args: { frameId: "shell-saved-local-list-50x18" },
};
export const SavedLocalNextPage: Story = { args: { frameId: "shell-saved-local-next-120x30" } };
export const CompactSavedLocalNextPage: Story = {
  args: { frameId: "shell-saved-local-next-50x18" },
};
export const EmptySavedLocalSnapshots: Story = {
  args: { frameId: "shell-saved-local-empty-120x30" },
};
export const CompactEmptySavedLocalSnapshots: Story = {
  args: { frameId: "shell-saved-local-empty-50x18" },
};
export const LoadingSavedLocalSnapshots: Story = {
  args: { frameId: "shell-saved-local-loading-120x30" },
};
export const CompactLoadingSavedLocalSnapshots: Story = {
  args: { frameId: "shell-saved-local-loading-50x18" },
};
export const InvalidSavedLocalSnapshots: Story = {
  args: { frameId: "shell-saved-local-error-120x30" },
};
export const CompactInvalidSavedLocalSnapshots: Story = {
  args: { frameId: "shell-saved-local-error-50x18" },
};
export const CapturedLocalImages: Story = {
  args: { frameId: "shell-local-image-comparison-120x30" },
};
export const CompactCapturedLocalImages: Story = {
  args: { frameId: "shell-local-image-comparison-50x18" },
};
export const AddedCapturedLocalImage: Story = {
  args: { frameId: "shell-local-image-added-120x30" },
};
export const CompactAddedCapturedLocalImage: Story = {
  args: { frameId: "shell-local-image-added-50x18" },
};
export const LegacyLocalImageUnavailable: Story = {
  args: { frameId: "shell-local-image-legacy-120x30" },
};
export const CompactLegacyLocalImageUnavailable: Story = {
  args: { frameId: "shell-local-image-legacy-50x18" },
};
export const UnsupportedCapturedLocalImage: Story = {
  args: { frameId: "shell-local-image-unsupported-120x30" },
};
export const CompactUnsupportedCapturedLocalImage: Story = {
  args: { frameId: "shell-local-image-unsupported-50x18" },
};

export const RepositoryExplorerTree: Story = { args: { frameId: "explorer-tree-120x30" } };

export const CompactRepositoryExplorerTree: Story = { args: { frameId: "explorer-tree-50x20" } };

export const RepositoryExplorerContent: Story = { args: { frameId: "explorer-content-120x30" } };

export const CompactRepositoryExplorerContent: Story = {
  args: { frameId: "explorer-content-50x20" },
};

export const RepositoryExplorerDiff: Story = { args: { frameId: "explorer-diff-120x30" } };

export const CompactRepositoryExplorerDiff: Story = { args: { frameId: "explorer-diff-50x20" } };

export const RepositoryExplorerBlame: Story = { args: { frameId: "explorer-blame-120x30" } };

export const CompactRepositoryExplorerBlame: Story = { args: { frameId: "explorer-blame-50x20" } };

export const RepositoryExplorerMarkdown: Story = { args: { frameId: "explorer-markdown-120x30" } };

export const CompactRepositoryExplorerMarkdown: Story = {
  args: { frameId: "explorer-markdown-50x20" },
};

export const RepositoryExplorerFind: Story = { args: { frameId: "explorer-find-120x30" } };

export const CompactRepositoryExplorerFind: Story = { args: { frameId: "explorer-find-50x20" } };

export const RepositoryExplorerBinary: Story = { args: { frameId: "explorer-binary-120x30" } };

export const CompactRepositoryExplorerBinary: Story = {
  args: { frameId: "explorer-binary-50x20" },
};

export const RepositoryExplorerDeleted: Story = { args: { frameId: "explorer-deleted-120x30" } };

export const CompactRepositoryExplorerDeleted: Story = {
  args: { frameId: "explorer-deleted-50x20" },
};

export const RepositoryExplorerEmpty: Story = { args: { frameId: "explorer-empty-120x30" } };

export const CompactRepositoryExplorerEmpty: Story = { args: { frameId: "explorer-empty-50x20" } };

export const RepositoryExplorerError: Story = { args: { frameId: "explorer-error-120x30" } };

export const CompactRepositoryExplorerError: Story = { args: { frameId: "explorer-error-50x20" } };

export const RepositoryExplorerLoading: Story = { args: { frameId: "explorer-loading-120x30" } };

export const CompactRepositoryExplorerLoading: Story = {
  args: { frameId: "explorer-loading-50x20" },
};

export const RepositoryEditorLaunched: Story = {
  args: { frameId: "explorer-editor-launched-120x30" },
};

export const CompactRepositoryEditorLaunched: Story = {
  args: { frameId: "explorer-editor-launched-50x20" },
};

export const RepositoryEditorFailed: Story = { args: { frameId: "explorer-editor-failed-120x30" } };

export const CompactRepositoryEditorFailed: Story = {
  args: { frameId: "explorer-editor-failed-50x20" },
};

export const RepositoryEditorUnavailable: Story = {
  args: { frameId: "explorer-editor-unavailable-120x30" },
};

export const CompactRepositoryEditorUnavailable: Story = {
  args: { frameId: "explorer-editor-unavailable-50x20" },
};

export const RepositoryEditorBusy: Story = { args: { frameId: "explorer-editor-busy-120x30" } };
export const CompactRepositoryEditorBusy: Story = {
  args: { frameId: "explorer-editor-busy-50x20" },
};

export const RepositoryBranches: Story = {
  args: { frameId: "repository-operations-branches-120x30" },
};
export const CompactRepositoryBranches: Story = {
  args: { frameId: "repository-operations-branches-50x20" },
};
export const RepositoryApproval: Story = {
  args: { frameId: "repository-operations-preview-120x30" },
};
export const CompactRepositoryApproval: Story = {
  args: { frameId: "repository-operations-preview-50x20" },
};
export const RepositoryBusy: Story = { args: { frameId: "repository-operations-busy-120x30" } };
export const RepositoryRecovery: Story = {
  args: { frameId: "repository-operations-uncertain-50x20" },
};
