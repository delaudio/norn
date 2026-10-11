//! Fixed-revision provider comparisons and complete, bounded patch inventory.
use super::super::approved_positions::{patch_path, range};
use super::*;
use crate::services::provider_snapshot::{self as snapshot, Capture, Section, SnapshotFile};

impl Api {
    pub(crate) fn capture(&self, params: DetailParams) -> Result<Capture, String> {
        params.validate()?;
        if params.repository != self.repository {
            return Err(error());
        }
        let started = Instant::now();
        let pr_url = format!("{}/{}", self.collection(), params.pr_id);
        let (before, _) = self.get(&pr_url, started)?;
        let (detail, _) = parse_metadata(self.repository.provider, &before, params.pr_id)?;
        let head = snapshot::full_sha(detail.source_commit_hash.as_deref().ok_or_else(error)?)?;
        let destination = snapshot::full_sha(
            detail
                .destination_commit_hash
                .as_deref()
                .ok_or_else(error)?,
        )?;
        if head.len() != destination.len() {
            return Err(error());
        }
        let (base, expected_count, expected_files, diff_url) = match self.repository.provider {
            ReviewProvider::Github => {
                let count = before
                    .get("changed_files")
                    .and_then(Value::as_u64)
                    .filter(|n| *n <= snapshot::FILE_LIMIT as u64)
                    .ok_or_else(error)? as usize;
                let (comparison, _) = self.get(
                    &format!(
                        "{}/compare/{destination}...{head}?per_page=1&page=1",
                        self.base()
                    ),
                    started,
                )?;
                let returned_base = snapshot::full_sha(
                    comparison
                        .pointer("/base_commit/sha")
                        .and_then(Value::as_str)
                        .ok_or_else(error)?,
                )?;
                if returned_base != destination {
                    return Err(error());
                }
                let base = snapshot::full_sha(
                    comparison
                        .pointer("/merge_base_commit/sha")
                        .and_then(Value::as_str)
                        .ok_or_else(error)?,
                )?;
                // Compare JSON's file array is capped at 300; actual complete
                // patch sections must match the PR's independent changed_files.
                let url = format!("{}/compare/{base}...{head}", self.base());
                (base, count, None, url)
            }
            ReviewProvider::Bitbucket => {
                // Source repository is native authenticated PR metadata, not a
                // client URL. A fork comparison remains on the fixed API origin.
                let source = before
                    .pointer("/source/repository/full_name")
                    .and_then(Value::as_str)
                    .ok_or_else(error)?;
                let (workspace, repo) = source.split_once('/').ok_or_else(error)?;
                let source = RepositoryIdentity {
                    provider: ReviewProvider::Bitbucket,
                    workspace: workspace.into(),
                    repo: repo.into(),
                };
                source.validate()?;
                let origin = format!(
                    "{}/repositories/{}/{}",
                    self.root,
                    encode_path_segment(&source.workspace),
                    encode_path_segment(&source.repo)
                );
                let (merge, _) = self.get(
                    &format!("{origin}/merge-base/{head}..{destination}"),
                    started,
                )?;
                let base = snapshot::full_sha(
                    merge
                        .get("hash")
                        .and_then(Value::as_str)
                        .ok_or_else(error)?,
                )?;
                // Bitbucket's source-first order is opposite Git diff. Using
                // the captured merge base removes mutable branch comparisons.
                let collection = format!("{origin}/diffstat/{head}..{base}");
                let mut page = 1;
                let mut files = Vec::new();
                let mut bytes = 0usize;
                let mut paths = HashSet::new();
                loop {
                    let (json, _) = self.get(
                        &format!("{collection}?topic=false&pagelen=100&page={page}"),
                        started,
                    )?;
                    bytes += serde_json::to_vec(&json).map_err(|_| error())?.len();
                    if bytes > snapshot::PATCH_BYTES
                        || json.get("page").and_then(Value::as_u64) != Some(u64::from(page))
                    {
                        return Err(error());
                    }
                    let raw = json
                        .get("values")
                        .and_then(Value::as_array)
                        .ok_or_else(error)?;
                    if raw.len() > 100 {
                        return Err(error());
                    }
                    for value in raw {
                        let file: BbDiffstat =
                            serde_json::from_value(value.clone()).map_err(|_| error())?;
                        let old = file.old.map(|file| file.path);
                        let new = file.new.map(|file| file.path);
                        let path = new
                            .as_ref()
                            .or(old.as_ref())
                            .filter(|path| snapshot::safe_path(path))
                            .ok_or_else(error)?
                            .clone();
                        if old.as_ref().is_some_and(|path| !snapshot::safe_path(path))
                            || new.as_ref().is_some_and(|path| !snapshot::safe_path(path))
                            || !paths.insert(path.clone())
                        {
                            return Err(error());
                        }
                        let status = match file.status.as_str() {
                            "added" => "added",
                            "removed" => "deleted",
                            "renamed" => "renamed",
                            "modified" => "modified",
                            "copied" => "copied",
                            _ => return Err(error()),
                        };
                        let old_path = old.filter(|old| old != &path);
                        files.push(SnapshotFile {
                            path,
                            old_path,
                            status: status.into(),
                            additions: file.lines_added,
                            deletions: file.lines_removed,
                            binary: false,
                        });
                        if files.len() > snapshot::FILE_LIMIT {
                            return Err(error());
                        }
                    }
                    let next = json
                        .get("next")
                        .filter(|v| !v.is_null())
                        .map(|v| v.as_str().ok_or_else(error))
                        .transpose()?;
                    match self.next_page(next, &collection, page, 100)? {
                        Some(next) => page = next,
                        None => break,
                    }
                }
                let count = files.len();
                let url = format!("{origin}/diff/{head}..{base}?topic=false&renames=true");
                (base, count, Some(files), url)
            }
        };
        if base.len() != head.len() {
            return Err(error());
        }
        let (bytes, _) = self.get_bytes(
            &diff_url,
            started,
            snapshot::PATCH_BYTES,
            "application/vnd.github.diff",
        )?;
        let patch = String::from_utf8(bytes).map_err(|_| error())?;
        let (after, _) = self.get(&pr_url, started)?;
        let (after_detail, _) = parse_metadata(self.repository.provider, &after, params.pr_id)?;
        validate_stable_pull_request_snapshot(&detail, &after_detail)?;
        if self.repository.provider == ReviewProvider::Github
            && before.get("changed_files") != after.get("changed_files")
        {
            return Err(error());
        }
        if self.repository.provider == ReviewProvider::Bitbucket
            && before.pointer("/source/repository/full_name")
                != after.pointer("/source/repository/full_name")
        {
            return Err(error());
        }
        Ok(Capture {
            request: params,
            title: detail.title,
            source_branch: detail.source_branch,
            destination_branch: detail.destination_branch,
            head_sha: head,
            destination_sha: destination,
            base_sha: base,
            patch,
            expected_count,
            expected_files,
        })
    }
}

