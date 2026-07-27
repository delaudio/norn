---
id: FE-001
title: Expose AI review as explicit user-invoked actions
domain: frontend
rules: true
---

# Expose AI review as explicit user-invoked actions

## Context

Lachesi supports AI-assisted review, but the product is still primarily a human review tool. The app currently provides two explicit actions:

- `Copy for AI`, which copies a prompt + PR metadata + diff payload
- `Review with Claude`, which launches a local terminal workflow for the same payload

The implementation is intentionally explicit rather than ambient:

- payload assembly is centralized in `src/lib/buildReviewPayload.ts`
- prompts are customizable per repo via `src/lib/reviewPrompt.ts`
- launch behavior is user-initiated from `src/components/review/ReviewActions.tsx`
- local Claude launching is handled by Tauri in `src-tauri/src/launch.rs`

The decision is whether AI review should remain a deliberate export/launch surface or become an implicit part of the core review flow.

## Decision

AI review in Lachesi will remain an explicit, opt-in set of user actions layered on top of the normal review workflow.

This means:

- the app builds a reusable review payload, but does not auto-send diffs to an AI service
- the user chooses whether to copy the payload or launch a local Claude session
- repo-specific prompts are supported, but the default human review flow does not depend on them
- AI assistance complements native commenting and publishing rather than replacing them
- GUI AI review starts the model directly and does not rerun repository
  analyzers that belong to the preceding development validation flow

## Do's and Don'ts

### Do

- Keep AI actions clearly labeled and separate from core comment publication
- Reuse one payload-building path for copy and launch flows
- Keep the AI prompt editable per repository
- Prefer local, user-mediated launch flows over hidden background submission
- Send `skipAnalyzers: true` from every GUI `start_inline_review` call

### Don't

- Don't auto-send PR data to an AI provider without an explicit user action
- Don't make AI review a required step before comments can be published
- Don't duplicate prompt-building logic across multiple UI surfaces
- Don't blur the distinction between AI suggestions and actual Bitbucket comments
- Don't rerun lint, test, typecheck, or other local analyzers automatically
  when the user starts GUI AI review

## Consequences

### Positive

- Users keep control over when code leaves the main review surface
- The feature remains useful even when different teammates prefer different terminals or AI tools
- Payload generation stays consistent across copy and launch entry points
- The core product remains understandable as a review tool first, AI helper second
- GUI review latency reflects model execution rather than duplicate repository gates

### Negative

- AI review requires an extra click instead of running automatically
- Local launch workflows introduce per-machine setup such as terminal choice and local Claude availability
- There is no built-in round-trip from AI output back into draft comments yet
- AI review does not independently collect analyzer evidence; validation results
  remain the responsibility of the preceding development flow

### Risks

- The payload can drift from the most useful review context if PR metadata needs evolve
- Terminal or local CLI setup can fail in team environments with mixed tooling
- Users may over-trust AI output if the UI does not keep the feature clearly assistive
- A caller could explicitly send `skipAnalyzers: false` through the IPC contract;
  code review must reject that for GUI review entry points

## Compliance and Enforcement

An automated rule enforces one high-confidence invariant:

- the native `launch_claude_review` command must stay confined to the explicit review action surface (`src/components/review/ReviewActions.tsx`) and the mock IPC layer
- GUI review entry points in `src/App.tsx` and `src/hooks/useAiReview.ts`
  must send `skipAnalyzers: true`

Code review should still reject broader violations that are not yet machine-checked, such as:

- trigger AI review automatically without an explicit user action
- fork the payload format independently for different AI buttons
- conflate AI output with already-published review comments
- omit `skipAnalyzers: true` from a GUI `start_inline_review` invocation

## References

- `src/components/review/ReviewActions.tsx`
- `src/lib/buildReviewPayload.ts`
- `src/lib/reviewPrompt.ts`
- `src-tauri/src/launch.rs`
