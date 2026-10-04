//! Deterministic, bounded repository inventory for health audits.
//!
//! The inventory enumerates tracked files at `HEAD` (a stable snapshot), applies
//! structural classification and exclusions, and produces a stable fingerprint.
//! It never reads the working tree as audited content and never mutates the
//! repository. Binary detection is extension-based for this initial inventory;
//! content sniffing can be added later without changing the fingerprint input
//! contract.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::process::Stdio;
use std::thread;
use std::time::{Duration, Instant};

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::local_review::{run_git_bounded_with_control, GitRunControl, LocalReviewCancellation};

pub const REPOSITORY_INVENTORY_SCHEMA_VERSION: &str = "norn.repository-inventory.v1";

const INVENTORY_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_TRACKED_FILES: usize = 20_000;
const MAX_INCLUDED_BYTES: u64 = 64 * 1024 * 1024;
const MAX_FILE_BYTES: u64 = 5 * 1024 * 1024;
const MAX_GIT_OUTPUT_BYTES: usize = 64 * 1024 * 1024;

#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryInventory {
    pub schema_version: &'static str,
    pub snapshot: InventorySnapshot,
    pub fingerprint: String,
    pub totals: InventoryTotals,
    pub languages: Vec<LanguageSummary>,
    pub files: Vec<InventoryFile>,
    pub exclusions: Vec<InventoryExclusion>,
    pub warnings: Vec<String>,
}

#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct InventorySnapshot {
    pub head_sha: String,
    pub source: &'static str,
    pub working_tree_dirty: bool,
}

#[derive(Serialize, Debug, Clone, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct InventoryTotals {
    pub tracked_files: usize,
    pub included_files: usize,
    pub excluded_files: usize,
    pub included_bytes: u64,
    pub source_files: usize,
    pub test_files: usize,
    pub documentation_files: usize,
    pub module_count: usize,
    pub modules_with_tests: usize,
}

#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LanguageSummary {
    pub language: String,
    pub files: usize,
    pub bytes: u64,
}

#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct InventoryFile {
    pub path: String,
    pub object_id: String,
    pub size_bytes: u64,
    pub kind: InventoryFileKind,
    pub language: Option<String>,
    pub module: String,
}

#[derive(Serialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InventoryFileKind {
    Source,
    Test,
    Documentation,
    Manifest,
    Other,
}

#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct InventoryExclusion {
    pub reason: InventoryExclusionReason,
    /// `None` when the path is redacted because it looks sensitive.
    pub path: Option<String>,
}

#[derive(Serialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InventoryExclusionReason {
    Generated,
    Vendored,
    Binary,
    Symlink,
    Submodule,
    Sensitive,
    Oversized,
    Ignored,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InventoryOptions {
    /// Additional path prefixes or substrings to exclude, from repository config.
    pub extra_excludes: Vec<String>,
}

struct TreeEntry {
    mode: String,
    object_type: String,
    object_id: String,
    size: Option<u64>,
    path: String,
}

pub fn build_repository_inventory(
    repo_path: &Path,
    options: &InventoryOptions,
) -> Result<RepositoryInventory, String> {
    if !repo_path.is_dir() {
        return Err(format!(
            "Repository path does not exist or is not a directory: {}",
            repo_path.display()
        ));
    }
    let control = GitRunControl::new(
        INVENTORY_TIMEOUT,
        LocalReviewCancellation::new(),
        "Repository inventory",
    );

    let head_sha = optional_text(
        repo_path,
        &["rev-parse", "--verify", "HEAD"],
        &control,
    )?
    .ok_or_else(|| {
        "Cannot audit a repository without commits; the inventory snapshots tracked files at HEAD."
            .to_string()
    })?;

    let dirty = !git_bytes(
        repo_path,
        &["status", "--porcelain=v1", "-z", "--untracked-files=no"],
        8 * 1024 * 1024,
        "Repository inventory status",
        &control,
    )?
    .is_empty();

    let tree = git_bytes(
        repo_path,
        &["ls-tree", "-r", "-z", "-l", "HEAD"],
        MAX_GIT_OUTPUT_BYTES,
        "Repository inventory tree",
        &control,
    )?;
    let entries = parse_tree(&tree)?;

    let mut files = Vec::new();
    let mut exclusions = Vec::new();
    let mut warnings = Vec::new();
    let mut included_bytes = 0_u64;

    let mut tracked_files = entries.len();
    if tracked_files > MAX_TRACKED_FILES {
        warnings.push(format!(
            "Repository has {tracked_files} tracked files; the inventory was limited to the first {MAX_TRACKED_FILES}."
        ));
        tracked_files = MAX_TRACKED_FILES;
    }

    'entries: for entry in entries.iter().take(MAX_TRACKED_FILES) {
        if let Some(reason) = classify_exclusion(entry, options) {
            exclusions.push(InventoryExclusion {
                reason,
                path: exclusion_path(reason, &entry.path),
            });
            continue;
        }
        if let Some(size) = entry.size {
            if included_bytes.saturating_add(size) > MAX_INCLUDED_BYTES {
                warnings.push(format!(
                    "Repository inventory reached the {} MiB evidence limit; remaining files were excluded.",
                    MAX_INCLUDED_BYTES / (1024 * 1024)
                ));
                break 'entries;
            }
            included_bytes = included_bytes.saturating_add(size);
        }
        let (kind, language) = classify_included(&entry.path);
        files.push(InventoryFile {
            path: entry.path.clone(),
            object_id: entry.object_id.clone(),
            size_bytes: entry.size.unwrap_or(0),
            kind,
            language,
            module: module_of(&entry.path),
        });
    }

    files.sort_by(|left, right| left.path.cmp(&right.path));
    exclusions.sort_by(|left, right| {
        left.reason
            .as_str()
            .cmp(right.reason.as_str())
            .then_with(|| left.path.cmp(&right.path))
    });

    let totals = summarize(&files, exclusions.len(), tracked_files, included_bytes);
    let languages = summarize_languages(&files);
    let fingerprint = fingerprint_of(&files, &exclusions)?;

    Ok(RepositoryInventory {
        schema_version: REPOSITORY_INVENTORY_SCHEMA_VERSION,
        snapshot: InventorySnapshot {
            head_sha,
            source: "head",
            working_tree_dirty: dirty,
        },
        fingerprint,
        totals,
        languages,
        files,
        exclusions,
        warnings,
    })
}

