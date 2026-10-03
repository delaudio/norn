//! Versioned review quality evaluation for a synthetic, sanitized corpus.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config::AiProvider;
use crate::repo_config;
use crate::services::review::{
    run_headless_review_native, HeadlessNativeReviewError, HeadlessNativeReviewRequest,
    ReviewAnchorSide, ReviewFindingAnchor, ReviewProvider, ReviewRun,
};

const CORPUS_SCHEMA_VERSION: &str = "norn.review-evaluation-corpus.v1";
const LEGACY_CORPUS_SCHEMA_VERSION: &str = "lachesi.review-evaluation-corpus.v1";
const RESULT_SCHEMA_VERSION: &str = "norn.review-evaluation-result.v1";
const LEGACY_RESULT_SCHEMA_VERSION: &str = "lachesi.review-evaluation-result.v1";
const DEFAULT_REVIEW_PROMPT: &str = include_str!("../../src/lib/defaultReviewPrompt.md");
const REVIEW_BOUNDARY: &str = "## Headless reviewer boundary\n\nReview only the supplied policy, context, evidence, and diff. Do not inspect the filesystem or run commands.";
const MINIMAL_REVIEW_PROMPT: &str =
    "You are a strict code review model. For each issue in the diff, return JSON matching norn.review.v1. If no issues, return an empty findings array.\n\n```json\n{\n  \"schemaVersion\": \"norn.review.v1\",\n  \"findings\": []\n}\n```";

