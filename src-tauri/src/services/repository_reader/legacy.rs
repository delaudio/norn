//! Shared shipped readonly DTOs and parsers; no Tauri dependency.
use serde::{Deserialize, Serialize};
#[cfg(test)]
use std::collections::BTreeMap;
#[cfg(test)]
pub(crate) const GIT_SHOW_MESSAGE_MARKER: &str = "\u{1f}NORN_COMMIT_MESSAGE_END\u{1f}";
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RepositoryFileEntry {
    pub path: String,
    pub status: RepositoryFileStatus,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub enum RepositoryFileStatus {
    Unchanged,
    Modified,
    Added,
    Deleted,
    Renamed,
    Untracked,
    Conflicted,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RepositoryFileContent {
    pub path: String,
    pub content: String,
    pub size: u64,
    pub truncated: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RepositoryFileDiff {
    pub path: String,
    pub raw_diff: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RepositoryBlameLine {
    pub line: u32,
    pub sha: String,
    pub short_sha: String,
    pub author: Option<String>,
    pub author_email: Option<String>,
    pub author_time: Option<i64>,
    pub summary: Option<String>,
    pub message: Option<String>,
}

pub(crate) fn file_status_from_porcelain(
    index_status: char,
    worktree_status: char,
) -> RepositoryFileStatus {
    if index_status == '?' && worktree_status == '?' {
        return RepositoryFileStatus::Untracked;
    }
    if index_status == 'U'
        || worktree_status == 'U'
        || matches!(
            (index_status, worktree_status),
            ('A', 'A') | ('D', 'D') | ('A', 'U') | ('D', 'U') | ('U', 'A') | ('U', 'D')
        )
    {
        return RepositoryFileStatus::Conflicted;
    }
    if index_status == 'D' || worktree_status == 'D' {
        return RepositoryFileStatus::Deleted;
    }
    if index_status == 'R' || worktree_status == 'R' {
        return RepositoryFileStatus::Renamed;
    }
    if index_status == 'A' || worktree_status == 'A' {
        return RepositoryFileStatus::Added;
    }
    if matches!(index_status, 'M' | 'T') || matches!(worktree_status, 'M' | 'T') {
        return RepositoryFileStatus::Modified;
    }
    RepositoryFileStatus::Unchanged
}

#[cfg(test)]
pub(crate) fn parse_git_status_porcelain_z(output: &str) -> BTreeMap<String, RepositoryFileStatus> {
    let mut statuses = BTreeMap::new();
    let mut parts = output.split_terminator('\0');

    while let Some(entry) = parts.next() {
        if entry.len() < 4 {
            continue;
        }
        let mut chars = entry.chars();
        let index_status = chars.next().unwrap_or(' ');
        let worktree_status = chars.next().unwrap_or(' ');
        let _space = chars.next();
        let path = chars.as_str();
        if path.is_empty() {
            continue;
        }

        let status = file_status_from_porcelain(index_status, worktree_status);
        if matches!(index_status, 'R' | 'C') || matches!(worktree_status, 'R' | 'C') {
            let _old_path = parts.next();
        }
        statuses.insert(path.to_string(), status);
    }

    statuses
}

fn short_sha(sha: &str) -> String {
    sha.chars().take(8).collect()
}

fn strip_mail_brackets(value: &str) -> String {
    value
        .trim()
        .trim_start_matches('<')
        .trim_end_matches('>')
        .to_string()
}

fn parse_blame_header(line: &str) -> Option<(String, u32)> {
    let mut parts = line.split_whitespace();
    let sha = parts.next()?;
    if sha.len() < 7 || !sha.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return None;
    }
    let _original_line = parts.next()?.parse::<u32>().ok()?;
    let final_line = parts.next()?.parse::<u32>().ok()?;
    Some((sha.to_string(), final_line))
}

pub(crate) fn parse_git_blame_line_porcelain(output: &str) -> Vec<RepositoryBlameLine> {
    let mut entries = Vec::new();
    let mut current: Option<RepositoryBlameLine> = None;

    for line in output.lines() {
        if let Some((sha, final_line)) = parse_blame_header(line) {
            if let Some(entry) = current.take() {
                entries.push(entry);
            }
            current = Some(RepositoryBlameLine {
                line: final_line,
                short_sha: short_sha(&sha),
                sha,
                author: None,
                author_email: None,
                author_time: None,
                summary: None,
                message: None,
            });
            continue;
        }

        let Some(entry) = current.as_mut() else {
            continue;
        };

        if line.starts_with('\t') {
            if let Some(entry) = current.take() {
                entries.push(entry);
            }
        } else if let Some(author) = line.strip_prefix("author ") {
            entry.author = Some(author.to_string());
        } else if let Some(author_email) = line.strip_prefix("author-mail ") {
            entry.author_email = Some(strip_mail_brackets(author_email));
        } else if let Some(author_time) = line.strip_prefix("author-time ") {
            entry.author_time = author_time.parse::<i64>().ok();
        } else if let Some(summary) = line.strip_prefix("summary ") {
            entry.summary = Some(summary.to_string());
        }
    }

    if let Some(entry) = current {
        entries.push(entry);
    }

    entries.sort_by_key(|entry| entry.line);
    entries
}

#[cfg(test)]
pub(crate) fn parse_commit_messages(output: &str, shas: &[String]) -> Vec<String> {
    let mut messages = output
        .trim_end()
        .split_terminator(GIT_SHOW_MESSAGE_MARKER)
        .map(|message| message.trim().to_string())
        .collect::<Vec<_>>();
    messages.truncate(shas.len());
    messages
}
