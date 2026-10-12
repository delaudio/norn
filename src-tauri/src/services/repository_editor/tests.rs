use super::*;
#[test]
fn editor_requests_never_accept_absolute_paths_traversal_or_unbound_tokens() {
    let context = Context {
        repository: crate::services::bitbucket::workspace_prs::RepositoryIdentity {
            provider: crate::config::ReviewProvider::Github,
            workspace: "owned".into(),
            repo: "editor".into(),
        },
        root_id: "a".repeat(64),
    };
    let mut params = PrepareParams {
        context: context.clone(),
        path: "src/--owned ü.txt".into(),
        line: Some(1),
        mode: Mode::Terminal,
    };
    assert!(params.validate().is_ok());
    for path in [
        "/outside.txt",
        "../outside.txt",
        "src/../outside.txt",
        "src//file",
        "",
    ] {
        params.path = path.into();
        assert!(params.validate().is_err());
    }
    params.path = "file.txt".into();
    params.line = Some(0);
    assert!(params.validate().is_err());
    assert!(OpenParams {
        context: context.clone(),
        token: "guess".into()
    }
    .validate()
    .is_err());
    assert!(StatusParams {
        context,
        launch_id: "a".repeat(64)
    }
    .validate()
    .is_ok());
    assert!(serde_json::from_value::<PrepareParams>(serde_json::json!({"context":params.context,"path":"file.txt","line":null,"mode":"terminal","program":"sh"})).is_err());
}