#[derive(Debug, Clone, PartialEq)]
pub struct EvaluationLiveOptions {
    pub repo_path: PathBuf,
    pub corpus_root: PathBuf,
    pub allow_provider_diff: bool,
    pub prompt_profile: EvaluationPromptProfile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvaluationPromptProfile {
    Default,
    Minimal,
}

impl EvaluationPromptProfile {
    pub fn as_str(self) -> &'static str {
        match self {
            EvaluationPromptProfile::Default => "default",
            EvaluationPromptProfile::Minimal => "minimal",
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EvaluationCorpus {
    pub schema_version: String,
    pub corpus_version: String,
    pub cases: Vec<EvaluationCase>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EvaluationCase {
    pub id: String,
    pub area: String,
    pub diff_path: String,
    pub provider: String,
    pub model: String,
    pub config_version: String,
    pub duration_ms: u64,
    #[serde(default)]
    pub expected: Vec<ExpectedFinding>,
    #[serde(default)]
    pub observed: Vec<ObservedFinding>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExpectedFinding {
    pub id: String,
    pub disposition: ExpectedDisposition,
    pub anchor: EvaluationAnchor,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ExpectedDisposition {
    Expected,
    Optional,
    NonFinding,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ObservedFinding {
    pub expectation_id: Option<String>,
    pub anchor: EvaluationAnchor,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EvaluationAnchor {
    pub path: String,
    pub line: u32,
    pub side: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EvaluationBaseline {
    pub schema_version: String,
    pub corpus_version: String,
    pub minimum_precision_milli: u32,
    pub maximum_missed_expected: u32,
    pub minimum_anchor_accuracy_milli: u32,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EvaluationResult {
    pub schema_version: String,
    pub corpus_version: String,
    pub cases: Vec<EvaluationCaseResult>,
    pub metrics: EvaluationMetrics,
    pub regressions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EvaluationCaseResult {
    pub id: String,
    pub area: String,
    pub provider: String,
    pub model: String,
    pub config_version: String,
    pub duration_ms: u64,
    pub expected: u32,
    pub optional: u32,
    pub non_findings: u32,
    pub matched_expected: u32,
    pub matched_optional: u32,
    pub unexpected: u32,
    pub missed_expected: u32,
    pub anchor_matches: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub live_execution: Option<EvaluationLiveExecution>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EvaluationLiveExecution {
    pub status: String,
    pub provider: String,
    pub model: String,
    pub config_version: String,
    pub prompt_profile: String,
    pub pipeline_version: String,
    pub duration_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost: Option<Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub review_run: Option<Value>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EvaluationMetrics {
    pub observed_findings: u32,
    pub matched_expected: u32,
    pub matched_optional: u32,
    pub false_positives: u32,
    pub missed_expected: u32,
    pub anchor_matches: u32,
    pub anchor_candidates: u32,
    pub precision_milli: u32,
    pub anchor_accuracy_milli: u32,
    pub total_duration_ms: u64,
    pub average_duration_ms: u64,
    pub execution_failures: u32,
}

#[derive(Debug, Clone)]
struct LiveCaseReview {
    observed: Vec<ObservedFinding>,
    duration_ms: u64,
    execution: EvaluationLiveExecution,
}

pub fn evaluate(
    corpus: EvaluationCorpus,
    baseline: EvaluationBaseline,
) -> Result<EvaluationResult, String> {
    evaluate_with_live(corpus, baseline, None)
}

pub fn evaluate_live(
    corpus: EvaluationCorpus,
    baseline: EvaluationBaseline,
    options: EvaluationLiveOptions,
) -> Result<EvaluationResult, String> {
    evaluate_with_live(corpus, baseline, Some(options))
}

pub fn load_and_evaluate(
    corpus_path: &Path,
    baseline_path: &Path,
) -> Result<EvaluationResult, String> {
    let corpus: EvaluationCorpus = read_json(corpus_path)?;
    let baseline: EvaluationBaseline = read_json(baseline_path)?;
    evaluate(corpus, baseline)
}

pub fn load_and_evaluate_live(
    corpus_path: &Path,
    baseline_path: &Path,
    options: EvaluationLiveOptions,
) -> Result<EvaluationResult, String> {
    let corpus: EvaluationCorpus = read_json(corpus_path)?;
    let baseline: EvaluationBaseline = read_json(baseline_path)?;
    evaluate_live(corpus, baseline, options)
}

fn evaluate_with_live(
    corpus: EvaluationCorpus,
    baseline: EvaluationBaseline,
    live_options: Option<EvaluationLiveOptions>,
) -> Result<EvaluationResult, String> {
    validate_corpus(&corpus)?;
    validate_baseline(&baseline, &corpus)?;
    let cases = corpus
        .cases
        .iter()
        .map(|case| match live_options.as_ref() {
            Some(options) => evaluate_live_case(case, options),
            None => Ok(evaluate_offline_case(case)),
        })
        .collect::<Result<Vec<_>, _>>()?;
    let metrics = aggregate_metrics(&cases);
    let regressions = baseline_regressions(&metrics, &baseline);
    Ok(EvaluationResult {
        schema_version: RESULT_SCHEMA_VERSION.to_string(),
        corpus_version: corpus.corpus_version,
        cases,
        metrics,
        regressions,
    })
}

fn evaluate_live_case(
    case: &EvaluationCase,
    options: &EvaluationLiveOptions,
) -> Result<EvaluationCaseResult, String> {
    if !options.allow_provider_diff {
        return Err(
            "`--live` requires `--allow-provider-diff` because this mode sends the corpus diff to a provider."
                .to_string(),
        );
    }
    let live_data = run_case_live_review(case, options)?;
    let mut result = evaluate_case(case, &live_data.observed, live_data.duration_ms)?;
    result.live_execution = Some(live_data.execution);
    Ok(result)
}

fn evaluate_offline_case(case: &EvaluationCase) -> EvaluationCaseResult {
    evaluate_case(case, &case.observed, case.duration_ms)
        .expect("offline evaluation cases are prevalidated")
}

fn evaluate_case(
    case: &EvaluationCase,
    observed: &[ObservedFinding],
    duration_ms: u64,
) -> Result<EvaluationCaseResult, String> {
    let expected_ids = case
        .expected
        .iter()
        .map(|expected| (expected.id.as_str(), expected))
        .collect::<std::collections::HashMap<_, _>>();
    let mut matched = HashSet::new();
    let mut matched_expected = 0;
    let mut matched_optional = 0;
    let mut unexpected = 0;
    let mut anchor_matches = 0;
    for observed in observed {
        let Some(id) = observed.expectation_id.as_deref() else {
            unexpected += 1;
            continue;
        };
        let Some(expected) = expected_ids.get(id) else {
            unexpected += 1;
            continue;
        };
        if !matched.insert(id) || expected.disposition == ExpectedDisposition::NonFinding {
            unexpected += 1;
            continue;
        }
        if observed.anchor == expected.anchor {
            anchor_matches += 1;
        }
        match expected.disposition {
            ExpectedDisposition::Expected => matched_expected += 1,
            ExpectedDisposition::Optional => matched_optional += 1,
            ExpectedDisposition::NonFinding => {
                unreachable!("nonfinding was handled above");
            }
        }
    }
    let expected = case
        .expected
        .iter()
        .filter(|finding| finding.disposition == ExpectedDisposition::Expected)
        .count() as u32;
    let optional = case
        .expected
        .iter()
        .filter(|finding| finding.disposition == ExpectedDisposition::Optional)
        .count() as u32;
    let non_findings = case
        .expected
        .iter()
        .filter(|finding| finding.disposition == ExpectedDisposition::NonFinding)
        .count() as u32;
    Ok(EvaluationCaseResult {
        id: case.id.clone(),
        area: case.area.clone(),
        provider: case.provider.clone(),
        model: case.model.clone(),
        config_version: case.config_version.clone(),
        duration_ms,
        expected,
        optional,
        non_findings,
        matched_expected,
        matched_optional,
        unexpected,
        missed_expected: expected.saturating_sub(matched_expected),
        anchor_matches,
        live_execution: None,
    })
}

fn run_case_live_review(
    case: &EvaluationCase,
    options: &EvaluationLiveOptions,
) -> Result<LiveCaseReview, String> {
    let ai_provider = match case.provider.to_ascii_lowercase().as_str() {
        "codex" => AiProvider::Codex,
        "claude" => AiProvider::Claude,
        value => {
            return Err(format!(
                "Case `{}` has unsupported provider `{}`. Use `codex` or `claude`.",
                case.id, value
            ));
        }
    };
    let model = case.model.trim();
    if model.is_empty() {
        return Err(format!("Case `{}` has an empty model", case.id));
    }

    let diff_path = options.corpus_root.join(&case.diff_path);
    let diff = fs::read_to_string(&diff_path).map_err(|error| {
        format!(
            "Case `{}` missing synthetic diff `{}`: {error}",
            case.id,
            diff_path.display()
        )
    })?;
    if diff.trim().is_empty() {
        return Err(format!(
            "Case `{}` diff `{}` is empty",
            case.id, case.diff_path
        ));
    }

    let prompt = synthetic_review_prompt(
        &options.repo_path,
        options.prompt_profile,
        case.config_version.as_str(),
    )?;
    let payload = build_synthetic_review_payload(
        &format!("{}\n\n{}", prompt, REVIEW_BOUNDARY),
        &format!("Evaluation case {}", case.id),
        "evaluation/base",
        "evaluation/target",
        &diff,
    );

    let config_result = repo_config::load_from_repo_path_with_profile(
        &options.repo_path,
        Some(&case.config_version),
    )
    .map_err(|error| format!("Case `{}` failed to load repo config: {error}", case.id))?;
    if !config_result.errors.is_empty() {
        return Err(format!(
            "Case `{}` has invalid repository config: {}",
            case.id,
            config_result
                .errors
                .into_iter()
                .map(|message| message.message)
                .collect::<Vec<_>>()
                .join("; ")
        ));
    }

    let review_profile = config_result.selected_profile.or_else(|| {
        config_result.config.as_ref().and_then(|config| {
            config
                .review
                .as_ref()
                .and_then(|review| review.profile.clone())
        })
    });

    let request = HeadlessNativeReviewRequest {
        repo_path: options.repo_path.clone(),
        review_provider: ReviewProvider::Github,
        workspace: "local".to_string(),
        repo: "norn-evaluation".to_string(),
        pr_id: case_pr_id(&case.id),
        title: format!("Review evaluation case {}", case.id),
        source_branch: "evaluation/base".to_string(),
        destination_branch: "evaluation/target".to_string(),
        reviewed_base_sha: None,
        reviewed_head_sha: None,
        payload,
        ai_provider,
        claude_model: (ai_provider == AiProvider::Claude).then(|| case.model.trim().to_string()),
        claude_effort: None,
        codex_model: (ai_provider == AiProvider::Codex).then(|| case.model.trim().to_string()),
        codex_effort: None,
        review_profile,
        policy_sources: Vec::new(),
        required_policy_analyzers: Vec::new(),
        resolved_policy_config: config_result.config,
        organization_policy_checked: true,
        run_analyzers: false,
    };

    let (duration_ms, observed, execution) = match run_headless_review_native(request) {
        Ok(review_run) => {
            let duration_ms = review_run_duration_ms(&review_run);
            let observed = observations_from_review_run(case, &review_run);
            let review_run = sanitize_review_run(review_run);
            let review_run = serde_json::to_value(review_run).map_err(|error| {
                format!(
                    "Case `{}` failed to serialize review artifacts: {error}",
                    case.id
                )
            })?;
            (
                duration_ms,
                observed,
                EvaluationLiveExecution {
                    status: "succeeded".to_string(),
                    provider: case.provider.clone(),
                    model: case.model.clone(),
                    config_version: case.config_version.clone(),
                    prompt_profile: options.prompt_profile.as_str().to_string(),
                    pipeline_version: env!("CARGO_PKG_VERSION").to_string(),
                    duration_ms,
                    runtime_error: None,
                    usage: None,
                    cost: None,
                    warnings: Vec::new(),
                    review_run: Some(review_run),
                },
            )
        }
        Err(error) => {
            let (status, runtime_error) = headless_review_error(error);
            (
                0,
                Vec::new(),
                EvaluationLiveExecution {
                    status,
                    provider: case.provider.clone(),
                    model: case.model.clone(),
                    config_version: case.config_version.clone(),
                    prompt_profile: options.prompt_profile.as_str().to_string(),
                    pipeline_version: env!("CARGO_PKG_VERSION").to_string(),
                    duration_ms: 0,
                    runtime_error: Some(runtime_error),
                    usage: None,
                    cost: None,
                    warnings: Vec::new(),
                    review_run: None,
                },
            )
        }
    };

    Ok(LiveCaseReview {
        observed,
        duration_ms,
        execution,
    })
}

fn headless_review_error(error: HeadlessNativeReviewError) -> (String, String) {
    match error {
        HeadlessNativeReviewError::Analyzer(message)
        | HeadlessNativeReviewError::Provider(message)
        | HeadlessNativeReviewError::Internal(message) => ("failed".to_string(), message),
        HeadlessNativeReviewError::Cancelled => (
            "cancelled".to_string(),
            "Review execution was cancelled before completion.".to_string(),
        ),
    }
}

fn observations_from_review_run(
    case: &EvaluationCase,
    review_run: &ReviewRun,
) -> Vec<ObservedFinding> {
    let mut matching_expected: HashMap<(String, u32, String), Vec<&str>> = HashMap::new();
    for expected in &case.expected {
        let key = (
            expected.anchor.path.clone(),
            expected.anchor.line,
            expected.anchor.side.clone(),
        );
        matching_expected
            .entry(key)
            .or_default()
            .push(expected.id.as_str());
    }

    review_run
        .findings
        .iter()
        .map(|finding| {
            let anchor = finding
                .anchor
                .as_ref()
                .map(|anchor| to_observed_anchor(case, anchor))
                .unwrap_or_else(|| EvaluationAnchor {
                    path: "unanchored".to_string(),
                    line: 0,
                    side: "new".to_string(),
                });

            let expectation_id = finding.anchor.as_ref().and_then(|anchor| {
                matching_expected
                    .get_mut(&(
                        anchor.path.clone(),
                        anchor.start_line,
                        anchor_side(anchor).to_string(),
                    ))
                    .and_then(|ids| {
                        if ids.is_empty() {
                            None
                        } else {
                            Some(ids.remove(0).to_string())
                        }
                    })
            });
            ObservedFinding {
                expectation_id,
                anchor,
            }
        })
        .collect()
}

fn to_observed_anchor(_case: &EvaluationCase, anchor: &ReviewFindingAnchor) -> EvaluationAnchor {
    EvaluationAnchor {
        path: anchor.path.clone(),
        line: anchor.start_line,
        side: anchor_side(anchor).to_string(),
    }
}

fn anchor_side(anchor: &ReviewFindingAnchor) -> &'static str {
    match anchor.side {
        ReviewAnchorSide::New => "new",
        ReviewAnchorSide::Old => "old",
    }
}

fn review_run_duration_ms(review_run: &ReviewRun) -> u64 {
    let started = review_run.created_at.parse::<u64>().ok().unwrap_or(0);
    let finished = review_run
        .finished_at
        .as_deref()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(started);
    if finished > started {
        finished - started
    } else {
        0
    }
}

fn sanitize_review_run(mut review_run: ReviewRun) -> ReviewRun {
    review_run
        .evidence
        .iter_mut()
        .for_each(|evidence| evidence.payload = None);
    review_run
}

fn synthetic_review_prompt(
    repo_path: &Path,
    profile: EvaluationPromptProfile,
    _config_version: &str,
) -> Result<String, String> {
    let config_result = repo_config::load_from_repo_path_with_profile(repo_path, None)
        .map_err(|error| format!("Failed to load corpus repository config: {error}"))?;
    let prompt_override = config_result
        .config
        .as_ref()
        .and_then(|config| config.review.as_ref())
        .and_then(|review| review.prompt.as_ref());
    let replacement = prompt_override
        .and_then(|prompt| prompt.replace.as_ref())
        .map(|value| value.trim())
        .filter(|value| !value.is_empty());
    let extension = prompt_override
        .and_then(|prompt| prompt.extend.as_ref())
        .map(|value| value.trim())
        .filter(|value| !value.is_empty());
    let mut prompt = match profile {
        EvaluationPromptProfile::Default => replacement
            .map(ToString::to_string)
            .unwrap_or_else(|| DEFAULT_REVIEW_PROMPT.trim().to_string()),
        EvaluationPromptProfile::Minimal => MINIMAL_REVIEW_PROMPT.to_string(),
    };
    if let Some(extension) = extension {
        prompt = format!("{prompt}\n\n## Repository review policy\n{extension}");
    }
    Ok(prompt)
}

fn build_synthetic_review_payload(
    prompt: &str,
    title: &str,
    source: &str,
    destination: &str,
    diff: &str,
) -> String {
    let fence = markdown_fence(diff);
    let opening_fence = format!("{fence}diff");
    let scope_note = "This target contains a synthetic corpus case.";
    let mut payload = [
        prompt.trim(),
        "",
        "## Review target",
        &format!("{title}"),
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

fn markdown_fence(content: &str) -> String {
    let max_run = content
        .split(|character| character != '`')
        .map(str::len)
        .max()
        .unwrap_or(0);
    "`".repeat(max_run.saturating_add(1).max(3))
}

fn case_pr_id(case_id: &str) -> u32 {
    let hash = case_id.as_bytes().iter().fold(0x811c9dc5_u32, |acc, byte| {
        acc.wrapping_mul(0x01000193).wrapping_add(u32::from(*byte))
    });
    if hash == 0 {
        1
    } else {
        hash
    }
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, String> {
    let contents = fs::read_to_string(path)
        .map_err(|error| format!("Failed to read {}: {error}", path.display()))?;
    serde_json::from_str(&contents)
        .map_err(|error| format!("Invalid JSON in {}: {error}", path.display()))
}

fn validate_corpus(corpus: &EvaluationCorpus) -> Result<(), String> {
    if ![CORPUS_SCHEMA_VERSION, LEGACY_CORPUS_SCHEMA_VERSION]
        .contains(&corpus.schema_version.as_str())
    {
        return Err(format!(
            "Unsupported evaluation corpus schema `{}`",
            corpus.schema_version
        ));
    }
    if corpus.corpus_version.trim().is_empty() || corpus.cases.is_empty() {
        return Err("Evaluation corpus must have a version and at least one case".to_string());
    }
    let mut ids = HashSet::new();
    for case in &corpus.cases {
        if case.id.trim().is_empty() || !ids.insert(&case.id) {
            return Err("Evaluation case ids must be non-empty and unique".to_string());
        }
        if case.diff_path.trim().is_empty()
            || case.provider.trim().is_empty()
            || case.model.trim().is_empty()
            || case.config_version.trim().is_empty()
        {
            return Err(format!(
                "Evaluation case `{}` has incomplete provenance",
                case.id
            ));
        }
        let mut expected_ids = HashSet::new();
        for expected in &case.expected {
            if expected.id.trim().is_empty() || !expected_ids.insert(&expected.id) {
                return Err(format!(
                    "Evaluation case `{}` has duplicate expected ids",
                    case.id
                ));
            }
        }
    }
    Ok(())
}

fn validate_baseline(
    baseline: &EvaluationBaseline,
    corpus: &EvaluationCorpus,
) -> Result<(), String> {
    if ![RESULT_SCHEMA_VERSION, LEGACY_RESULT_SCHEMA_VERSION]
        .contains(&baseline.schema_version.as_str())
    {
        return Err(format!(
            "Unsupported evaluation baseline schema `{}`",
            baseline.schema_version
        ));
    }
    if baseline.corpus_version != corpus.corpus_version {
        return Err("Evaluation baseline must target the loaded corpus version".to_string());
    }
    if baseline.minimum_precision_milli > 1000 || baseline.minimum_anchor_accuracy_milli > 1000 {
        return Err("Evaluation baseline ratios must be between 0 and 1000".to_string());
    }
    Ok(())
}

fn aggregate_metrics(cases: &[EvaluationCaseResult]) -> EvaluationMetrics {
    let observed_findings = cases
        .iter()
        .map(|case| case.matched_expected + case.matched_optional + case.unexpected)
        .sum::<u32>();
    let matched_expected = cases.iter().map(|case| case.matched_expected).sum::<u32>();
    let matched_optional = cases.iter().map(|case| case.matched_optional).sum::<u32>();
    let false_positives = cases.iter().map(|case| case.unexpected).sum::<u32>();
    let missed_expected = cases.iter().map(|case| case.missed_expected).sum::<u32>();
    let anchor_matches = cases.iter().map(|case| case.anchor_matches).sum::<u32>();
    let anchor_candidates = matched_expected + matched_optional;
    let total_duration_ms = cases.iter().map(|case| case.duration_ms).sum::<u64>();
    let execution_failures = cases
        .iter()
        .filter(|case| {
            case.live_execution
                .as_ref()
                .is_some_and(|execution| execution.status != "succeeded")
        })
        .count() as u32;
    EvaluationMetrics {
        observed_findings,
        matched_expected,
        matched_optional,
        false_positives,
        missed_expected,
        anchor_matches,
        anchor_candidates,
        precision_milli: ratio_milli(matched_expected + matched_optional, observed_findings),
        anchor_accuracy_milli: ratio_milli(anchor_matches, anchor_candidates),
        total_duration_ms,
        average_duration_ms: if cases.is_empty() {
            0
        } else {
            total_duration_ms / u64::try_from(cases.len()).unwrap_or(1)
        },
        execution_failures,
    }
}

fn ratio_milli(numerator: u32, denominator: u32) -> u32 {
    if denominator == 0 {
        1000
    } else {
        numerator.saturating_mul(1000) / denominator
    }
}

fn baseline_regressions(metrics: &EvaluationMetrics, baseline: &EvaluationBaseline) -> Vec<String> {
    let mut regressions = Vec::new();
    if metrics.execution_failures > 0 {
        regressions.push("evaluation_case_execution_failure".to_string());
    }
    if metrics.precision_milli < baseline.minimum_precision_milli {
        regressions.push("precision_below_baseline".to_string());
    }
    if metrics.missed_expected > baseline.maximum_missed_expected {
        regressions.push("missed_expected_above_baseline".to_string());
    }
    if metrics.anchor_accuracy_milli < baseline.minimum_anchor_accuracy_milli {
        regressions.push("anchor_accuracy_below_baseline".to_string());
    }
    regressions
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn corpus() -> EvaluationCorpus {
        serde_json::from_value(serde_json::json!({
            "schemaVersion": CORPUS_SCHEMA_VERSION,
            "corpusVersion": "2026.1",
            "cases": [{
                "id": "clean-diff", "area": "rust", "diffPath": "cases/clean.diff",
                "provider": "codex", "model": "gpt-5", "configVersion": "v1", "durationMs": 42,
                "expected": [
                    {"id": "must-find", "disposition": "expected", "anchor": {"path": "src/lib.rs", "line": 4, "side": "new"}},
                    {"id": "do-not-flag", "disposition": "nonfinding", "anchor": {"path": "src/lib.rs", "line": 8, "side": "new"}}
                ],
                "observed": [
                    {"expectationId": "must-find", "anchor": {"path": "src/lib.rs", "line": 4, "side": "new"}},
                    {"expectationId": "do-not-flag", "anchor": {"path": "src/lib.rs", "line": 8, "side": "new"}}
                ]
            }]
        })).expect("corpus")
    }

    fn baseline() -> EvaluationBaseline {
        EvaluationBaseline {
            schema_version: RESULT_SCHEMA_VERSION.to_string(),
            corpus_version: "2026.1".to_string(),
            minimum_precision_milli: 600,
            maximum_missed_expected: 0,
            minimum_anchor_accuracy_milli: 1000,
        }
    }

    #[test]
    fn distinguishes_expected_non_findings_and_missed_findings() {
        let result = evaluate(corpus(), baseline()).expect("evaluate");
        assert_eq!(result.metrics.matched_expected, 1);
        assert_eq!(result.metrics.false_positives, 1);
        assert_eq!(result.metrics.missed_expected, 0);
        assert_eq!(result.metrics.precision_milli, 500);
        assert_eq!(result.regressions, vec!["precision_below_baseline"]);
    }

    #[test]
    fn accepts_legacy_inputs_but_writes_the_canonical_result_schema() {
        let mut corpus = corpus();
        corpus.schema_version = LEGACY_CORPUS_SCHEMA_VERSION.to_string();
        let mut baseline = baseline();
        baseline.schema_version = LEGACY_RESULT_SCHEMA_VERSION.to_string();

        let result = evaluate(corpus, baseline).expect("legacy evaluation input");

        assert_eq!(result.schema_version, RESULT_SCHEMA_VERSION);
    }

    #[test]
    fn rejects_baselines_for_another_corpus_version() {
        let mut invalid = baseline();
        invalid.corpus_version = "other".to_string();
        assert!(evaluate(corpus(), invalid).is_err());
    }

    #[test]
    fn checked_in_corpus_meets_its_explicit_baseline() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let result = load_and_evaluate(
            &root.join("fixtures/review-evaluation/v1/corpus.json"),
            &root.join("fixtures/review-evaluation/v1/baseline.json"),
        )
        .expect("evaluate checked-in corpus");
        assert!(result.regressions.is_empty());
        assert_eq!(result.cases.len(), 8);
    }
}
