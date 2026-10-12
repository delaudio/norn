use super::*;
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InfoParams {
    pub repository: RepositoryIdentity,
}
impl InfoParams {
    pub fn validate(&self) -> Result<(), String> {
        self.repository.validate()
    }
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Info {
    pub request: InfoParams,
    pub context: Context,
    pub branch: Option<String>,
    pub head_sha: Option<String>,
}
pub(crate) struct Manager {
    cancellation: LocalReviewCancellation,
}
impl Manager {
    pub fn new() -> Self {
        Self {
            cancellation: LocalReviewCancellation::new(),
        }
    }
    pub fn stop(&self) {
        self.cancellation.cancel();
    }
    fn control(&self) -> GitRunControl {
        GitRunControl::new(
            Duration::from_secs(20),
            self.cancellation.clone(),
            "Repository exploration",
        )
    }
    pub fn info(&self, params: InfoParams) -> Result<Info, String> {
        params.validate()?;
        let control = self.control();
        control.check()?;
        let source = context::resolve(&params.repository, &control)?;
        let head_sha = git::head(&source.root, &control)?;
        let output = git::run(
            &source.root,
            &["symbolic-ref", "--short", "-q", "HEAD"],
            4096,
            &control,
        )?;
        let branch = if output.status.success() {
            Some(
                String::from_utf8(output.stdout)
                    .map_err(|_| invalid())?
                    .trim_end_matches(['\r', '\n'])
                    .to_owned(),
            )
        } else if output.status.code() == Some(1) {
            None
        } else {
            return Err(invalid());
        };
        let context = Context {
            repository: params.repository.clone(),
            root_id: source.root_id,
        };
        if context::resolve_context(&context, &control)? != source.root
            || git::head(&source.root, &control)? != head_sha
        {
            return Err(invalid());
        }
        Ok(Info {
            request: params,
            context,
            branch,
            head_sha,
        })
    }
    pub fn files(&self, params: FilesParams) -> Result<inventory::FilesPage, String> {
        inventory::page(params, &self.control())
    }
    pub fn content(&self, params: ContentParams) -> Result<content::ContentPage, String> {
        content::page(params, &self.control())
    }
    pub fn diff(&self, params: DiffParams) -> Result<diff::DiffPage, String> {
        diff::page(params, &self.control())
    }
    pub fn blame(&self, params: BlameParams) -> Result<blame::BlamePage, String> {
        blame::page(params, &self.control())
    }
    pub fn find(&self, params: FindParams) -> Result<find::FindPage, String> {
        find::page(params, &self.control())
    }
}