impl InventoryExclusionReason {
    fn as_str(self) -> &'static str {
        match self {
            Self::Generated => "generated",
            Self::Vendored => "vendored",
            Self::Binary => "binary",
            Self::Symlink => "symlink",
            Self::Submodule => "submodule",
            Self::Sensitive => "sensitive",
            Self::Oversized => "oversized",
            Self::Ignored => "ignored",
        }
    }
}

fn exclusion_path(reason: InventoryExclusionReason, path: &str) -> Option<String> {
    // Sensitive paths are never revealed as inventory evidence.
    (reason != InventoryExclusionReason::Sensitive).then(|| path.to_string())
}

fn parse_tree(output: &[u8]) -> Result<Vec<TreeEntry>, String> {
    let mut entries = Vec::new();
    for record in output
        .split(|byte| *byte == 0)
        .filter(|record| !record.is_empty())
    {
        let tab = record
            .iter()
            .position(|byte| *byte == b'\t')
            .ok_or_else(|| "Git returned an invalid inventory tree record.".to_string())?;
        let header = std::str::from_utf8(&record[..tab])
            .map_err(|_| "Git returned a non-UTF-8 inventory header.".to_string())?;
        let path = std::str::from_utf8(&record[tab + 1..])
            .map_err(|_| "Git returned a non-UTF-8 inventory path.".to_string())?;
        let mut fields = header.split_ascii_whitespace();
        let mode = fields
            .next()
            .ok_or_else(|| "Git returned an inventory entry without a mode.".to_string())?;
        let object_type = fields
            .next()
            .ok_or_else(|| "Git returned an inventory entry without a type.".to_string())?;
        let object_id = fields
            .next()
            .ok_or_else(|| "Git returned an inventory entry without an object id.".to_string())?;
        let size = fields.next().and_then(|value| value.parse::<u64>().ok());
        entries.push(TreeEntry {
            mode: mode.to_string(),
            object_type: object_type.to_string(),
            object_id: object_id.to_string(),
            size,
            path: path.to_string(),
        });
    }
    Ok(entries)
}

fn classify_exclusion(
    entry: &TreeEntry,
    options: &InventoryOptions,
) -> Option<InventoryExclusionReason> {
    if entry.mode == "120000" {
        return Some(InventoryExclusionReason::Symlink);
    }
    if entry.object_type == "commit" || entry.mode == "160000" {
        return Some(InventoryExclusionReason::Submodule);
    }
    if is_sensitive_path(&entry.path) {
        return Some(InventoryExclusionReason::Sensitive);
    }
    if options
        .extra_excludes
        .iter()
        .any(|pattern| path_contains(&entry.path, pattern))
    {
        return Some(InventoryExclusionReason::Ignored);
    }
    if is_vendored_path(&entry.path) {
        return Some(InventoryExclusionReason::Vendored);
    }
    if is_generated_path(&entry.path) {
        return Some(InventoryExclusionReason::Generated);
    }
    if is_binary_path(&entry.path) {
        return Some(InventoryExclusionReason::Binary);
    }
    if entry.size.is_some_and(|size| size > MAX_FILE_BYTES) {
        return Some(InventoryExclusionReason::Oversized);
    }
    None
}

fn path_contains(path: &str, pattern: &str) -> bool {
    let pattern = pattern.trim_matches('/');
    if pattern.is_empty() {
        return false;
    }
    path == pattern
        || path.starts_with(&format!("{pattern}/"))
        || path.contains(&format!("/{pattern}/"))
        || path.ends_with(&format!("/{pattern}"))
}

fn is_sensitive_path(path: &str) -> bool {
    let path = path.to_ascii_lowercase();
    let file_name = path.rsplit('/').next().unwrap_or(path.as_str());
    if file_name == ".env" || file_name.starts_with(".env.") {
        return true;
    }
    const SENSITIVE_NAMES: &[&str] = &[
        ".npmrc",
        ".netrc",
        ".git-credentials",
        "id_rsa",
        "id_ed25519",
        "credentials",
        "secrets",
        "secrets.json",
        "service-account.json",
    ];
    if SENSITIVE_NAMES.contains(&file_name) {
        return true;
    }
    if path.starts_with("secrets/") || path.contains("/secrets/") {
        return true;
    }
    const SENSITIVE_EXTENSIONS: &[&str] = &["pem", "key", "p12", "pfx", "keystore", "jks", "asc"];
    file_extension(file_name).is_some_and(|extension| SENSITIVE_EXTENSIONS.contains(&extension))
}

fn is_vendored_path(path: &str) -> bool {
    const VENDOR_SEGMENTS: &[&str] = &[
        "node_modules",
        "vendor",
        "third_party",
        "third-party",
        "external",
        "Pods",
    ];
    path.split('/')
        .any(|segment| VENDOR_SEGMENTS.contains(&segment))
}

