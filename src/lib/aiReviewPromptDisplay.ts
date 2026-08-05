const PROMPT_DISPLAY_MARKER = "[[norn:ai-review-prompt]]";
const LEGACY_PROMPT_DISPLAY_MARKER = "[[lachesi:ai-review-prompt]]";

export interface ParsedReviewPromptDisplay {
  intro: string;
  prompt: string;
}

export function buildReviewPromptDisplayMessage(payload: string): string {
  return [
    "Run the standard AI review for this pull request.",
    "",
    PROMPT_DISPLAY_MARKER,
    payload.trim(),
  ].join("\n");
}

export function parseReviewPromptDisplayMessage(content: string): ParsedReviewPromptDisplay | null {
  const marker = content.includes(PROMPT_DISPLAY_MARKER)
    ? PROMPT_DISPLAY_MARKER
    : LEGACY_PROMPT_DISPLAY_MARKER;
  const markerIndex = content.indexOf(marker);
  if (markerIndex < 0) return null;

  const intro = content.slice(0, markerIndex).trim();
  const prompt = content.slice(markerIndex + marker.length).trim();
  if (!prompt) return null;

  return {
    intro: intro || "Run the standard AI review for this pull request.",
    prompt,
  };
}
