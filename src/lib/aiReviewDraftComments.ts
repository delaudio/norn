import { changeKeyForAnchor, parseUnifiedDiff } from "@/lib/diff";
import { jiraBrowseUrl } from "@/lib/jira";
import type {
  AiReviewDraftCommentSuggestion,
  AiReviewThread,
  BranchStatus,
  PullRequestDetail,
  ReviewFinding,
  ReviewFindingRef,
  ReviewPublicationMode,
  ReviewRun,
} from "@/types";

export interface AiReviewCommentDraftPayloadInput {
  pr: PullRequestDetail;
  thread: AiReviewThread;
  reviewRun?: ReviewRun | null;
  branchStatus?: BranchStatus | null;
  rawDiff: string;
  jiraKeys?: string[];
  jiraBaseUrl?: string | null;
  jiraContext?: string | null;
}

export interface NormalizeAiReviewDraftCommentsResult {
  comments: AiReviewDraftCommentSuggestion[];
  skipped: number;
}

export interface LinkedAiReviewDraftComment extends AiReviewDraftCommentSuggestion {
  findingRef: ReviewFindingRef | null;
  publicationMode: ReviewPublicationMode | null;
}

function buildConversationTranscript(thread: AiReviewThread): string {
  return thread.messages
    .map((message) => {
      const speaker = message.role === "assistant" ? "Assistant" : "Reviewer";
      return `### ${speaker}\n${message.content.trim()}`;
    })
    .join("\n\n");
}

function formatFindingAnchor(reviewRun: ReviewRun, index: number): string {
  const finding = reviewRun.findings[index];
  if (!finding?.anchor) return "unanchored";
  const { path, startLine, endLine, side } = finding.anchor;
  const range =
    endLine != null && endLine !== startLine ? `${startLine}-${endLine}` : `${startLine}`;
  return `${path}:${range} (${side})`;
}

function buildStructuredFindingsContext(reviewRun: ReviewRun): string {
  const lines = [
    `Run: ${reviewRun.id}`,
    `Schema: ${reviewRun.schemaVersion}`,
    `Turn: ${reviewRun.turnKind}`,
  ];

  reviewRun.findings.forEach((finding, index) => {
    lines.push("", `### Finding ${index + 1}`);
    lines.push(`- Fingerprint: ${finding.fingerprint}`);
    lines.push(`- Severity: ${finding.severity}`);
    lines.push(`- Category: ${finding.category}`);
    lines.push(`- Confidence: ${finding.confidence}`);
    lines.push(`- Title: ${finding.title}`);
    lines.push(`- Anchor: ${formatFindingAnchor(reviewRun, index)}`);
    lines.push(`- Summary: ${finding.summary}`);
  });

  const resourceEvidence = reviewRun.evidence.filter(
    (evidence) => evidence.kind === "doc" && evidence.payload?.trim(),
  );
  if (resourceEvidence.length > 0) {
    lines.push("", "## Supporting resources");
    for (const evidence of resourceEvidence) {
      const summary = evidence.summary?.trim();
      lines.push(
        summary
          ? `- ${evidence.title}: ${evidence.payload} — ${summary}`
          : `- ${evidence.title}: ${evidence.payload}`,
      );
    }
  }

  return lines.join("\n");
}

function tokenize(text: string): Set<string> {
  return new Set(
    text
      .toLowerCase()
      .split(/[^a-z0-9_]+/i)
      .map((token) => token.trim())
      .filter((token) => token.length >= 4),
  );
}

function keywordOverlapScore(left: string, right: string): number {
  const leftTokens = tokenize(left);
  const rightTokens = tokenize(right);
  let score = 0;
  for (const token of leftTokens) {
    if (rightTokens.has(token)) {
      score += 1;
    }
  }
  return score;
}

function suggestionLine(
  comment: Pick<AiReviewDraftCommentSuggestion, "to" | "from">,
): number | null {
  return comment.to ?? comment.from ?? null;
}

function suggestionSide(
  comment: Pick<AiReviewDraftCommentSuggestion, "to" | "from">,
): "new" | "old" | null {
  if (comment.to != null) return "new";
  if (comment.from != null) return "old";
  return null;
}

function anchorDistance(
  finding: ReviewFinding,
  comment: Pick<AiReviewDraftCommentSuggestion, "path" | "to" | "from">,
): number | null {
  if (!finding.anchor) return null;
  if (finding.anchor.path !== comment.path.trim()) return null;
  if (finding.anchor.side !== suggestionSide(comment)) return null;
  const line = suggestionLine(comment);
  if (line == null) return null;

  const start = finding.anchor.startLine;
  const end = finding.anchor.endLine ?? start;
  if (line >= start && line <= end) return 0;
  if (line < start) return start - line;
  return line - end;
}

function publicationModeForFinding(finding: ReviewFinding): ReviewPublicationMode {
  return finding.anchor ? "inline" : "general";
}

