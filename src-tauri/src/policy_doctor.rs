//! Read-only policy coverage analysis for `norn policy doctor`.
//!
//! The analysis inspects the effective repository configuration that Norn would
//! use for a review and reports which review categories are covered, which are
//! missing, and which example packs would fill the gaps. It never calls a model
//! or a provider and never reads secret material.

use crate::repo_config::{
    PolicyConfig, RepoConfigValidationMessage, RepoReviewConfig, RepoReviewConfigLoadResult,
};
use serde::Serialize;

pub const POLICY_DOCTOR_SCHEMA_VERSION: &str = "norn.policy-doctor.v1";

#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PolicyDoctorGap {
    pub category: String,
    pub message: String,
    pub suggested_pack: Option<String>,
}

#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PolicyDoctorReport {
    pub schema_version: &'static str,
    pub coverage: &'static str,
    pub score: u32,
    pub covered: Vec<String>,
    pub missing: Vec<PolicyDoctorGap>,
    pub suggested_packs: Vec<String>,
    pub rule_count: usize,
    pub path_rule_count: usize,
    pub ast_rule_count: usize,
    pub analyzer_count: usize,
    pub profile_count: usize,
    pub loaded_policy_packs: Vec<String>,
    pub warnings: Vec<RepoConfigValidationMessage>,
    pub errors: Vec<RepoConfigValidationMessage>,
}

struct Signal {
    id: &'static str,
    label: &'static str,
    covered: bool,
    gap: &'static str,
    suggested_pack: Option<&'static str>,
}

fn tokens(text: &str) -> impl Iterator<Item = &str> {
    text.split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|token| !token.is_empty())
}

fn mentions(text: &str, needles: &[&str]) -> bool {
    tokens(text).any(|token| needles.contains(&token))
}

