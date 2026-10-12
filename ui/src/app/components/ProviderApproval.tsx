import type { ScrollBoxRenderable } from "@opentui/core";
import type { RefObject } from "react";
import type { ProviderSnapshot } from "../providerStore";
import { theme } from "../theme";

function approvalOutcome(state: ProviderSnapshot): string {
  if (state.approvalSending)
    return "Approval send is in progress. Await its result; do not send again.";
  const proof = state.approvalResult?.proof ?? state.approvalPreview?.proof;
  const entry = state.approvalResult?.entry ?? state.approvalPreview?.entry;
  const result = state.approvalResult;
  const unresolved = entry?.state === "uncertain" || entry?.state === "writing";
  if (result) {
    if (!result.localSaved)
      return "Local receipt unavailable: verify provider status before sending again.";
    if (unresolved) return "Outcome uncertain. Verify status; no automatic resend.";
    if (result.observedApproved)
      return result.outdated
        ? "Approval found for the previous commit; the PR has changed."
        : "Approval verified for the captured commit.";
    if (entry?.state === "rejected")
      return "Provider rejected this attempt. Refresh the preview before another attempt.";
    return "Approval has not been verified.";
  }
  if (state.approvalAttempted)
    return "Approval result unavailable. Check provider status before sending again.";
  if (unresolved) return "An earlier send is unresolved. Verify status without sending again.";
  if (proof?.alreadyApproved) return "This reviewer already approved the current commit.";
  if (entry?.state === "approved")
    return "This approval was already sent. Inspect provider status.";
  if (proof?.draft || (proof && proof.state !== "OPEN"))
    return "Approval is unavailable for a draft or closed PR.";
  return "Preview only. Nothing is sent until you confirm.";
}
function approvalContext(state: ProviderSnapshot) {
  const proof = state.approvalResult?.proof ?? state.approvalPreview?.proof;
  const entry = state.approvalResult?.entry ?? state.approvalPreview?.entry;
  const previous =
    entry &&
    proof &&
    (entry.proof.headSha !== proof.headSha || entry.proof.destinationSha !== proof.destinationSha)
      ? entry.proof
      : null;
  return { proof, entry, previous };
}
export function ProviderApproval({
  state,
  scroll,
}: {
  state: ProviderSnapshot;
  scroll: RefObject<ScrollBoxRenderable | null>;
}) {
  const { proof, entry, previous } = approvalContext(state);
  const confirmable = !!state.approvalPreview?.approvalId && !state.approvalBusy;
  const outcome = approvalOutcome(state);
  return (
    <box flexDirection="column" flexGrow={1} minHeight={0}>
      <text height={2} flexShrink={0} fg={theme.primary}>
        APPROVE PULL REQUEST · y confirm · s status · r verify · b page · Esc cancel · j/k scroll
      </text>
      {state.approvalBusy ? (
        <text height={2} flexShrink={0} fg={theme.warning}>
          {state.approvalSending
            ? "Sending approval once; awaiting provider acknowledgment…"
            : "Reading approval metadata…"}
        </text>
      ) : null}
      <scrollbox
        ref={scroll}
        id="provider-approval"
        flexGrow={1}
        minHeight={0}
        scrollY
        scrollX={false}
      >
        {proof ? (
          <>
            <text
              flexShrink={0}
              fg={theme.text}
            >{`${proof.request.repository.provider} ${proof.request.repository.workspace}/${proof.request.repository.repo} PR #${proof.request.prId}`}</text>
            <text flexShrink={0} fg={theme.text}>
              {proof.title}
            </text>
            <text
              flexShrink={0}
              fg={theme.secondary}
            >{`Reviewer: ${proof.actor.displayName} (${proof.actor.accountId})`}</text>
            <text flexShrink={0} fg={theme.text}>{`Head: ${proof.headSha}`}</text>
            <text flexShrink={0} fg={theme.text}>{`Destination: ${proof.destinationSha}`}</text>
            <text
              flexShrink={0}
              fg={theme.muted}
            >{`${proof.sourceBranch} → ${proof.destinationBranch}`}</text>
            <text flexShrink={0} fg={theme.text}>
              This sends your approval to the provider. It does not merge the PR or publish
              comments.
            </text>
            {proof.request.repository.provider === "bitbucket" ? (
              <text flexShrink={0} fg={theme.warning}>
                Bitbucket cannot pin an approval atomically to a commit; changes during the send
                remain uncertain.
              </text>
            ) : null}
          </>
        ) : null}
        <text flexShrink={0} fg={theme.warning}>
          {outcome}
        </text>
        {previous ? (
          <>
            <text flexShrink={0} fg={theme.warning}>
              The PR changed after this attempt. Its outcome remains tied to these previous commits.
            </text>
            <text flexShrink={0} fg={theme.muted}>{`Attempt head: ${previous.headSha}`}</text>
            <text
              flexShrink={0}
              fg={theme.muted}
            >{`Attempt destination: ${previous.destinationSha}`}</text>
          </>
        ) : null}
        {entry ? (
          <text
            flexShrink={0}
            fg={theme.muted}
          >{`Local attempt ${entry.attempt}: ${entry.state}${entry.remoteReviewId ? ` · receipt ${entry.remoteReviewId}` : ""}`}</text>
        ) : null}
        {confirmable ? (
          <text flexShrink={0} fg={theme.primary}>
            Press y to send this approval once. Esc cancels the preview.
          </text>
        ) : null}
        {state.approvalError ? (
          <text flexShrink={0} fg={theme.error}>
            {state.approvalError}
          </text>
        ) : null}
      </scrollbox>
    </box>
  );
}