fn is_generated_path(path: &str) -> bool {
    const GENERATED_SEGMENTS: &[&str] = &[
        "target",
        "dist",
        "build",
        "out",
        "coverage",
        ".next",
        ".nuxt",
        "generated",
        "__pycache__",
    ];
    if path
        .split('/')
        .any(|segment| GENERATED_SEGMENTS.contains(&segment))
    {
        return true;
    }
    let file_name = path.rsplit('/').next().unwrap_or(path);
    file_name.ends_with(".min.js")
        || file_name.ends_with(".min.css")
        || file_name.ends_with(".pb.go")
        || file_name.ends_with(".designer.cs")
        || file_name.ends_with(".generated.ts")
        || file_name.ends_with(".g.cs")
}

fn is_binary_path(path: &str) -> bool {
    let file_name = path.rsplit('/').next().unwrap_or(path);
    const BINARY_EXTENSIONS: &[&str] = &[
        "png", "jpg", "jpeg", "gif", "webp", "bmp", "ico", "tiff", "avif", "pdf", "zip", "gz",
        "tgz", "bz2", "xz", "7z", "rar", "jar", "war", "class", "pyc", "pyo", "so", "dylib", "dll",
        "exe", "bin", "o", "a", "wasm", "woff", "woff2", "ttf", "otf", "eot", "mp3", "mp4", "mov",
        "avi", "wav", "ogg", "webm", "sqlite", "db", "psd", "sketch",
    ];
    file_extension(file_name).is_some_and(|extension| BINARY_EXTENSIONS.contains(&extension))
}

fn classify_included(path: &str) -> (InventoryFileKind, Option<String>) {
    let file_name = path.rsplit('/').next().unwrap_or(path);
    let lower = file_name.to_ascii_lowercase();
    if is_test_path(path, &lower) {
        return (InventoryFileKind::Test, language_of(&lower));
    }
    if is_manifest_name(&lower) {
        return (InventoryFileKind::Manifest, None);
    }
    if is_documentation_path(path, &lower) {
        return (InventoryFileKind::Documentation, None);
    }
    let language = language_of(&lower);
    let kind = if language.is_some() {
        InventoryFileKind::Source
    } else {
        InventoryFileKind::Other
    };
    (kind, language)
}

fn is_test_path(path: &str, file_name: &str) -> bool {
    path.split('/')
        .any(|segment| matches!(segment, "tests" | "test" | "__tests__" | "spec"))
        || file_name.contains(".test.")
        || file_name.contains(".spec.")
        || file_name.ends_with("_test.go")
        || file_name.ends_with("_test.rs")
}

fn is_manifest_name(file_name: &str) -> bool {
    const MANIFESTS: &[&str] = &[
        "package.json",
        "cargo.toml",
        "cargo.lock",
        "pnpm-lock.yaml",
        "package-lock.json",
        "yarn.lock",
        "go.mod",
        "go.sum",
        "pyproject.toml",
        "requirements.txt",
        "gemfile",
        "pom.xml",
        "build.gradle",
        "composer.json",
        "dockerfile",
        "makefile",
        "justfile",
    ];
    MANIFESTS.contains(&file_name)
}

fn is_documentation_path(path: &str, file_name: &str) -> bool {
    path.starts_with("docs/")
        || file_name.ends_with(".md")
        || file_name.ends_with(".mdx")
        || file_name.ends_with(".rst")
        || matches!(
            file_name,
            "readme" | "readme.md" | "changelog" | "changelog.md"
        )
}

fn language_of(file_name: &str) -> Option<String> {
    let extension = file_extension(file_name)?;
    let language = match extension {
        "ts" => "typescript",
        "tsx" => "typescript",
        "mts" | "cts" => "typescript",
        "js" | "jsx" | "mjs" | "cjs" => "javascript",
        "rs" => "rust",
        "go" => "go",
        "py" => "python",
        "rb" => "ruby",
        "java" => "java",
        "kt" | "kts" => "kotlin",
        "swift" => "swift",
        "c" => "c",
        "h" => "c",
        "cc" | "cpp" | "cxx" | "hpp" => "cpp",
        "cs" => "csharp",
        "php" => "php",
        "scala" => "scala",
        "sh" | "bash" | "zsh" => "shell",
        "sql" => "sql",
        "css" => "css",
        "scss" | "sass" => "scss",
        "html" | "htm" => "html",
        "vue" => "vue",
        "svelte" => "svelte",
        "ex" | "exs" => "elixir",
        "erl" | "hrl" => "erlang",
        "clj" | "cljs" => "clojure",
        "lua" => "lua",
        "dart" => "dart",
        _ => return None,
    };
    Some(language.to_string())
}

fn file_extension(file_name: &str) -> Option<&str> {
    let (_, extension) = file_name.rsplit_once('.')?;
    (!extension.is_empty()).then_some(extension)
}

fn module_of(path: &str) -> String {
    match path.rsplit_once('/') {
        Some((directory, _)) => directory.to_string(),
        None => String::new(),
    }
}

fn summarize(
    files: &[InventoryFile],
    excluded_files: usize,
    tracked_files: usize,
    included_bytes: u64,
) -> InventoryTotals {
    let mut modules = BTreeSet::new();
    let mut modules_with_tests = BTreeSet::new();
    let mut source_files = 0;
    let mut test_files = 0;
    let mut documentation_files = 0;
    for file in files {
        modules.insert(file.module.clone());
        match file.kind {
            InventoryFileKind::Source => source_files += 1,
            InventoryFileKind::Test => {
                test_files += 1;
                modules_with_tests.insert(file.module.clone());
            }
            InventoryFileKind::Documentation => documentation_files += 1,
            InventoryFileKind::Manifest | InventoryFileKind::Other => {}
        }
    }
    InventoryTotals {
        tracked_files,
        included_files: files.len(),
        excluded_files,
        included_bytes,
        source_files,
        test_files,
        documentation_files,
        module_count: modules.len(),
        modules_with_tests: modules_with_tests.len(),
    }
}

