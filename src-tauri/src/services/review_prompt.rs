//! Shared review prompt resolution and payload rendering, reused by the
//! headless engine and (later) the stdio backend's inline review session.

use crate::repo_config;

const DEFAULT_REVIEW_PROMPT: &str = include_str!("../../../src/lib/defaultReviewPrompt.md");

/// Resolve the configured review prompt, applying the repository policy
/// `replace`/`extend` options over the bundled default.
pub fn resolve_review_prompt(config: Option<&repo_config::RepoReviewConfig>) -> String {
    let prompt = config
        .and_then(|config| config.review.as_ref())
        .and_then(|review| review.prompt.as_ref())
        .cloned()
        .unwrap_or_default();
    let replacement = prompt
        .replace
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let extension = prompt
        .extend
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let base = replacement.unwrap_or_else(|| DEFAULT_REVIEW_PROMPT.trim().to_string());
    match extension {
        Some(extension) => format!("{base}\n\n## Repository review policy\n{extension}"),
        None => base,
    }
}

/// Render a review payload with the target header and a fenced diff. The fence
/// is widened past any backtick run in the diff so the code block cannot break.
#[allow(clippy::too_many_arguments)]
pub fn build_review_payload(
    prompt: &str,
    title: &str,
    scope_label: &str,
    scope_note: &str,
    source: &str,
    destination: &str,
    diff: &str,
) -> String {
    let fence = markdown_fence(diff);
    let opening_fence = format!("{fence}diff");
    let mut payload = [
        prompt.trim(),
        "",
        "## Review target",
        &format!("{title} ({scope_label})"),
        &format!("Branch: {source} -> {destination}"),
        scope_note,
        "",
        "## Diff",
        &opening_fence,
    ]
    .join("\n");
    payload.push('\n');
    payload.push_str(diff);
    if !diff.ends_with('\n') {
        payload.push('\n');
    }
    payload.push_str(&fence);
    payload
}

pub fn markdown_fence(content: &str) -> String {
    let max_run = content
        .split(|character| character != '`')
        .map(str::len)
        .max()
        .unwrap_or(0);
    "`".repeat(max_run.saturating_add(1).max(3))
}

/// Build the native fix prompt from the full persisted conversation.
pub(crate) fn build_fix_payload(
    pr_id: u32,
    source: &str,
    destination: &str,
    thread: &crate::services::review::AiReviewThread,
    diff: &str,
) -> String {
    let mut lines = vec![
        "You are a senior software engineer fixing actionable findings from an AI pull request review.".to_string(),
        "Work in the current git branch and make the minimum code changes needed to address valid issues.".to_string(),
        "You may inspect and edit files, and run local checks if useful.".to_string(),
        "Do not create commits, do not push, and do not change git history.".to_string(),
        "Use the full review conversation as context, not only the first review message.".to_string(),
        "If later assistant messages refine or contradict earlier ones, treat the latest assistant position as authoritative.".to_string(),
        "If a finding is incorrect or not actionable, leave the code unchanged for that item and explain it briefly in the summary.".to_string(),
        r#"Return ONLY JSON matching this shape: {"status":"success|failed","summary":"string","commitMessage":"string","tests":["..."],"filesTouched":["..."],"failureReason":"string?"}."#.to_string(),
        format!("\n## Pull request\nPull request #{pr_id}\nBranch: {source} -> {destination}\n\n## Review conversation"),
    ];
    for message in &thread.messages {
        let speaker = if message.role == crate::services::review::AiReviewMessageRole::Assistant {
            "Assistant"
        } else {
            "Reviewer"
        };
        lines.push(format!("### {speaker}\n{}", message.content.trim()));
    }
    lines.push(format!("\n## Diff\n```diff\n{}\n```", diff.trim()));
    lines.join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repo_config::{PromptConfig, RepoReviewConfig, ReviewConfig};

    fn config_with(prompt: PromptConfig) -> RepoReviewConfig {
        RepoReviewConfig {
            review: Some(ReviewConfig {
                prompt: Some(prompt),
                ..ReviewConfig::default()
            }),
            ..RepoReviewConfig::default()
        }
    }

    #[test]
    fn resolves_the_default_prompt_without_configuration() {
        let resolved = resolve_review_prompt(None);
        assert!(!resolved.is_empty());
        assert!(!resolved.contains("## Repository review policy"));
    }

    #[test]
    fn repository_policy_extends_the_default_prompt() {
        let resolved = resolve_review_prompt(Some(&config_with(PromptConfig {
            extend: Some("Only flag security issues.".to_string()),
            replace: None,
        })));
        assert!(resolved.contains("## Repository review policy"));
        assert!(resolved.contains("Only flag security issues."));
    }

    #[test]
    fn repository_policy_can_replace_the_prompt() {
        let resolved = resolve_review_prompt(Some(&config_with(PromptConfig {
            extend: None,
            replace: Some("Custom prompt".to_string()),
        })));
        assert_eq!(resolved, "Custom prompt");
    }

    #[test]
    fn payload_widens_the_fence_past_backtick_runs() {
        let diff = "```\n+code\n```";
        let payload =
            build_review_payload("Prompt", "Title", "pr", "note", "feature", "main", diff);
        assert!(payload.contains("````diff"), "payload: {payload}");
        assert!(payload.ends_with("````"), "payload: {payload}");
    }

    #[test]
    fn payload_includes_target_and_diff() {
        let payload = build_review_payload(
            "Prompt", "Title", "pr", "note", "feature", "main", "+line\n",
        );
        assert!(payload.starts_with("Prompt\n"));
        assert!(payload.contains("Title (pr)"));
        assert!(payload.contains("Branch: feature -> main"));
        assert!(payload.contains("+line"));
    }
}
