//! The browser consumes one explicitly selected immutable local diff layer.
use super::*;
use crate::services::raster::{ImageSide, Raster};
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BrowserParams {
    pub snapshot: SnapshotRef,
    pub layer: Layer,
}
impl BrowserParams {
    pub fn validate(&self) -> Result<(), String> {
        self.snapshot.validate()
    }
}
#[derive(Clone)]
pub struct BrowserSnapshotSource {
    snapshot: SnapshotRef,
    layer: Layer,
    manager: Arc<Manager>,
}
impl std::fmt::Debug for BrowserSnapshotSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LocalBrowserSnapshotSource")
            .field("snapshot", &self.snapshot)
            .field("layer", &self.layer)
            .finish_non_exhaustive()
    }
}
impl PartialEq for BrowserSnapshotSource {
    fn eq(&self, other: &Self) -> bool {
        self.snapshot == other.snapshot
            && self.layer == other.layer
            && Arc::ptr_eq(&self.manager, &other.manager)
    }
}
impl Eq for BrowserSnapshotSource {}
impl BrowserSnapshotSource {
    pub(crate) fn raster(&self, path: &str, side: &str) -> Result<Option<Arc<Raster>>, String> {
        let side = match side {
            "old" => ImageSide::Old,
            "new" => ImageSide::New,
            _ => return Err(invalid()),
        };
        let data = self.manager.get(&self.snapshot)?;
        let canonical = data
            .browser_file(self.layer, path, side)
            .ok_or_else(invalid)?;
        self.manager
            .raster(&self.snapshot, self.layer, &canonical.path, side)
    }
}
impl Manager {
    pub(crate) fn browser_state(
        self: &Arc<Self>,
        params: &BrowserParams,
    ) -> Result<crate::browser_diff::WebDiffState, String> {
        params.validate()?;
        let data = self.get(&params.snapshot)?;
        let patch = data
            .layers()
            .find(|(layer, _)| *layer == params.layer)
            .map(|(_, patch)| patch.to_owned())
            .ok_or_else(invalid)?;
        let layer_name = match params.layer {
            Layer::Committed => "committed",
            Layer::Staged => "staged",
            Layer::Unstaged => "unstaged",
        };
        let diffstat = data
            .browser_files(params.layer)
            .map(|file| {
                let (status, old_path, new_path) = match file.status.as_str() {
                    "added" => ("added", None, Some(file.path.clone())),
                    "deleted" => ("removed", Some(file.path.clone()), None),
                    "renamed" => ("renamed", file.old_path.clone(), Some(file.path.clone())),
                    "copied" => ("copied", file.old_path.clone(), Some(file.path.clone())),
                    _ => ("modified", None, Some(file.path.clone())),
                };
                crate::services::bitbucket::DiffstatEntry {
                    status: status.into(),
                    old_path,
                    new_path,
                    lines_added: file.additions,
                    lines_removed: file.deletions,
                }
            })
            .collect();
        Ok(crate::browser_diff::WebDiffState {
            target_kind: crate::browser_diff::WebDiffTargetKind::Local,
            provider: Some(params.snapshot.repository.provider),
            workspace: params.snapshot.repository.workspace.clone(),
            repo: params.snapshot.repository.repo.clone(),
            pr_title: format!("Local {layer_name} comparison"),
            source_branch: data.info.branch.clone(),
            target_branch: layer_name.into(),
            base_sha: Some(data.info.base_sha.clone()),
            diff: Some(patch),
            diffstat: Some(diffstat),
            local_snapshot_sha256: Some(params.snapshot.snapshot_id.clone()),
            snapshot_source: Some(crate::browser_diff::WebDiffSnapshotSource::Local(
                BrowserSnapshotSource {
                    snapshot: params.snapshot.clone(),
                    layer: params.layer,
                    manager: self.clone(),
                },
            )),
            ..Default::default()
        })
    }
}