fn summarize_languages(files: &[InventoryFile]) -> Vec<LanguageSummary> {
    let mut by_language: BTreeMap<String, (usize, u64)> = BTreeMap::new();
    for file in files {
        if let Some(language) = file.language.as_deref() {
            let entry = by_language.entry(language.to_string()).or_insert((0, 0));
            entry.0 += 1;
            entry.1 = entry.1.saturating_add(file.size_bytes);
        }
    }
    by_language
        .into_iter()
        .map(|(language, (files, bytes))| LanguageSummary {
            language,
            files,
            bytes,
        })
        .collect()
}

fn fingerprint_of(
    files: &[InventoryFile],
    exclusions: &[InventoryExclusion],
) -> Result<String, String> {
    let mut hasher = Sha256::new();
    hasher.update(REPOSITORY_INVENTORY_SCHEMA_VERSION.as_bytes());
    for file in files {
        hasher.update([0u8]);
        hasher.update(file.path.as_bytes());
        hasher.update([0u8]);
        hasher.update(file.object_id.as_bytes());
        hasher.update(file.size_bytes.to_be_bytes());
        hasher.update(file.kind.as_str().as_bytes());
        if let Some(language) = file.language.as_deref() {
            hasher.update(language.as_bytes());
        }
    }
    for exclusion in exclusions {
        hasher.update([1u8]);
        hasher.update(exclusion.reason.as_str().as_bytes());
        if let Some(path) = exclusion.path.as_deref() {
            hasher.update(path.as_bytes());
        }
    }
    Ok(hex::encode(hasher.finalize()))
}

impl InventoryFileKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Source => "source",
            Self::Test => "test",
            Self::Documentation => "documentation",
            Self::Manifest => "manifest",
            Self::Other => "other",
        }
    }
}

fn git_bytes(
    repo_path: &Path,
    args: &[&str],
    limit: usize,
    description: &str,
    control: &GitRunControl,
) -> Result<Vec<u8>, String> {
    let output = run_git_bounded_with_control(repo_path, args, None, limit, description, control)?;
    if !output.status.success() {
        return Err(format!(
            "{description} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(output.stdout)
}

fn optional_text(
    repo_path: &Path,
    args: &[&str],
    control: &GitRunControl,
) -> Result<Option<String>, String> {
    let output = run_git_bounded_with_control(
        repo_path,
        args,
        None,
        4 * 1024,
        "Repository inventory metadata",
        control,
    )?;
    if !output.status.success() {
        return Ok(None);
    }
    Ok(Some(
        String::from_utf8_lossy(&output.stdout).trim().to_string(),
    ))
}

pub const REPOSITORY_AUDIT_EVIDENCE_SCHEMA_VERSION: &str = "norn.repository-audit-evidence.v1";
const DEFAULT_ANALYZER_TIMEOUT_SECONDS: u64 = 120;
const MAX_ANALYZER_TIMEOUT_SECONDS: u64 = 900;
const MAX_ANALYZER_OUTPUT_BYTES: usize = 256 * 1024;
const LARGE_MODULE_SOURCE_FILES: usize = 20;

/// Deterministic evidence for a repository health audit: the inventory
/// fingerprint, analyzer provenance, and inventory-derived health signals.
#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryAuditEvidence {
    pub schema_version: &'static str,
    pub inventory_fingerprint: String,
    pub analyzers: Vec<AnalyzerEvidence>,
    pub signals: Vec<HealthSignal>,
    pub warnings: Vec<String>,
}

#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AnalyzerEvidence {
    pub id: String,
    pub command: Option<String>,
    pub applicability: AnalyzerApplicability,
    pub status: AnalyzerOutcome,
    pub exit_status: Option<i32>,
    pub duration_ms: u64,
    pub truncated: bool,
    pub evidence_id: String,
    pub message: String,
}

#[derive(Serialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AnalyzerApplicability {
    Applicable,
    Disabled,
    Unavailable,
}

#[derive(Serialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AnalyzerOutcome {
    Ran,
    Failed,
    TimedOut,
    Skipped,
}

#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HealthSignal {
    pub id: String,
    pub kind: HealthSignalKind,
    pub value: u64,
    pub summary: String,
    pub evidence_id: String,
}

#[derive(Serialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HealthSignalKind {
    TestGaps,
    LargeModules,
    LegacyMarkers,
    DependencyManifests,
}

/// Collects deterministic audit evidence for a repository snapshot: the
/// inventory, configured analyzer evidence, and inventory-derived signals.
/// Analyzers only run when explicitly enabled; they are bounded by timeout and
/// output limits, and a failure never invalidates unrelated evidence.
pub fn collect_repository_audit_evidence(
    repo_path: &Path,
    analyzers: &BTreeMap<String, crate::repo_config::AnalyzerConfig>,
    options: &InventoryOptions,
) -> Result<RepositoryAuditEvidence, String> {
    let inventory = build_repository_inventory(repo_path, options)?;
    let mut warnings = inventory.warnings.clone();

    let mut analyzer_evidence = Vec::new();
    for (id, analyzer) in analyzers {
        analyzer_evidence.push(run_analyzer_evidence(repo_path, id, analyzer));
    }

    let mut signals = derive_health_signals(&inventory);
    match legacy_marker_count(repo_path) {
        Ok(count) if count > 0 => signals.push(HealthSignal {
            id: "audit.legacy-markers".to_string(),
            kind: HealthSignalKind::LegacyMarkers,
            value: count,
            summary: format!("{count} legacy marker comment(s) (TODO/FIXME/HACK/XXX)"),
            evidence_id: "signal:audit.legacy-markers".to_string(),
        }),
        Ok(_) => {}
        Err(error) => warnings.push(format!("Legacy marker scan unavailable: {error}")),
    }

    signals.sort_by(|left, right| left.id.cmp(&right.id));

    Ok(RepositoryAuditEvidence {
        schema_version: REPOSITORY_AUDIT_EVIDENCE_SCHEMA_VERSION,
        inventory_fingerprint: inventory.fingerprint,
        analyzers: analyzer_evidence,
        signals,
        warnings,
    })
}

