import type { ScrollBoxRenderable } from "@opentui/core";
import type { RefObject } from "react";
import type { FixPreview, FixState } from "../../transport/backendClient";
import { theme } from "../theme";

export function FixPanel({
  preview,
  result,
  loading,
  scroll,
}: {
  preview: FixPreview | null;
  result: FixState | null;
  loading: boolean;
  scroll: RefObject<ScrollBoxRenderable | null>;
}) {
  if (!preview && !result && !loading) return null;
  return (
    <box
      height={preview ? 8 : result ? 4 : 1}
      flexShrink={0}
      paddingX={1}
      flexDirection="column"
      backgroundColor={theme.panel}
    >
      <text fg={theme.primary} height={1} wrapMode="none" truncate>
        {preview
          ? preview.action === "commit"
            ? "Approve commit? · y approve · Esc/n dismiss"
            : preview.action === "push"
              ? "Approve push? · y approve · Esc/n dismiss"
              : "Approve filesystem changes? · y approve · Esc/n dismiss"
          : loading
            ? "Inspecting fix effects…"
            : `Fix · ${result?.status} · ${result?.phase}`}
      </text>
      <scrollbox ref={scroll} height="100%" flexGrow={1} minHeight={0}>
        {preview ? (
          <>
            <text
              fg={theme.text}
            >{`${preview.repoPath} · ${preview.currentBranch} → ${preview.sourceBranch}`}</text>
            <text
              fg={theme.muted}
            >{`${preview.dirtyFileCount ?? preview.dirtyFiles.length} local edit(s) · approval expires in ${preview.expiresInSeconds}s`}</text>
            {preview.effects.map((effect) => (
              <text key={effect} fg={theme.text}>
                {effect}
              </text>
            ))}
          </>
        ) : result ? (
          <>
            <text fg={theme.text}>
              {result.commitSha
                ? `Commit: ${result.commitSha}`
                : (result.summary ?? "Inspect partial edits in the checkout.")}
            </text>
            <text
              fg={theme.muted}
            >{`Files: ${result.filesTouched.join(", ") || "none reported"}`}</text>
            <text fg={theme.muted}>{`Checks: ${result.tests.join("; ") || "none reported"}`}</text>
            <text
              fg={theme.text}
            >{`Suggested commit: ${result.suggestedCommitMessage ?? "none"}`}</text>
          </>
        ) : null}
      </scrollbox>
    </box>
  );
}
