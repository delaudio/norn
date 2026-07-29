//! Versioned, offline evaluation of structured review findings.

use std::collections::HashSet;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

const CORPUS_SCHEMA_VERSION: &str = "lachesi.review-evaluation-corpus.v1";
const RESULT_SCHEMA_VERSION: &str = "lachesi.review-evaluation-result.v1";

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

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EvaluationResult {
    pub schema_version: String,
    pub corpus_version: String,
    pub cases: Vec<EvaluationCaseResult>,
    pub metrics: EvaluationMetrics,
    pub regressions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
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
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
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
}

pub fn evaluate(
    corpus: EvaluationCorpus,
    baseline: EvaluationBaseline,
) -> Result<EvaluationResult, String> {
    validate_corpus(&corpus)?;
    validate_baseline(&baseline, &corpus)?;
    let cases = corpus
        .cases
        .iter()
        .map(evaluate_case)
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

pub fn load_and_evaluate(
    corpus_path: &Path,
    baseline_path: &Path,
) -> Result<EvaluationResult, String> {
    let corpus: EvaluationCorpus = read_json(corpus_path)?;
    let baseline: EvaluationBaseline = read_json(baseline_path)?;
    evaluate(corpus, baseline)
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, String> {
    let contents = fs::read_to_string(path)
        .map_err(|error| format!("Failed to read {}: {error}", path.display()))?;
    serde_json::from_str(&contents)
        .map_err(|error| format!("Invalid JSON in {}: {error}", path.display()))
}

fn validate_corpus(corpus: &EvaluationCorpus) -> Result<(), String> {
    if corpus.schema_version != CORPUS_SCHEMA_VERSION {
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
    if baseline.schema_version != RESULT_SCHEMA_VERSION {
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

fn evaluate_case(case: &EvaluationCase) -> Result<EvaluationCaseResult, String> {
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
    for observed in &case.observed {
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
            ExpectedDisposition::NonFinding => unreachable!(),
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
        duration_ms: case.duration_ms,
        expected,
        optional,
        non_findings,
        matched_expected,
        matched_optional,
        unexpected,
        missed_expected: expected.saturating_sub(matched_expected),
        anchor_matches,
    })
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
        average_duration_ms: total_duration_ms / u64::try_from(cases.len()).unwrap_or(1),
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