fn run_analyzer_evidence(
    repo_path: &Path,
    id: &str,
    analyzer: &crate::repo_config::AnalyzerConfig,
) -> AnalyzerEvidence {
    let evidence_id = format!("analyzer:{id}");
    if !analyzer.enabled {
        return AnalyzerEvidence {
            id: id.to_string(),
            command: analyzer.command.clone(),
            applicability: AnalyzerApplicability::Disabled,
            status: AnalyzerOutcome::Skipped,
            exit_status: None,
            duration_ms: 0,
            truncated: false,
            evidence_id,
            message: "Analyzer is disabled in repository configuration.".to_string(),
        };
    }
    let Some(command) = analyzer
        .command
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return AnalyzerEvidence {
            id: id.to_string(),
            command: None,
            applicability: AnalyzerApplicability::Unavailable,
            status: AnalyzerOutcome::Skipped,
            exit_status: None,
            duration_ms: 0,
            truncated: false,
            evidence_id,
            message: "Analyzer has no command configured.".to_string(),
        };
    };

    let timeout_seconds = analyzer
        .timeout_seconds
        .unwrap_or(DEFAULT_ANALYZER_TIMEOUT_SECONDS)
        .clamp(1, MAX_ANALYZER_TIMEOUT_SECONDS);
    let result = run_bounded_command(repo_path, command, timeout_seconds);
    let (status, message) = match result.status {
        CommandStatus::Ran(Some(0)) => (
            AnalyzerOutcome::Ran,
            "Analyzer completed successfully.".to_string(),
        ),
        CommandStatus::Ran(code) => (
            AnalyzerOutcome::Failed,
            format!(
                "Analyzer exited with status {}.",
                code.map(|code| code.to_string())
                    .unwrap_or_else(|| "unknown".to_string())
            ),
        ),
        CommandStatus::TimedOut => (
            AnalyzerOutcome::TimedOut,
            format!("Analyzer timed out after {timeout_seconds}s."),
        ),
        CommandStatus::SpawnFailed(error) => (
            AnalyzerOutcome::Failed,
            format!("Analyzer could not start: {error}"),
        ),
    };

    AnalyzerEvidence {
        id: id.to_string(),
        command: Some(command.to_string()),
        applicability: AnalyzerApplicability::Applicable,
        status,
        exit_status: result.exit_status,
        duration_ms: result.duration_ms,
        truncated: result.truncated,
        evidence_id,
        message,
    }
}

struct BoundedCommandResult {
    status: CommandStatus,
    exit_status: Option<i32>,
    duration_ms: u64,
    truncated: bool,
}

enum CommandStatus {
    Ran(Option<i32>),
    TimedOut,
    SpawnFailed(String),
}

fn run_bounded_command(
    repo_path: &Path,
    command: &str,
    timeout_seconds: u64,
) -> BoundedCommandResult {
    let started = Instant::now();
    let mut child = match std::process::Command::new("/bin/sh")
        .arg("-lc")
        .arg(command)
        .current_dir(repo_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(error) => {
            return BoundedCommandResult {
                status: CommandStatus::SpawnFailed(error.to_string()),
                exit_status: None,
                duration_ms: started.elapsed().as_millis() as u64,
                truncated: false,
            }
        }
    };

    let stdout_reader = child
        .stdout
        .take()
        .map(|reader| thread::spawn(move || read_capped(reader, MAX_ANALYZER_OUTPUT_BYTES)));
    let stderr_reader = child
        .stderr
        .take()
        .map(|reader| thread::spawn(move || read_capped(reader, MAX_ANALYZER_OUTPUT_BYTES)));

    let timeout = Duration::from_secs(timeout_seconds);
    let (status, exit_status) = loop {
        match child.try_wait() {
            Ok(Some(status)) => break (CommandStatus::Ran(status.code()), status.code()),
            Ok(None) if started.elapsed() >= timeout => {
                let _ = child.kill();
                let _ = child.wait();
                break (CommandStatus::TimedOut, None);
            }
            Ok(None) => thread::sleep(Duration::from_millis(50)),
            Err(error) => {
                let _ = child.kill();
                break (CommandStatus::SpawnFailed(error.to_string()), None);
            }
        }
    };

    let mut truncated = false;
    if let Some(reader) = stdout_reader {
        truncated |= reader.join().unwrap_or(false);
    }
    if let Some(reader) = stderr_reader {
        truncated |= reader.join().unwrap_or(false);
    }

    BoundedCommandResult {
        status,
        exit_status,
        duration_ms: started.elapsed().as_millis() as u64,
        truncated,
    }
}

fn read_capped<R: std::io::Read>(mut reader: R, limit: usize) -> bool {
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 8192];
    let mut truncated = false;
    loop {
        match reader.read(&mut chunk) {
            Ok(0) => break,
            Ok(read) => {
                if buffer.len() < limit {
                    let remaining = limit - buffer.len();
                    let take = read.min(remaining);
                    buffer.extend_from_slice(&chunk[..take]);
                    truncated |= take < read;
                } else {
                    truncated = true;
                }
            }
            Err(_) => break,
        }
    }
    truncated
}