export function linkAiReviewDraftCommentsToFindings(
  reviewRun: ReviewRun | null | undefined,
  comments: AiReviewDraftCommentSuggestion[],
): LinkedAiReviewDraftComment[] {
  if (!reviewRun) {
    return comments.map((comment) => ({
      ...comment,
      findingRef: null,
      publicationMode: null,
    }));
  }

  const usedFindingIds = new Set<string>();

  return comments.map((comment) => {
    const candidates = reviewRun.findings
      .filter((finding) => !usedFindingIds.has(finding.id))
      .map((finding) => {
        const distance = anchorDistance(finding, comment);
        if (distance !== 0) {
          return null;
        }
        const score = 100 + keywordOverlapScore(comment.raw, `${finding.title} ${finding.summary}`);
        return { finding, score };
      })
      .filter(
        (candidate): candidate is { finding: ReviewFinding; score: number } => candidate != null,
      )
      .sort((left, right) => right.score - left.score);

    const match = candidates[0]?.finding ?? null;
    if (match) {
      usedFindingIds.add(match.id);
    }
    const matchLine = match?.anchor?.endLine ?? match?.anchor?.startLine ?? null;
    const anchoredComment =
      match?.anchor && matchLine != null
        ? {
            ...comment,
            path: match.anchor.path,
            to: match.anchor.side === "new" ? matchLine : null,
            from: match.anchor.side === "old" ? matchLine : null,
          }
        : comment;

    return {
      ...anchoredComment,
      findingRef: match
        ? {
            reviewRunId: reviewRun.id,
            findingId: match.id,
            findingFingerprint: match.fingerprint,
          }
        : null,
      publicationMode: match ? publicationModeForFinding(match) : null,
    };
  });
}

export function buildAiReviewCommentDraftPayload({
  pr,
  thread,
  reviewRun,
  branchStatus,
  rawDiff,
  jiraKeys,
  jiraBaseUrl,
  jiraContext,
}: AiReviewCommentDraftPayloadInput): string {
  const hasStructuredFindings = Boolean(reviewRun?.findings.length);
  const lines = [
    hasStructuredFindings
      ? "You are preparing draft Bitbucket PR review comments from normalized AI review findings."
      : "You are preparing draft Bitbucket PR review comments from an AI review conversation.",
    "Convert only concrete, actionable findings into draft inline comments for the changed lines in the diff.",
    hasStructuredFindings
      ? "Treat the structured findings below as the canonical review output. Use the assistant summary only if you need extra wording context."
      : "Use the full review conversation as context, not only the first assistant message.",
    hasStructuredFindings
      ? "Preserve each finding's intent, but tighten the wording into concise inline comments."
      : "If later assistant messages refine or contradict earlier ones, treat the latest assistant position as authoritative.",
    'Return ONLY JSON matching this shape: {"comments":[{"path":"string","to":123|null,"from":123|null,"raw":"string"}]}.',
    "Only emit comments you can anchor confidently to a changed line visible in the diff.",
    "Use `to` for comments on the new side and `from` for comments on the old side. Prefer setting exactly one of them.",
    "Omit approvals, summaries, resources, duplicate findings, and anything that cannot be anchored precisely.",
    "Keep each `raw` concise and ready to post as a review comment. No markdown headings, no resources section, no code fences.",
    "",
    "## Pull request",
    `${pr.title} (#${pr.id})`,
    `Branch: ${pr.sourceBranch} -> ${pr.destinationBranch}`,
  ];

  if (branchStatus && (branchStatus.behind > 0 || branchStatus.ahead > 0)) {
    const behind = `${branchStatus.behind}${branchStatus.behindCapped ? "+" : ""}`;
    const ahead = `${branchStatus.ahead}${branchStatus.aheadCapped ? "+" : ""}`;
    lines.push(`Commits: ${ahead} ahead, ${behind} behind ${pr.destinationBranch}`);
  }

  if (pr.descriptionRaw.trim()) {
    lines.push("", "## Description", pr.descriptionRaw.trim());
  }

  if (jiraKeys && jiraKeys.length > 0) {
    lines.push("", "## Related Jira");
    for (const key of jiraKeys) {
      lines.push(jiraBaseUrl ? `- ${key}: ${jiraBrowseUrl(jiraBaseUrl, key)}` : `- ${key}`);
    }
    if (jiraContext?.trim()) {
      lines.push("", jiraContext.trim());
    }
  }

  if (hasStructuredFindings && reviewRun) {
    lines.push("", "## Structured review findings", buildStructuredFindingsContext(reviewRun));
    if (reviewRun.summaryMarkdown?.trim()) {
      lines.push("", "## Assistant summary", reviewRun.summaryMarkdown.trim());
    }
  } else {
    lines.push("", "## Review conversation", buildConversationTranscript(thread));
  }
  lines.push("", "## Diff", "```diff", rawDiff.trim(), "```");
  return lines.join("\n");
}

export function normalizeAiReviewDraftComments(
  rawDiff: string,
  suggestions: AiReviewDraftCommentSuggestion[],
): NormalizeAiReviewDraftCommentsResult {
  const files = parseUnifiedDiff(rawDiff);
  const fileByPath = new Map<string, (typeof files)[number]>();
  for (const file of files) {
    if (file.newPath) fileByPath.set(file.newPath, file);
    if (file.oldPath) fileByPath.set(file.oldPath, file);
  }

  const seen = new Set<string>();
  const comments: AiReviewDraftCommentSuggestion[] = [];
  let skipped = 0;

  for (const suggestion of suggestions) {
    const path = suggestion.path.trim();
    const raw = suggestion.raw.trim();
    if (!path || !raw) {
      skipped += 1;
      continue;
    }

    const file = fileByPath.get(path);
    if (!file) {
      skipped += 1;
      continue;
    }

    const to = suggestion.to ?? null;
    const from = suggestion.from ?? null;
    if (to == null && from == null) {
      skipped += 1;
      continue;
    }

    if (!changeKeyForAnchor(file, to, from)) {
      skipped += 1;
      continue;
    }

    const dedupeKey = `${path}:${to ?? ""}:${from ?? ""}:${raw}`;
    if (seen.has(dedupeKey)) {
      skipped += 1;
      continue;
    }
    seen.add(dedupeKey);
    comments.push({ path, to, from, raw });
  }

  return { comments, skipped };
}
