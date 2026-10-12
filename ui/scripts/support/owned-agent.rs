//! Standalone, std-only fake agent for the owned native workflow gate.
use std::{
    io::{Read, Write},
    path::PathBuf,
    time::Duration,
};
fn main() {
    let owned =
        PathBuf::from(std::env::var_os("NORN_OWNED_AGENT_DIR").expect("owned agent directory"));
    let mut payload = String::new();
    std::io::stdin()
        .read_to_string(&mut payload)
        .expect("owned input");
    std::fs::write(owned.join("payload"), payload).expect("owned payload receipt");
    std::fs::write(owned.join("started"), std::process::id().to_string())
        .expect("owned start receipt");
    if std::fs::read_to_string(owned.join("mode")).unwrap_or_default() == "slow" {
        std::thread::sleep(Duration::from_secs(20));
    }
    let result = r#"{"result":"Owned workflow review.\n\n```json\n{\"schemaVersion\":\"norn.review.v1\",\"findings\":[{\"title\":\"Owned workflow finding\",\"body\":\"Inspect the selected local layer.\",\"severity\":\"major\",\"category\":\"bug\",\"confidence\":\"high\",\"file\":\"same.txt\",\"line\":1}]}\n```","session_id":"private-owned-workflow-session"}"#;
    writeln!(std::io::stdout(), "{result}").expect("owned result");
    std::fs::write(owned.join("completed"), "owned").expect("owned completion receipt");
}
