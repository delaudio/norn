import defaultReviewPrompt from "@/lib/defaultReviewPrompt.md?raw";
import { type KeyValueStorage, readMigratedStorageValue } from "@/lib/storageMigration";
import { tauriCall } from "@/lib/tauri";
import type { RepoReviewConfigLoadResult } from "@/types";

/** Built-in review prompt, used when a repo has no custom prompt. */
export const DEFAULT_REVIEW_PROMPT = defaultReviewPrompt.trim();

function key(repoKey: string): string {
  return `norn.reviewPrompt.v1.${repoKey}`;
}

function legacyKey(repoKey: string): string {
  return `lachesi.reviewPrompt.${repoKey}`;
}

/** The raw stored prompt for a repo (empty string if none set). */
export function getReviewPrompt(repoKey: string, storage?: KeyValueStorage | null): string {
  try {
    const activeStorage =
      storage === undefined ? (typeof localStorage === "undefined" ? null : localStorage) : storage;
    if (!activeStorage) return "";
    return readMigratedStorageValue(activeStorage, key(repoKey), legacyKey(repoKey)) ?? "";
  } catch {
    return "";
  }
}

export function setReviewPrompt(
  repoKey: string,
  text: string,
  storage?: KeyValueStorage | null,
): void {
  try {
    const activeStorage =
      storage === undefined ? (typeof localStorage === "undefined" ? null : localStorage) : storage;
    if (!activeStorage) return;
    if (text.trim()) activeStorage.setItem(key(repoKey), text);
    else activeStorage.setItem(key(repoKey), "");
  } catch {
    // ignore storage failures
  }
}

/** The prompt to actually use: the repo's custom prompt, else the default. */
export function effectiveReviewPrompt(repoKey: string): string {
  return getReviewPrompt(repoKey).trim() || DEFAULT_REVIEW_PROMPT;
}

export interface ResolvedReviewPrompt {
  prompt: string;
  warnings: string[];
  selectedProfile: string | null;
  availableProfiles: string[];
}

/** Resolve the prompt used by review runs, including repo-owned prompt extensions. */
export async function resolveReviewPrompt(
  repoKey: string,
  repoPath?: string | null,
  reviewProfile?: string | null,
): Promise<ResolvedReviewPrompt> {
  const localOverride = getReviewPrompt(repoKey).trim();
  if (localOverride) {
    return {
      prompt: localOverride,
      warnings: [],
      selectedProfile: reviewProfile?.trim() || null,
      availableProfiles: [],
    };
  }

  if (!repoPath?.trim()) {
    return {
      prompt: DEFAULT_REVIEW_PROMPT,
      warnings: [],
      selectedProfile: null,
      availableProfiles: [],
    };
  }

  const result = await tauriCall<RepoReviewConfigLoadResult>("validate_repo_review_config", {
    repoPath,
    reviewProfile: reviewProfile?.trim() || null,
  });
  if (result.errors.length > 0) {
    throw new Error(result.errors.map((error) => error.message).join("\n"));
  }

  const replacement = result.config?.review?.prompt?.replace?.trim();
  const extension = result.config?.review?.prompt?.extend?.trim();
  const basePrompt = replacement || DEFAULT_REVIEW_PROMPT;
  const prompt = extension
    ? `${basePrompt}\n\n## Repository review policy\n${extension}`
    : basePrompt;

  return {
    prompt,
    warnings: result.warnings.map((warning) => warning.message),
    selectedProfile: result.selectedProfile,
    availableProfiles: Object.keys(result.config?.profiles ?? {}),
  };
}
