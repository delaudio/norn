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
use std::time::Duration;

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

#[cfg(test)]
mod tests {
    use super::*;
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
}