fn derive_health_signals(inventory: &RepositoryInventory) -> Vec<HealthSignal> {
    let mut signals = Vec::new();

    let mut source_modules = BTreeSet::new();
    let mut test_modules = BTreeSet::new();
    let mut source_files_by_module: BTreeMap<&str, u64> = BTreeMap::new();
    for file in &inventory.files {
        match file.kind {
            InventoryFileKind::Source => {
                source_modules.insert(file.module.as_str());
                *source_files_by_module
                    .entry(file.module.as_str())
                    .or_insert(0) += 1;
            }
            InventoryFileKind::Test => {
                test_modules.insert(file.module.as_str());
            }
            _ => {}
        }
    }

    let test_gaps = source_modules
        .iter()
        .filter(|module| !test_modules.contains(*module))
        .count();
    if test_gaps > 0 {
        signals.push(HealthSignal {
            id: "audit.test-gaps".to_string(),
            kind: HealthSignalKind::TestGaps,
            value: test_gaps as u64,
            summary: format!("{test_gaps} module(s) contain source files without a test file"),
            evidence_id: "signal:audit.test-gaps".to_string(),
        });
    }

    let large_modules = source_files_by_module
        .values()
        .filter(|count| **count as usize >= LARGE_MODULE_SOURCE_FILES)
        .count();
    if large_modules > 0 {
        signals.push(HealthSignal {
            id: "audit.large-modules".to_string(),
            kind: HealthSignalKind::LargeModules,
            value: large_modules as u64,
            summary: format!(
                "{large_modules} module(s) have at least {LARGE_MODULE_SOURCE_FILES} source files"
            ),
            evidence_id: "signal:audit.large-modules".to_string(),
        });
    }

    let manifests = inventory
        .files
        .iter()
        .filter(|file| file.kind == InventoryFileKind::Manifest)
        .count();
    if manifests > 0 {
        signals.push(HealthSignal {
            id: "audit.dependency-manifests".to_string(),
            kind: HealthSignalKind::DependencyManifests,
            value: manifests as u64,
            summary: format!("{manifests} dependency manifest(s) detected"),
            evidence_id: "signal:audit.dependency-manifests".to_string(),
        });
    }

    signals
}

