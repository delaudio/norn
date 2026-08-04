use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;

use crate::{config, credentials};

const NOTION_VERSION: &str = "2022-06-28";

async fn run<T, F>(f: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| e.to_string())?
}

fn http() -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .user_agent("norn")
        .build()
        .map_err(|e| e.to_string())
}

fn get_json<T: DeserializeOwned>(req: reqwest::blocking::RequestBuilder) -> Result<T, String> {
    let resp = req.send().map_err(|e| e.to_string())?;
    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().unwrap_or_default();
        return Err(format!("API error {status}: {body}"));
    }
    resp.json::<T>().map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Jira
// ---------------------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JiraIssue {
    key: String,
    summary: String,
    status: String,
    description_text: String,
    notion_urls: Vec<String>,
}

/// Flatten a Jira ADF document into plain text, collecting link hrefs.
fn adf_to_text(node: &Value, out: &mut String, links: &mut Vec<String>) {
    match node.get("type").and_then(|v| v.as_str()) {
        Some("text") => {
            if let Some(s) = node.get("text").and_then(|v| v.as_str()) {
                out.push_str(s);
            }
            if let Some(marks) = node.get("marks").and_then(|v| v.as_array()) {
                for mark in marks {
                    if mark.get("type").and_then(|v| v.as_str()) == Some("link") {
                        if let Some(href) = mark
                            .get("attrs")
                            .and_then(|a| a.get("href"))
                            .and_then(|v| v.as_str())
                        {
                            links.push(href.to_string());
                        }
                    }
                }
            }
        }
        Some("hardBreak") => out.push('\n'),
        _ => {}
    }
    if let Some(content) = node.get("content").and_then(|v| v.as_array()) {
        for child in content {
            adf_to_text(child, out, links);
        }
        if matches!(
            node.get("type").and_then(|v| v.as_str()),
            Some("paragraph" | "heading" | "listItem" | "blockquote" | "codeBlock")
        ) {
            out.push('\n');
        }
    }
}

#[tauri::command]
pub async fn get_jira_issue(key: String) -> Result<JiraIssue, String> {
    run(move || {
        let base = config::load()
            .jira_base_url
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| "Jira site URL not configured".to_string())?
            .trim_end_matches('/')
            .to_string();
        let creds = credentials::load().ok_or_else(|| "No credentials configured".to_string())?;
        let token =
            credentials::load_jira_token().ok_or_else(|| "No Jira token configured".to_string())?;
        let client = http()?;

        let issue: Value = get_json(
            client
                .get(format!(
                    "{base}/rest/api/3/issue/{key}?fields=summary,description,status"
                ))
                .basic_auth(&creds.username, Some(&token)),
        )?;
        let fields = issue.get("fields").cloned().unwrap_or(Value::Null);
        let summary = fields
            .get("summary")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let status = fields
            .get("status")
            .and_then(|s| s.get("name"))
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();

        let mut description_text = String::new();
        let mut links: Vec<String> = Vec::new();
        if let Some(desc) = fields.get("description") {
            if !desc.is_null() {
                adf_to_text(desc, &mut description_text, &mut links);
            }
        }

        let remotes: Value = get_json(
            client
                .get(format!("{base}/rest/api/3/issue/{key}/remotelink"))
                .basic_auth(&creds.username, Some(&token)),
        )
        .unwrap_or(Value::Array(Vec::new()));
        if let Some(arr) = remotes.as_array() {
            for r in arr {
                if let Some(url) = r
                    .get("object")
                    .and_then(|o| o.get("url"))
                    .and_then(|v| v.as_str())
                {
                    links.push(url.to_string());
                }
            }
        }

        let mut notion_urls: Vec<String> = Vec::new();
        for link in links {
            if (link.contains("notion.so") || link.contains("notion.site"))
                && !notion_urls.contains(&link)
            {
                notion_urls.push(link);
            }
        }

        Ok(JiraIssue {
            key,
            summary,
            status,
            description_text: description_text.trim().to_string(),
            notion_urls,
        })
    })
    .await
}

// ---------------------------------------------------------------------------
// Notion
// ---------------------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NotionPage {
    title: String,
    text: String,
}

/// Extract the 32-hex page id from a Notion URL and format it with dashes.
fn notion_page_id(url: &str) -> Option<String> {
    let hex: String = url.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    if hex.len() < 32 {
        return None;
    }
    let id = &hex[hex.len() - 32..];
    Some(format!(
        "{}-{}-{}-{}-{}",
        &id[0..8],
        &id[8..12],
        &id[12..16],
        &id[16..20],
        &id[20..32]
    ))
}

fn rich_text_to_string(value: &Value) -> String {
    value
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|t| t.get("plain_text").and_then(|v| v.as_str()))
                .collect::<String>()
        })
        .unwrap_or_default()
}

fn block_text(block: &Value) -> Option<String> {
    let ty = block.get("type").and_then(|v| v.as_str())?;
    let rt = block.get(ty)?.get("rich_text")?;
    let text = rich_text_to_string(rt);
    if text.is_empty() {
        return None;
    }
    let prefix = match ty {
        "heading_1" => "# ",
        "heading_2" => "## ",
        "heading_3" => "### ",
        "bulleted_list_item" | "numbered_list_item" => "- ",
        "to_do" => "- [ ] ",
        "quote" => "> ",
        _ => "",
    };
    Some(format!("{prefix}{text}"))
}

#[tauri::command]
pub async fn get_notion_page(url: String) -> Result<NotionPage, String> {
    run(move || {
        let token = credentials::load_notion_token()
            .ok_or_else(|| "No Notion token configured".to_string())?;
        let id =
            notion_page_id(&url).ok_or_else(|| "Could not parse Notion page id".to_string())?;
        let client = http()?;
        let auth = format!("Bearer {token}");

        let page: Value = get_json(
            client
                .get(format!("https://api.notion.com/v1/pages/{id}"))
                .header("Authorization", &auth)
                .header("Notion-Version", NOTION_VERSION),
        )
        .unwrap_or(Value::Null);
        let mut title = String::new();
        if let Some(props) = page.get("properties").and_then(|v| v.as_object()) {
            for prop in props.values() {
                if prop.get("type").and_then(|v| v.as_str()) == Some("title") {
                    title = rich_text_to_string(prop.get("title").unwrap_or(&Value::Null));
                    break;
                }
            }
        }

        let blocks: Value = get_json(
            client
                .get(format!(
                    "https://api.notion.com/v1/blocks/{id}/children?page_size=100"
                ))
                .header("Authorization", &auth)
                .header("Notion-Version", NOTION_VERSION),
        )?;
        let mut lines: Vec<String> = Vec::new();
        if let Some(arr) = blocks.get("results").and_then(|v| v.as_array()) {
            for block in arr {
                if let Some(line) = block_text(block) {
                    lines.push(line);
                }
            }
        }

        Ok(NotionPage {
            title,
            text: lines.join("\n"),
        })
    })
    .await
}
