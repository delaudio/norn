//! Validate a requested single-line anchor against the exact provider patch.
use crate::draft_publication::{ApprovedComment, CommentMode};
use std::collections::HashSet;

fn invalid() -> String {
    "The diff position could not be verified. Choose a supported comment position explicitly."
        .into()
}
fn patch_path(value: &str, prefix: &str) -> Result<Option<String>, String> {
    let path = if value.starts_with('"') {
        let bytes = value.as_bytes();
        let mut out = Vec::new();
        let mut index = 1;
        loop {
            let byte = *bytes.get(index).ok_or_else(invalid)?;
            index += 1;
            match byte {
                b'"' => {
                    if !bytes[index..].is_empty() && bytes[index] != b'\t' {
                        return Err(invalid());
                    }
                    break;
                }
                b'\\' => {
                    let escape = *bytes.get(index).ok_or_else(invalid)?;
                    index += 1;
                    match escape {
                        b'\\' | b'"' => out.push(escape),
                        b'n' => out.push(b'\n'),
                        b't' => out.push(b'\t'),
                        b'r' => out.push(b'\r'),
                        b'b' => out.push(8),
                        b'f' => out.push(12),
                        b'v' => out.push(11),
                        b'0'..=b'7' => {
                            let second = *bytes.get(index).ok_or_else(invalid)?;
                            let third = *bytes.get(index + 1).ok_or_else(invalid)?;
                            if !matches!(second, b'0'..=b'7') || !matches!(third, b'0'..=b'7') {
                                return Err(invalid());
                            }
                            let number = u16::from(escape - b'0') * 64
                                + u16::from(second - b'0') * 8
                                + u16::from(third - b'0');
                            out.push(u8::try_from(number).map_err(|_| invalid())?);
                            index += 2;
                        }
                        _ => return Err(invalid()),
                    }
                }
                other => out.push(other),
            }
        }
        String::from_utf8(out).map_err(|_| invalid())?
    } else {
        value.split('\t').next().ok_or_else(invalid)?.into()
    };
    if path == "/dev/null" {
        return Ok(None);
    }
    let path = path.strip_prefix(prefix).ok_or_else(invalid)?;
    if path.is_empty() || path.len() > 4096 {
        return Err(invalid());
    }
    Ok(Some(path.into()))
}
fn range(value: &str, sign: char) -> Result<(u32, u32), String> {
    let raw = value.strip_prefix(sign).ok_or_else(invalid)?;
    let (start, count) = raw.split_once(',').unwrap_or((raw, "1"));
    if start.is_empty()
        || count.is_empty()
        || !start.bytes().all(|byte| byte.is_ascii_digit())
        || !count.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(invalid());
    }
    let start: u32 = start.parse().map_err(|_| invalid())?;
    let count: u32 = count.parse().map_err(|_| invalid())?;
    if (start == 0 && count != 0) || start.checked_add(count).is_none() {
        return Err(invalid());
    }
    Ok((start, count))
}
pub(super) fn validate_inline_positions(
    patch: &str,
    comments: &[ApprovedComment],
) -> Result<(), String> {
    let expected: HashSet<_> = comments
        .iter()
        .filter(|comment| comment.mode == CommentMode::Inline && comment.parent_id.is_none())
        .map(|comment| {
            (
                comment.path.clone(),
                comment.from.is_some(),
                comment.to.or(comment.from).unwrap_or(0),
            )
        })
        .collect();
    if expected.is_empty() {
        return Ok(());
    }
    let mut found = HashSet::new();
    let mut old_path = None;
    let mut new_path = None;
    let mut headers_complete = false;
    let mut old_header_seen = false;
    let mut old_line = 0u32;
    let mut new_line = 0u32;
    let mut old_remaining = 0u32;
    let mut new_remaining = 0u32;
    for line in patch.lines() {
        if line.starts_with("diff --git ") {
            if old_remaining != 0 || new_remaining != 0 {
                return Err(invalid());
            }
            old_path = None;
            new_path = None;
            headers_complete = false;
            old_header_seen = false;
            continue;
        }
        if line.starts_with("@@ ") {
            if old_remaining != 0
                || new_remaining != 0
                || !headers_complete
                || (old_path.is_none() && new_path.is_none())
            {
                return Err(invalid());
            }
            let header = line
                .strip_prefix("@@ ")
                .and_then(|value| value.split_once(" @@").map(|pair| pair.0))
                .ok_or_else(invalid)?;
            let parts = header.split_whitespace().collect::<Vec<_>>();
            if parts.len() != 2 {
                return Err(invalid());
            }
            (old_line, old_remaining) = range(parts[0], '-')?;
            (new_line, new_remaining) = range(parts[1], '+')?;
            if (old_path.is_none() && old_remaining != 0)
                || (new_path.is_none() && new_remaining != 0)
            {
                return Err(invalid());
            }
            continue;
        }
        if old_remaining == 0 && new_remaining == 0 {
            if let Some(value) = line.strip_prefix("--- ") {
                old_path = patch_path(value, "a/")?;
                new_path = None;
                headers_complete = false;
                old_header_seen = true;
            }
            if let Some(value) = line.strip_prefix("+++ ") {
                if !old_header_seen {
                    return Err(invalid());
                }
                old_header_seen = false;
                new_path = patch_path(value, "b/")?;
                headers_complete = true;
            }
            continue;
        }
        if line.starts_with("\\ No newline at end of file") {
            continue;
        }
        let origin = line.as_bytes().first().copied().ok_or_else(invalid)?;
        let (old, new) = match origin {
            b' ' => (true, true),
            b'-' => (true, false),
            b'+' => (false, true),
            _ => return Err(invalid()),
        };
        // Providers use the changed file's new path, or old path for deletion.
        // A renamed old path is not silently relocated to the new file.
        let path = new_path
            .as_ref()
            .or(old_path.as_ref())
            .ok_or_else(invalid)?;
        if old {
            if old_remaining == 0 {
                return Err(invalid());
            }
            let candidate = (path.clone(), true, old_line);
            if expected.contains(&candidate) {
                found.insert(candidate);
            }
            old_line = old_line.checked_add(1).ok_or_else(invalid)?;
            old_remaining -= 1;
        }
        if new {
            if new_remaining == 0 {
                return Err(invalid());
            }
            let candidate = (path.clone(), false, new_line);
            if expected.contains(&candidate) {
                found.insert(candidate);
            }
            new_line = new_line.checked_add(1).ok_or_else(invalid)?;
            new_remaining -= 1;
        }
    }
    if old_remaining != 0 || new_remaining != 0 || found != expected {
        return Err(invalid());
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{config::ReviewProvider, draft_publication::PublicationScope};
    fn comment(path: &str, old: bool, line: u32) -> ApprovedComment {
        ApprovedComment {
            scope: PublicationScope {
                provider: ReviewProvider::Github,
                workspace: "fixture".into(),
                repo: "demo".into(),
                pr_id: 9,
            },
            local_id: "fixture".into(),
            head_sha: "a".repeat(40),
            base_sha: "b".repeat(40),
            mode: CommentMode::Inline,
            path: path.into(),
            to: (!old).then_some(line),
            from: old.then_some(line),
            parent_id: None,
            raw: "fixture".into(),
            finding_ref: None,
            original_draft_hash: None,
        }
    }
    #[test]
    fn unchanged_old_new_and_quoted_unicode_space_paths_are_exact() {
        let patch = "diff --git a/old b/new\n--- a/old name.rs\n+++ b/new name.rs\n@@ -4,2 +8,2 @@ function\n context\n-old\n+new\ndiff --git a/unicode b/unicode\n--- \"a/\\303\\250.rs\"\n+++ \"b/\\303\\250.rs\"\n@@ -1 +1 @@\n-old\n+new\n";
        assert!(validate_inline_positions(
            patch,
            &[
                comment("new name.rs", true, 5),
                comment("new name.rs", false, 9),
                comment("new name.rs", false, 8),
                comment("è.rs", true, 1)
            ]
        )
        .is_ok());
        assert!(validate_inline_positions(patch, &[comment("old name.rs", true, 5)]).is_err());
        assert!(validate_inline_positions(patch, &[comment("new name.rs", false, 5)]).is_err());
        assert!(validate_inline_positions(patch, &[comment("è.rs", false, 2)]).is_err());
    }
    #[test]
    fn deletion_new_file_and_header_like_source_lines_do_not_change_paths() {
        let patch = "diff --git a/file b/file\n--- a/file\n+++ /dev/null\n@@ -1,2 +0,0 @@\n--- header-like source\n-old\n\\ No newline at end of file\ndiff --git a/added b/added\n--- /dev/null\n+++ b/added\n@@ -0,0 +1,2 @@\n+++ header-like source\n+new\n";
        assert!(validate_inline_positions(
            patch,
            &[comment("file", true, 1), comment("added", false, 1)]
        )
        .is_ok());
        assert!(validate_inline_positions(patch, &[comment("file", false, 1)]).is_err());
    }
    #[test]
    fn malformed_or_truncated_hunks_and_paths_cannot_approve() {
        for patch in [
            "+++ b/file\n@@ -0,0 +1 @@\n+new\n",
            "--- /dev/null\n+++ b/file\n+++ b/file\n@@ -0,0 +1 @@\n+new\n",
            "--- a/file\n+++ b/file\n@@ -1,2 +1,2 @@\n-old\n+new\n",
            "--- a/file\n+++ b/file\n@@ -4294967295 +1 @@\n-old\n+new\n",
            "--- a/file\n+++ b/file\n@@ -0 +1 @@\n-old\n+new\n",
            "--- \"a/\\999\"\n+++ b/file\n@@ -1 +1 @@\n-old\n+new\n",
        ] {
            assert!(validate_inline_positions(patch, &[comment("file", false, 1)]).is_err());
        }
    }
}