pub fn analyze(result: &RepoReviewConfigLoadResult) -> PolicyDoctorReport {
    let config: Option<&RepoReviewConfig> = result.config.as_ref();
    let policy: Option<&PolicyConfig> = config.and_then(|config| config.policy.as_ref());
    let rules = policy.map(|policy| policy.rules.as_slice()).unwrap_or(&[]);
    let path_rules = policy
        .map(|policy| policy.path_rules.as_slice())
        .unwrap_or(&[]);
    let ast_rules = policy
        .map(|policy| policy.ast_rules.as_slice())
        .unwrap_or(&[]);
    let suppressions = policy
        .map(|policy| policy.suppressions.as_slice())
        .unwrap_or(&[]);

    let mut all_text = String::new();
    for rule in rules {
        all_text.push_str(&rule.id.to_lowercase());
        all_text.push(' ');
        all_text.push_str(&rule.instruction.to_lowercase());
        all_text.push(' ');
    }
    for rule in path_rules {
        all_text.push_str(&rule.id.to_lowercase());
        all_text.push(' ');
        all_text.push_str(&rule.instruction.to_lowercase());
        all_text.push(' ');
    }
    for rule in ast_rules {
        all_text.push_str(&rule.id.to_lowercase());
        all_text.push(' ');
        all_text.push_str(&rule.instruction.to_lowercase());
        all_text.push(' ');
    }

    let analyzers = config.map(|config| &config.analyzers);
    let analyzer_count = analyzers.map(|analyzers| analyzers.len()).unwrap_or(0);
    let analyzer_matches = |needles: &[&str]| {
        analyzers
            .map(|analyzers| {
                analyzers.iter().any(|(id, analyzer)| {
                    let id = id.to_lowercase();
                    let command = analyzer
                        .command
                        .as_deref()
                        .unwrap_or_default()
                        .to_lowercase();
                    needles.contains(&id.as_str())
                        || needles.iter().any(|needle| command.contains(needle))
                })
            })
            .unwrap_or(false)
    };

    let profile_count = config.map(|config| config.profiles.len()).unwrap_or(0);
    let pack_count = result.loaded_policy_packs.len();

    let signals = vec![
        Signal {
            id: "security",
            label: "security rules",
            covered: mentions(
                &all_text,
                &[
                    "security",
                    "secure",
                    "secret",
                    "secrets",
                    "credential",
                    "credentials",
                    "token",
                    "tokens",
                    "injection",
                    "auth",
                    "authentication",
                    "authorization",
                    "vulnerability",
                ],
            ),
            gap: "No rule covers secrets, credentials, injection, or auth risks.",
            suggested_pack: Some("agentic-code"),
        },
        Signal {
            id: "architecture",
            label: "architecture and boundary rules",
            covered: mentions(
                &all_text,
                &[
                    "arch",
                    "architecture",
                    "boundary",
                    "layer",
                    "layering",
                    "contract",
                    "contracts",
                    "api",
                    "apis",
                    "compatibility",
                    "ipc",
                ],
            ),
            gap: "No architecture, boundary, or API-contract rule is configured.",
            suggested_pack: Some("bitbucket-tauri-basic"),
        },
        Signal {
            id: "testing",
            label: "testing rules",
            covered: mentions(&all_text, &["test", "tests", "testing", "coverage"])
                || analyzer_matches(&["test"]),
            gap: "No testing rule or test analyzer is configured.",
            suggested_pack: Some("typescript-basic"),
        },
        Signal {
            id: "dependencies",
            label: "dependency rules",
            covered: mentions(
                &all_text,
                &["depend", "dependency", "dependencies", "lockfile", "supply"],
            ),
            gap: "No dependency-drift coverage.",
            suggested_pack: Some("agentic-code"),
        },
        Signal {
            id: "documentation",
            label: "documentation rules",
            covered: mentions(
                &all_text,
                &["doc", "docs", "documentation", "readme", "changelog"],
            ),
            gap: "No documentation rule.",
            suggested_pack: None,
        },
        Signal {
            id: "path-rules",
            label: "path-scoped rules",
            covered: !path_rules.is_empty(),
            gap: "No path-scoped rules; findings apply repository-wide.",
            suggested_pack: Some("agentic-code"),
        },
        Signal {
            id: "ast-rules",
            label: "AST rule declarations",
            covered: !ast_rules.is_empty(),
            gap: "No AST-oriented rule declarations.",
            suggested_pack: None,
        },
        Signal {
            id: "typecheck-analyzer",
            label: "typecheck analyzer",
            covered: analyzer_matches(&["typecheck", "tsc"]),
            gap: "No typecheck analyzer configured.",
            suggested_pack: Some("typescript-basic"),
        },
        Signal {
            id: "test-analyzer",
            label: "test analyzer",
            covered: analyzer_matches(&["test"]),
            gap: "No test analyzer configured.",
            suggested_pack: Some("typescript-basic"),
        },
        Signal {
            id: "lint-analyzer",
            label: "lint analyzer",
            covered: analyzer_matches(&["lint"]),
            gap: "No lint analyzer configured.",
            suggested_pack: None,
        },
        Signal {
            id: "profiles",
            label: "named profiles",
            covered: profile_count > 0,
            gap: "No named profile; reviews run in the default mode.",
            suggested_pack: None,
        },
        Signal {
            id: "policy-packs",
            label: "policy packs",
            covered: pack_count > 0,
            gap: "No policy pack is configured.",
            suggested_pack: Some("agentic-code"),
        },
        Signal {
            id: "suppressions",
            label: "auditable suppressions",
            covered: !suppressions.is_empty(),
            gap:
                "No suppression entries; scope and expire suppressions instead of disabling rules.",
            suggested_pack: None,
        },
    ];

    let covered = signals
        .iter()
        .filter(|signal| signal.covered)
        .map(|signal| signal.label.to_string())
        .collect::<Vec<_>>();
    let missing = signals
        .iter()
        .filter(|signal| !signal.covered)
        .map(|signal| PolicyDoctorGap {
            category: signal.id.to_string(),
            message: signal.gap.to_string(),
            suggested_pack: signal.suggested_pack.map(str::to_string),
        })
        .collect::<Vec<_>>();
    let covered_count = signals.iter().filter(|signal| signal.covered).count();
    let score = u32::try_from(covered_count * 100 / signals.len().max(1)).unwrap_or(0);
    let coverage = if score >= 75 {
        "high"
    } else if score >= 40 {
        "medium"
    } else if score > 0 {
        "low"
    } else {
        "none"
    };

    let mut suggested_packs: Vec<String> = Vec::new();
    for gap in &missing {
        if let Some(pack) = &gap.suggested_pack {
            if !suggested_packs.contains(pack) {
                suggested_packs.push(pack.clone());
            }
        }
    }

    PolicyDoctorReport {
        schema_version: POLICY_DOCTOR_SCHEMA_VERSION,
        coverage,
        score,
        covered,
        missing,
        suggested_packs,
        rule_count: rules.len(),
        path_rule_count: path_rules.len(),
        ast_rule_count: ast_rules.len(),
        analyzer_count,
        profile_count,
        loaded_policy_packs: result
            .loaded_policy_packs
            .iter()
            .map(|pack| pack.id.clone())
            .collect(),
        warnings: result.warnings.clone(),
        errors: result.errors.clone(),
    }
}