fn legacy_marker_count(repo_path: &Path) -> Result<u64, String> {
    let control = GitRunControl::new(
        Duration::from_secs(30),
        LocalReviewCancellation::new(),
        "Repository audit legacy markers",
    );
    let output = run_git_bounded_with_control(
        repo_path,
        &[
            "grep", "-I", "--count", "-e", "TODO", "-e", "FIXME", "-e", "HACK", "-e", "XXX", "HEAD",
        ],
        None,
        8 * 1024 * 1024,
        "Repository audit legacy markers",
        &control,
    )?;
    // `git grep` exits 1 when there are no matches; that is not an error.
    if !output.status.success() && output.status.code() != Some(1) {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    let mut total = 0_u64;
    for line in output.stdout.split(|byte| *byte == b'\n') {
        if line.is_empty() {
            continue;
        }
        if let Some(count) = line.rsplit(|byte| *byte == b':').next() {
            if let Ok(count) = std::str::from_utf8(count)
                .unwrap_or_default()
                .trim()
                .parse::<u64>()
            {
                total = total.saturating_add(count);
            }
        }
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repo_config;
    use std::fs;
    use std::process::Command;

    struct Fixture {
        path: std::path::PathBuf,
    }

    impl Fixture {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "norn-repository-audit-{name}-{}",
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).expect("create fixture");
            run(&path, &["init", "-q"]);
            run(&path, &["config", "user.email", "norn@example.com"]);
            run(&path, &["config", "user.name", "Norn"]);
            Self { path }
        }

        fn write(&self, path: &str, value: &str) {
            self.write_bytes(path, value.as_bytes());
        }

        fn write_bytes(&self, path: &str, value: &[u8]) {
            let target = self.path.join(path);
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).expect("create parent");
            }
            fs::write(target, value).expect("write fixture");
        }

        fn commit_all(&self, message: &str) {
            run(&self.path, &["add", "."]);
            run(&self.path, &["commit", "-qm", message]);
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn run(path: &Path, args: &[&str]) {
        let output = Command::new("git")
            .arg("-C")
            .arg(path)
            .args(args)
            .output()
            .expect("run git");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn build(fixture: &Fixture) -> RepositoryInventory {
        build_repository_inventory(&fixture.path, &InventoryOptions::default()).expect("inventory")
    }

    #[test]
    fn inventory_is_deterministic_for_the_same_snapshot() {
        let fixture = Fixture::new("deterministic");
        fixture.write("src/lib.rs", "pub fn one() {}\n");
        fixture.write("src/lib_test.rs", "#[test] fn t() {}\n");
        fixture.write("README.md", "# readme\n");
        fixture.write("Cargo.toml", "[package]\n");
        fixture.commit_all("seed");

        let first = build(&fixture);
        let second = build(&fixture);

        assert_eq!(first.fingerprint, second.fingerprint);
        assert_eq!(
            serde_json::to_string(&first).expect("json"),
            serde_json::to_string(&second).expect("json")
        );
        assert_eq!(first.totals.included_files, 4);
        assert_eq!(first.snapshot.source, "head");
        assert!(!first.snapshot.working_tree_dirty);
    }

    #[test]
    fn inventory_classifies_files_and_redacts_sensitive_paths() {
        let fixture = Fixture::new("classification");
        fixture.write("src/app.ts", "export const app = 1;\n");
        fixture.write("src/app.test.ts", "test('app', () => {});\n");
        fixture.write("docs/guide.md", "# guide\n");
        fixture.write("package.json", "{}\n");
        fixture.write("assets/logo.png", "not-really-an-image\n");
        fixture.write("target/debug/artifact.bin", "artifact\n");
        fixture.write("vendor/library.go", "package library\n");
        fixture.write(".env", "SECRET=super-secret\n");
        fixture.write("certs/server.pem", "-----BEGIN-----\n");
        fixture.commit_all("seed");

        let inventory = build(&fixture);

        let kinds: BTreeMap<&str, InventoryFileKind> = inventory
            .files
            .iter()
            .map(|file| (file.path.as_str(), file.kind))
            .collect();
        assert_eq!(kinds.get("src/app.ts"), Some(&InventoryFileKind::Source));
        assert_eq!(kinds.get("src/app.test.ts"), Some(&InventoryFileKind::Test));
        assert_eq!(
            kinds.get("docs/guide.md"),
            Some(&InventoryFileKind::Documentation)
        );
        assert_eq!(
            kinds.get("package.json"),
            Some(&InventoryFileKind::Manifest)
        );
        assert_eq!(
            inventory
                .files
                .iter()
                .find(|file| file.path == "src/app.ts")
                .and_then(|file| file.language.as_deref()),
            Some("typescript")
        );
        assert_eq!(inventory.totals.source_files, 1);
        assert_eq!(inventory.totals.test_files, 1);
        assert_eq!(inventory.totals.modules_with_tests, 1);

        let reasons: BTreeMap<&str, InventoryExclusionReason> = inventory
            .exclusions
            .iter()
            .filter_map(|exclusion| {
                exclusion
                    .path
                    .as_deref()
                    .map(|path| (path, exclusion.reason))
            })
            .collect();
        assert_eq!(
            reasons.get("assets/logo.png"),
            Some(&InventoryExclusionReason::Binary)
        );
        assert_eq!(
            reasons.get("target/debug/artifact.bin"),
            Some(&InventoryExclusionReason::Generated)
        );
        assert_eq!(
            reasons.get("vendor/library.go"),
            Some(&InventoryExclusionReason::Vendored)
        );

        // Sensitive paths must never appear in evidence.
        assert!(inventory.exclusions.iter().any(|exclusion| exclusion.reason
            == InventoryExclusionReason::Sensitive
            && exclusion.path.is_none()));
        assert!(!inventory.files.iter().any(|file| file.path == ".env"));
        assert!(!inventory
            .files
            .iter()
            .any(|file| file.path == "certs/server.pem"));
        let rendered = serde_json::to_string(&inventory).expect("json");
        assert!(!rendered.contains(".env"));
        assert!(!rendered.contains("server.pem"));
        assert!(!rendered.contains("super-secret"));
    }

    fn git_text(path: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .arg("-C")
            .arg(path)
            .args(args)
            .output()
            .expect("run git");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    }

    #[test]
    fn inventory_reports_a_dirty_working_tree_without_including_it() {
        let fixture = Fixture::new("dirty");
        fixture.write("src/lib.rs", "pub fn one() {}\n");
        fixture.commit_all("seed");
        let head_blob = git_text(&fixture.path, &["rev-parse", "HEAD:src/lib.rs"]);
        fixture.write("src/lib.rs", "pub fn changed() {}\n");

        let inventory = build(&fixture);
        assert!(inventory.snapshot.working_tree_dirty);
        let file = inventory
            .files
            .iter()
            .find(|file| file.path == "src/lib.rs")
            .expect("file");
        assert_eq!(file.object_id, head_blob);
    }

    #[test]
    fn inventory_honors_extra_excludes() {
        let fixture = Fixture::new("extra-excludes");
        fixture.write("src/keep.ts", "export const keep = 1;\n");
        fixture.write("src/skip-me.ts", "export const skip = 1;\n");
        fixture.commit_all("seed");

        let inventory = build_repository_inventory(
            &fixture.path,
            &InventoryOptions {
                extra_excludes: vec!["skip-me.ts".to_string()],
            },
        )
        .expect("inventory");

        assert!(inventory
            .files
            .iter()
            .any(|file| file.path == "src/keep.ts"));
        assert!(!inventory
            .files
            .iter()
            .any(|file| file.path == "src/skip-me.ts"));
        assert!(inventory.exclusions.iter().any(|exclusion| exclusion.reason
            == InventoryExclusionReason::Ignored
            && exclusion.path.as_deref() == Some("src/skip-me.ts")));
    }

    #[test]
    fn inventory_excludes_oversized_files() {
        let fixture = Fixture::new("oversized");
        fixture.write_bytes("big.txt", &vec![b'a'; (MAX_FILE_BYTES as usize) + 1024]);
        fixture.write("src/lib.rs", "pub fn one() {}\n");
        fixture.commit_all("seed");

        let inventory = build(&fixture);
        assert!(inventory.exclusions.iter().any(|exclusion| {
            exclusion.reason == InventoryExclusionReason::Oversized
                && exclusion.path.as_deref() == Some("big.txt")
        }));
        assert!(!inventory.files.iter().any(|file| file.path == "big.txt"));
    }

    #[test]
    fn inventory_rejects_a_repository_without_commits() {
        let fixture = Fixture::new("no-commits");
        let error = build_repository_inventory(&fixture.path, &InventoryOptions::default())
            .expect_err("empty repository should fail");
        assert!(error.contains("without commits"), "{error}");
    }

    #[cfg(unix)]
    #[test]
    fn inventory_excludes_symlinks() {
        let fixture = Fixture::new("symlink");
        fixture.write("src/lib.rs", "pub fn one() {}\n");
        fixture.commit_all("seed");
        std::os::unix::fs::symlink("src/lib.rs", fixture.path.join("link.rs")).expect("symlink");
        run(&fixture.path, &["add", "link.rs"]);
        run(&fixture.path, &["commit", "-qm", "symlink"]);

        let inventory = build(&fixture);
        assert!(inventory
            .exclusions
            .iter()
            .any(|exclusion| exclusion.reason == InventoryExclusionReason::Symlink));
        assert!(!inventory.files.iter().any(|file| file.path == "link.rs"));
    }

    fn analyzer(
        enabled: bool,
        command: Option<&str>,
        timeout_seconds: u64,
    ) -> repo_config::AnalyzerConfig {
        repo_config::AnalyzerConfig {
            enabled,
            command: command.map(str::to_string),
            timeout_seconds: Some(timeout_seconds),
            required: false,
            config: None,
        }
    }

    #[test]
    fn analyzer_evidence_records_provenance_and_outcomes() {
        let fixture = Fixture::new("analyzer-outcomes");
        fixture.write("src/lib.rs", "pub fn one() {}\n");
        fixture.commit_all("seed");

        let mut analyzers = BTreeMap::new();
        analyzers.insert("clean".to_string(), analyzer(true, Some("echo ok"), 30));
        analyzers.insert("failing".to_string(), analyzer(true, Some("exit 3"), 30));
        analyzers.insert("timeout".to_string(), analyzer(true, Some("sleep 5"), 1));
        analyzers.insert(
            "truncated".to_string(),
            analyzer(true, Some("yes a | head -c 400000"), 30),
        );
        analyzers.insert(
            "disabled".to_string(),
            analyzer(false, Some("echo nope"), 30),
        );
        analyzers.insert("missing".to_string(), analyzer(true, None, 30));

        let evidence = collect_repository_audit_evidence(
            &fixture.path,
            &analyzers,
            &InventoryOptions::default(),
        )
        .expect("evidence");
        let by_id: BTreeMap<&str, &AnalyzerEvidence> = evidence
            .analyzers
            .iter()
            .map(|entry| (entry.id.as_str(), entry))
            .collect();

        assert_eq!(by_id["clean"].status, AnalyzerOutcome::Ran);
        assert_eq!(by_id["clean"].exit_status, Some(0));
        assert_eq!(
            by_id["clean"].applicability,
            AnalyzerApplicability::Applicable
        );
        assert_eq!(by_id["failing"].status, AnalyzerOutcome::Failed);
        assert_eq!(by_id["failing"].exit_status, Some(3));
        assert_eq!(by_id["timeout"].status, AnalyzerOutcome::TimedOut);
        assert!(by_id["truncated"].truncated);
        assert_eq!(
            by_id["disabled"].applicability,
            AnalyzerApplicability::Disabled
        );
        assert_eq!(by_id["disabled"].status, AnalyzerOutcome::Skipped);
        assert_eq!(
            by_id["missing"].applicability,
            AnalyzerApplicability::Unavailable
        );
        assert!(evidence
            .analyzers
            .iter()
            .all(|entry| entry.evidence_id == format!("analyzer:{}", entry.id)));
    }

    #[test]
    fn analyzer_failure_does_not_invalidate_other_evidence() {
        let fixture = Fixture::new("analyzer-isolation");
        fixture.write("src/app.ts", "export const a = 1;\n");
        fixture.commit_all("seed");

        let mut analyzers = BTreeMap::new();
        analyzers.insert("bad".to_string(), analyzer(true, Some("exit 1"), 30));
        analyzers.insert("good".to_string(), analyzer(true, Some("echo ok"), 30));

        let evidence = collect_repository_audit_evidence(
            &fixture.path,
            &analyzers,
            &InventoryOptions::default(),
        )
        .expect("evidence");

        let by_id: BTreeMap<&str, &AnalyzerEvidence> = evidence
            .analyzers
            .iter()
            .map(|entry| (entry.id.as_str(), entry))
            .collect();
        assert_eq!(by_id["good"].status, AnalyzerOutcome::Ran);
        assert_eq!(by_id["bad"].status, AnalyzerOutcome::Failed);
        assert!(evidence
            .signals
            .iter()
            .any(|signal| signal.kind == HealthSignalKind::TestGaps));
        assert!(!evidence.inventory_fingerprint.is_empty());
    }

    #[test]
    fn evidence_derives_inventory_signals() {
        let fixture = Fixture::new("evidence-signals");
        fixture.write("src/app.ts", "export const a = 1; // TODO: split\n");
        fixture.write("lib/mod.ts", "export const b = 1;\n");
        fixture.write("package.json", "{}\n");
        fixture.commit_all("seed");

        let evidence = collect_repository_audit_evidence(
            &fixture.path,
            &BTreeMap::new(),
            &InventoryOptions::default(),
        )
        .expect("evidence");

        assert_eq!(
            evidence.schema_version,
            REPOSITORY_AUDIT_EVIDENCE_SCHEMA_VERSION
        );
        assert_eq!(evidence.inventory_fingerprint.len(), 64);
        let signal = |kind: HealthSignalKind| {
            evidence
                .signals
                .iter()
                .find(|signal| signal.kind == kind)
                .unwrap_or_else(|| panic!("missing signal {kind:?}"))
        };
        // `src` and `lib` contain source but no tests.
        assert_eq!(signal(HealthSignalKind::TestGaps).value, 2);
        assert_eq!(signal(HealthSignalKind::DependencyManifests).value, 1);
        assert_eq!(signal(HealthSignalKind::LegacyMarkers).value, 1);
        assert!(evidence
            .signals
            .iter()
            .all(|signal| signal.evidence_id.starts_with("signal:")));
    }
}
