//! Bounded saved-local navigation, independent of the current checkout/config.
use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ListParams {
    pub offset: usize,
    pub limit: usize,
    #[serde(deserialize_with = "required_version")]
    pub catalog_version: Option<String>,
}
fn required_version<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    Option::<String>::deserialize(d)
}
impl ListParams {
    pub fn validate(&self) -> Result<(), String> {
        if self.offset > 1000
            || !(1..=32).contains(&self.limit)
            || self.catalog_version.as_ref().is_some_and(|v| !valid_id(v))
            || self.offset > 0 && self.catalog_version.is_none()
        {
            return Err(invalid());
        }
        Ok(())
    }
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub snapshot: SnapshotRef,
    pub created_at: String,
    pub mode: Mode,
    pub branch: String,
    pub base_sha: String,
    pub layers: Vec<Layer>,
    pub file_count: usize,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListPage {
    pub request: ListParams,
    pub catalog_version: String,
    pub snapshots: Vec<Summary>,
    pub total: usize,
    pub next_offset: Option<usize>,
}
impl Manager {
    pub(crate) fn list(&self, request: ListParams) -> Result<ListPage, String> {
        request.validate()?;
        self.check()?;
        let entries = crate::review_storage::local_evidence::catalogue()?;
        let encoded = serde_json::to_vec(&entries).map_err(|_| invalid())?;
        if encoded.len() > 1024 * 1024 {
            return Err(invalid());
        }
        let version = format!("{:x}", Sha256::digest(&encoded));
        if request
            .catalog_version
            .as_ref()
            .is_some_and(|v| v != &version)
            || request.offset > entries.len()
        {
            return Err("Saved local snapshots changed. Refresh the list explicitly.".into());
        }
        let total = entries.len();
        let mut snapshots = Vec::new();
        for (snapshot, created_at) in entries.into_iter().skip(request.offset).take(request.limit) {
            self.check()?;
            let data = self.get(&snapshot)?;
            snapshots.push(Summary {
                snapshot,
                created_at,
                mode: data.info.comparison.mode,
                branch: data.info.branch.clone(),
                base_sha: data.info.base_sha.clone(),
                layers: data.info.comparison.layers.clone(),
                file_count: data.info.file_count,
            });
        }
        let end = request.offset + snapshots.len();
        Ok(ListPage {
            request,
            catalog_version: version,
            snapshots,
            total,
            next_offset: (end < total).then_some(end),
        })
    }
}
