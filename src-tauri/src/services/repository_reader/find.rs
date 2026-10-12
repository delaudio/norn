use super::*;
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FindSource {
    Content,
    Diff,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FindParams {
    pub context: Context,
    pub path: String,
    pub source: FindSource,
    pub query: String,
    pub offset: usize,
    pub limit: usize,
    pub content_sha256: String,
}
impl FindParams {
    pub fn validate(&self) -> Result<(), String> {
        self.context.validate()?;
        context::validate_path(&self.path)?;
        if self.query.is_empty()
            || self.query.len() > 256
            || self.query.contains('\0')
            || self.offset > MAX_DIFF
            || !(1..=32).contains(&self.limit)
            || !valid_hash(&self.content_sha256)
        {
            return Err(invalid());
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Match {
    pub start_utf16: usize,
    pub end_utf16: usize,
    pub line: usize,
    pub column_utf16: usize,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FindPage {
    pub request: FindParams,
    pub total: usize,
    pub matches: Vec<Match>,
    pub next_offset: Option<usize>,
}

pub(crate) fn matches(
    text: &str,
    query: &str,
    offset: usize,
    limit: usize,
    control: &GitRunControl,
) -> Result<(usize, Vec<Match>), String> {
    let lower = text.to_lowercase();
    let needle = query.to_lowercase();
    if needle.is_empty() {
        return Err(invalid());
    }
    let mut selected = Vec::new();
    let mut total = 0;
    // Map lowercase byte offsets to original UTF-16 positions in one pass.
    // Unicode lowercase can expand a character; never return a mid-character
    // position or count UTF-8 bytes as JavaScript string indices.
    let mut chars = text.chars().peekable();
    let (mut lowered, mut utf16, mut line, mut column) = (0usize, 0usize, 1usize, 0usize);
    for (start, _) in lower.match_indices(&needle) {
        control.check()?;
        total += 1;
        if total <= offset || selected.len() == limit {
            continue;
        }
        while let Some(&ch) = chars.peek() {
            let width = ch.to_lowercase().map(char::len_utf8).sum::<usize>();
            if lowered + width > start {
                break;
            }
            chars.next();
            lowered += width;
            utf16 += ch.len_utf16();
            if ch == '\n' {
                line += 1;
                column = 0;
            } else {
                column += ch.len_utf16();
            }
        }
        let (start_utf16, start_line, start_column) = (utf16, line, column);
        let mut end_utf16 = utf16;
        let mut end_lower = lowered;
        for ch in chars.clone() {
            if end_lower >= start + needle.len() {
                break;
            }
            end_lower += ch.to_lowercase().map(char::len_utf8).sum::<usize>();
            end_utf16 += ch.len_utf16();
        }
        selected.push(Match {
            start_utf16,
            end_utf16,
            line: start_line,
            column_utf16: start_column,
        });
    }
    if offset > total {
        return Err(invalid());
    }
    Ok((total, selected))
}
pub(crate) fn page(params: FindParams, control: &GitRunControl) -> Result<FindPage, String> {
    params.validate()?;
    let root = context::resolve_context(&params.context, control)?;
    let text = match params.source {
        FindSource::Content => {
            let inventory = inventory::collect(&root, control)?;
            if !inventory.files.iter().any(|file| file.path == params.path) {
                return Err(invalid());
            }
            let document = content::read_at(&root, &params.path, control)?;
            if document.state != ContentState::Text {
                return Err("Find is available for text previews only.".into());
            }
            document.text
        }
        FindSource::Diff => diff::read_at(&root, &params.path, control)?.0,
    };
    if hash(text.as_bytes()) != params.content_sha256 {
        return Err("Preview changed. Refresh before searching.".into());
    }
    let (total, found) = matches(&text, &params.query, params.offset, params.limit, control)?;
    if context::resolve_context(&params.context, control)? != root {
        return Err(invalid());
    }
    let next = params.offset + found.len();
    Ok(FindPage {
        request: params,
        total,
        matches: found,
        next_offset: (next < total).then_some(next),
    })
}