fn path(value: &str, prefix: &str) -> Result<Option<String>, String> {
    let value = patch_path(value, prefix).map_err(|_| snapshot::invalid())?;
    if value
        .as_ref()
        .is_some_and(|path| !snapshot::safe_path(path))
    {
        return Err(snapshot::invalid());
    }
    Ok(value)
}
fn quoted_token(value: &str) -> Result<(String, &str), String> {
    let bytes = value.as_bytes();
    let mut i = 1;
    while i < bytes.len() {
        if bytes[i] == b'\\' {
            i += 2;
            continue;
        }
        if bytes[i] == b'"' {
            return Ok((
                patch_path(&value[..=i], "")
                    .map_err(|_| snapshot::invalid())?
                    .ok_or_else(snapshot::invalid)?,
                &value[i + 1..],
            ));
        }
        i += 1;
    }
    Err(snapshot::invalid())
}
fn header_paths(value: &str) -> Result<(String, String), String> {
    if value.starts_with('"') {
        let (old, rest) = quoted_token(value)?;
        let rest = rest.strip_prefix(' ').ok_or_else(snapshot::invalid)?;
        let new = if rest.starts_with('"') {
            let (new, trailing) = quoted_token(rest)?;
            if !trailing.is_empty() {
                return Err(snapshot::invalid());
            }
            new
        } else {
            rest.into()
        };
        return Ok((
            old.strip_prefix("a/").ok_or_else(snapshot::invalid)?.into(),
            new.strip_prefix("b/").ok_or_else(snapshot::invalid)?.into(),
        ));
    }
    if let Some(i) = value.find(" \"b/") {
        let old = value[..i]
            .strip_prefix("a/")
            .ok_or_else(snapshot::invalid)?;
        let (new, rest) = quoted_token(&value[i + 1..])?;
        if !rest.is_empty() {
            return Err(snapshot::invalid());
        }
        return Ok((
            old.into(),
            new.strip_prefix("b/").ok_or_else(snapshot::invalid)?.into(),
        ));
    }
    let matches = value
        .match_indices(" b/")
        .filter_map(|(i, _)| {
            let old = value[..i].strip_prefix("a/")?;
            let new = &value[i + 3..];
            (old == new).then(|| (old.to_string(), new.to_string()))
        })
        .collect::<Vec<_>>();
    if matches.len() == 1 {
        return Ok(matches[0].clone());
    }
    // Differing paths require authoritative ---/+++ or rename/copy metadata.
    Err(snapshot::invalid())
}
#[derive(Default)]
struct FileState {
    header: String,
    old: Option<Option<String>>,
    new: Option<Option<String>>,
    from: Option<String>,
    to: Option<String>,
    copied: bool,
    added: bool,
    deleted: bool,
    binary: bool,
    binary_paths: Option<String>,
    additions: u32,
    deletions: u32,
    old_remaining: u32,
    new_remaining: u32,
    old_end: u32,
    new_end: u32,
    has_hunk: bool,
    newline_marker_allowed: bool,
    mode: bool,
    old_mode: Option<String>,
    new_mode: Option<String>,
}
impl FileState {
    fn finish(self, start: usize, end: usize) -> Result<Section, String> {
        if self.old_remaining != 0
            || self.new_remaining != 0
            || self.added && self.deleted
            || self.old_mode.is_some() != self.new_mode.is_some()
        {
            return Err(snapshot::invalid());
        }
        let pair = match (&self.old, &self.new, &self.from, &self.to) {
            (Some(old), Some(new), _, _) => (old.clone(), new.clone()),
            (None, None, Some(old), Some(new)) => (Some(old.clone()), Some(new.clone())),
            (None, None, None, None) => {
                let (old, new) = header_paths(&self.header)?;
                (Some(old), Some(new))
            }
            _ => return Err(snapshot::invalid()),
        };
        if self.added && self.old.as_ref().is_some_and(Option::is_some)
            || self.deleted && self.new.as_ref().is_some_and(Option::is_some)
        {
            return Err(snapshot::invalid());
        }
        let old = if self.added { None } else { pair.0 };
        let new = if self.deleted { None } else { pair.1 };
        let header_old = old
            .as_ref()
            .or(new.as_ref())
            .ok_or_else(snapshot::invalid)?;
        let header_new = new
            .as_ref()
            .or(old.as_ref())
            .ok_or_else(snapshot::invalid)?;
        if self.header.starts_with('"') || self.header.contains(" \"b/") {
            if header_paths(&self.header)? != (header_old.clone(), header_new.clone()) {
                return Err(snapshot::invalid());
            }
        } else if self.header != format!("a/{header_old} b/{header_new}") {
            return Err(snapshot::invalid());
        }
        if self
            .from
            .as_ref()
            .is_some_and(|from| old.as_ref() != Some(from))
            || self.to.as_ref().is_some_and(|to| new.as_ref() != Some(to))
        {
            return Err(snapshot::invalid());
        }
        if !self.has_hunk && !self.binary && !self.mode && self.from.is_none() {
            return Err(snapshot::invalid());
        }
        let file_path = new
            .as_ref()
            .or(old.as_ref())
            .ok_or_else(snapshot::invalid)?
            .clone();
        if [&old, &new]
            .into_iter()
            .flatten()
            .any(|path| !snapshot::safe_path(path))
        {
            return Err(snapshot::invalid());
        }
        if let Some(marker) = &self.binary_paths {
            let valid = marker
                .match_indices(" and ")
                .filter(|(index, _)| {
                    path(&marker[..*index], "a/").ok() == Some(old.clone())
                        && path(&marker[*index + 5..], "b/").ok() == Some(new.clone())
                })
                .count();
            if valid != 1 {
                return Err(snapshot::invalid());
            }
        }
        let status = if old.is_none() {
            "added"
        } else if new.is_none() {
            "deleted"
        } else if old != new {
            if self.copied {
                "copied"
            } else {
                "renamed"
            }
        } else {
            "modified"
        };
        let old_path = old.filter(|old| old != &file_path);
        Ok(snapshot::section(
            SnapshotFile {
                path: file_path,
                old_path,
                status: status.into(),
                additions: self.additions,
                deletions: self.deletions,
                binary: self.binary,
            },
            start,
            end,
        ))
    }
}
pub(crate) fn snapshot_sections(patch: &str) -> Result<Vec<Section>, String> {
    if patch.len() > snapshot::PATCH_BYTES {
        return Err(snapshot::invalid());
    }
    let mut sections = Vec::new();
    let mut state: Option<FileState> = None;
    let mut start = 0;
    let mut offset = 0;
    for raw in patch.split_inclusive('\n') {
        let line = raw.strip_suffix('\n').unwrap_or(raw);
        if let Some(header) = line.strip_prefix("diff --git ") {
            if let Some(previous) = state.take() {
                sections.push(previous.finish(start, offset)?);
            }
            if sections.len() >= snapshot::FILE_LIMIT {
                return Err(snapshot::invalid());
            }
            start = offset;
            state = Some(FileState {
                header: header.into(),
                ..FileState::default()
            });
            offset += raw.len();
            continue;
        }
        let file = state.as_mut().ok_or_else(snapshot::invalid)?;
        if line == "\\ No newline at end of file" {
            if !file.newline_marker_allowed {
                return Err(snapshot::invalid());
            }
            file.newline_marker_allowed = false;
            offset += raw.len();
            continue;
        }
        if file.old_remaining != 0 || file.new_remaining != 0 {
            match line.as_bytes().first() {
                Some(b' ') if file.old_remaining > 0 && file.new_remaining > 0 => {
                    file.old_remaining -= 1;
                    file.new_remaining -= 1;
                }
                Some(b'-') if file.old_remaining > 0 => {
                    file.old_remaining -= 1;
                    file.deletions += 1;
                }
                Some(b'+') if file.new_remaining > 0 => {
                    file.new_remaining -= 1;
                    file.additions += 1;
                }
                _ => return Err(snapshot::invalid()),
            }
            file.newline_marker_allowed = true;
        } else if let Some(hunk) = line.strip_prefix("@@ ") {
            file.newline_marker_allowed = false;
            if file.old.is_none() || file.new.is_none() || file.binary {
                return Err(snapshot::invalid());
            }
            let (ranges, _) = hunk.split_once(" @@").ok_or_else(snapshot::invalid)?;
            let parts = ranges.split_whitespace().collect::<Vec<_>>();
            if parts.len() != 2 {
                return Err(snapshot::invalid());
            }
            let (old, old_count) = range(parts[0], '-').map_err(|_| snapshot::invalid())?;
            let (new, new_count) = range(parts[1], '+').map_err(|_| snapshot::invalid())?;
            if old < file.old_end
                || new < file.new_end
                || old_count == 0 && new_count == 0
                || file.old == Some(None) && old_count != 0
                || file.new == Some(None) && new_count != 0
            {
                return Err(snapshot::invalid());
            }
            file.old_end = old + old_count;
            file.new_end = new + new_count;
            file.old_remaining = old_count;
            file.new_remaining = new_count;
            file.has_hunk = true;
        } else if let Some(value) = line.strip_prefix("--- ") {
            if file.old.is_some() || file.new.is_some() || file.has_hunk {
                return Err(snapshot::invalid());
            }
            file.old = Some(path(value, "a/")?);
        } else if let Some(value) = line.strip_prefix("+++ ") {
            if file.old.is_none() || file.new.is_some() || file.has_hunk {
                return Err(snapshot::invalid());
            }
            file.new = Some(path(value, "b/")?);
        } else if let Some(value) = line
            .strip_prefix("rename from ")
            .or_else(|| line.strip_prefix("copy from "))
        {
            if file.from.is_some() || file.has_hunk {
                return Err(snapshot::invalid());
            }
            file.copied = line.starts_with("copy");
            file.from = path(value, "")?;
        } else if let Some(value) = line
            .strip_prefix("rename to ")
            .or_else(|| line.strip_prefix("copy to "))
        {
            if file.to.is_some() || file.has_hunk || file.copied != line.starts_with("copy") {
                return Err(snapshot::invalid());
            }
            file.to = path(value, "")?;
        } else if line.starts_with("Binary files ") && line.ends_with(" differ") {
            if file.has_hunk || file.binary {
                return Err(snapshot::invalid());
            }
            file.binary = true;
            file.binary_paths = Some(line[13..line.len() - 7].into());
        } else if line.starts_with("new file mode ")
            || line.starts_with("deleted file mode ")
            || line.starts_with("old mode ")
            || line.starts_with("new mode ")
        {
            if file.has_hunk || file.binary {
                return Err(snapshot::invalid());
            }
            let mode = line.rsplit(' ').next().ok_or_else(snapshot::invalid)?;
            if mode.len() != 6 || !mode.bytes().all(|b| (b'0'..=b'7').contains(&b)) {
                return Err(snapshot::invalid());
            }
            if line.starts_with("old mode") && file.old_mode.replace(mode.into()).is_some() {
                return Err(snapshot::invalid());
            }
            if line.starts_with("new mode") && file.new_mode.replace(mode.into()).is_some() {
                return Err(snapshot::invalid());
            }
            if line.starts_with("new file") && file.added
                || line.starts_with("deleted file") && file.deleted
            {
                return Err(snapshot::invalid());
            }
            file.mode = true;
            file.added |= line.starts_with("new file");
            file.deleted |= line.starts_with("deleted file");
        } else if let Some(value) = line.strip_prefix("index ") {
            if file.has_hunk || file.binary {
                return Err(snapshot::invalid());
            }
            let parts = value.split_whitespace().collect::<Vec<_>>();
            if !(1..=2).contains(&parts.len()) {
                return Err(snapshot::invalid());
            }
            let (old, new) = parts[0].split_once("..").ok_or_else(snapshot::invalid)?;
            if [old, new].iter().any(|id| {
                id.is_empty() || id.len() > 64 || !id.bytes().all(|b| b.is_ascii_hexdigit())
            }) || parts.get(1).is_some_and(|mode| {
                mode.len() != 6 || !mode.bytes().all(|b| (b'0'..=b'7').contains(&b))
            }) {
                return Err(snapshot::invalid());
            }
        } else if let Some(value) = line
            .strip_prefix("similarity index ")
            .or_else(|| line.strip_prefix("dissimilarity index "))
        {
            if file.has_hunk
                || file.binary
                || value
                    .strip_suffix('%')
                    .and_then(|v| v.parse::<u32>().ok())
                    .is_none_or(|n| n > 100)
            {
                return Err(snapshot::invalid());
            }
        } else if !line.is_empty() {
            return Err(snapshot::invalid());
        }
        offset += raw.len();
    }
    if let Some(state) = state {
        sections.push(state.finish(start, offset)?);
    }
    let mut paths = HashSet::new();
    // Repeated sections for one provider file are incomplete/contradictory,
    // unlike separate staged/unstaged local layers handled elsewhere.
    // Manager validates the complete independent inventory/count as well.
    for section in &sections {
        if !paths.insert(section.path()) {
            return Err(snapshot::invalid());
        }
    }
    Ok(sections)
}
