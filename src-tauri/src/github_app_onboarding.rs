//! Provider-neutral boundary for GitHub App installation onboarding.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

pub const REQUIRED_PERMISSIONS: &[(&str, &str)] = &[
    ("contents", "read"),
    ("metadata", "read"),
    ("pull_requests", "write"),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GithubAppInstallationStatus {
    Active,
    Suspended,
    Uninstalled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GithubAppRepository {
    pub id: u64,
    pub owner: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GithubAppInstallation {
    pub id: u64,
    pub organization_login: String,
    pub status: GithubAppInstallationStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GithubAppEnrollment {
    pub tenant_id: String,
    pub installation_id: u64,
    pub organization_login: String,
    pub status: GithubAppInstallationStatus,
    pub repositories: Vec<GithubAppRepository>,
}

pub trait GithubAppApi {
    fn installation(&self, installation_id: u64) -> Result<GithubAppInstallation, String>;
    fn repositories(&self, installation_id: u64) -> Result<Vec<GithubAppRepository>, String>;
    fn configure_webhook(&self, installation_id: u64, repository_id: u64) -> Result<(), String>;
}

pub trait GithubAppEnrollmentStore {
    fn save(&mut self, enrollment: GithubAppEnrollment) -> Result<(), String>;
    fn load(&self, tenant_id: &str) -> Result<Option<GithubAppEnrollment>, String>;
}

#[derive(Debug, Default)]
pub struct InMemoryGithubAppEnrollmentStore {
    enrollments: BTreeMap<String, GithubAppEnrollment>,
}

/// Durable local store for installation and repository identifiers. It never
/// stores an installation access token or a personal access token.
#[derive(Debug, Default)]
pub struct SqliteGithubAppEnrollmentStore;

impl GithubAppEnrollmentStore for SqliteGithubAppEnrollmentStore {
    fn save(&mut self, enrollment: GithubAppEnrollment) -> Result<(), String> {
        crate::review_storage::save_github_app_enrollment(&enrollment)
    }

    fn load(&self, tenant_id: &str) -> Result<Option<GithubAppEnrollment>, String> {
        crate::review_storage::load_github_app_enrollment(tenant_id)
    }
}

impl GithubAppEnrollmentStore for InMemoryGithubAppEnrollmentStore {
    fn save(&mut self, enrollment: GithubAppEnrollment) -> Result<(), String> {
        self.enrollments
            .insert(enrollment.tenant_id.clone(), enrollment);
        Ok(())
    }

    fn load(&self, tenant_id: &str) -> Result<Option<GithubAppEnrollment>, String> {
        Ok(self.enrollments.get(tenant_id).cloned())
    }
}

pub struct GithubAppOnboarding<A, S> {
    app_slug: String,
    api: A,
    store: S,
}

impl<A, S> GithubAppOnboarding<A, S>
where
    A: GithubAppApi,
    S: GithubAppEnrollmentStore,
{
    pub fn new(app_slug: String, api: A, store: S) -> Result<Self, String> {
        if app_slug.trim().is_empty() {
            return Err("GitHub App slug must not be empty".to_string());
        }
        Ok(Self {
            app_slug,
            api,
            store,
        })
    }

    pub fn installation_url(&self, state: &str) -> Result<String, String> {
        if state.trim().is_empty()
            || state
                .bytes()
                .any(|byte| !byte.is_ascii_alphanumeric() && byte != b'-' && byte != b'_')
        {
            return Err("Installation state must be an opaque URL-safe value".to_string());
        }
        Ok(format!(
            "https://github.com/apps/{}/installations/new?state={state}",
            self.app_slug
        ))
    }

    pub fn complete_installation(
        &mut self,
        tenant_id: &str,
        installation_id: u64,
        selected_repository_ids: &[u64],
    ) -> Result<GithubAppEnrollment, String> {
        validate_tenant(tenant_id)?;
        if installation_id == 0 || selected_repository_ids.is_empty() {
            return Err(
                "An installation and at least one selected repository are required".to_string(),
            );
        }
        let installation = self.api.installation(installation_id)?;
        if installation.id != installation_id {
            return Err(
                "GitHub App installation response does not match requested installation"
                    .to_string(),
            );
        }
        if installation.status != GithubAppInstallationStatus::Active {
            return Err("GitHub App installation is not active".to_string());
        }
        let available = self.api.repositories(installation_id)?;
        let selected = select_repositories(&available, selected_repository_ids)?;
        for repository in &selected {
            self.api.configure_webhook(installation_id, repository.id)?;
        }
        let enrollment = GithubAppEnrollment {
            tenant_id: tenant_id.to_string(),
            installation_id,
            organization_login: installation.organization_login,
            status: GithubAppInstallationStatus::Active,
            repositories: selected,
        };
        self.store.save(enrollment.clone())?;
        Ok(enrollment)
    }

    pub fn remove_repository(
        &mut self,
        tenant_id: &str,
        repository_id: u64,
    ) -> Result<GithubAppEnrollment, String> {
        let mut enrollment = self.require_enrollment(tenant_id)?;
        let repository_count = enrollment.repositories.len();
        enrollment
            .repositories
            .retain(|repository| repository.id != repository_id);
        if enrollment.repositories.len() == repository_count {
            return Err("Repository is not enrolled for tenant".to_string());
        }
        self.store.save(enrollment.clone())?;
        Ok(enrollment)
    }

    pub fn record_installation_status(
        &mut self,
        tenant_id: &str,
        installation_id: u64,
        status: GithubAppInstallationStatus,
    ) -> Result<GithubAppEnrollment, String> {
        let mut enrollment = self.require_enrollment(tenant_id)?;
        if enrollment.installation_id != installation_id {
            return Err("Installation does not belong to tenant".to_string());
        }
        enrollment.status = status;
        self.store.save(enrollment.clone())?;
        Ok(enrollment)
    }

    pub fn allows_repository(&self, tenant_id: &str, repository_id: u64) -> Result<bool, String> {
        Ok(self.store.load(tenant_id)?.is_some_and(|enrollment| {
            enrollment.status == GithubAppInstallationStatus::Active
                && enrollment
                    .repositories
                    .iter()
                    .any(|repository| repository.id == repository_id)
        }))
    }

    fn require_enrollment(&self, tenant_id: &str) -> Result<GithubAppEnrollment, String> {
        validate_tenant(tenant_id)?;
        self.store
            .load(tenant_id)?
            .ok_or_else(|| "No GitHub App enrollment for tenant".to_string())
    }
}

fn validate_tenant(tenant_id: &str) -> Result<(), String> {
    if tenant_id.trim().is_empty() {
        Err("Tenant id must not be empty".to_string())
    } else {
        Ok(())
    }
}

fn select_repositories(
    available: &[GithubAppRepository],
    selected_ids: &[u64],
) -> Result<Vec<GithubAppRepository>, String> {
    let ids = selected_ids.iter().copied().collect::<BTreeSet<_>>();
    if ids.len() != selected_ids.len() || ids.contains(&0) {
        return Err("Selected repository ids must be unique positive values".to_string());
    }
    let selected = available
        .iter()
        .filter(|repository| ids.contains(&repository.id))
        .cloned()
        .collect::<Vec<_>>();
    if selected.len() != ids.len() {
        return Err("Selected repository is not available to the installation".to_string());
    }
    Ok(selected)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    #[derive(Default)]
    struct MockGithubApi {
        installation: Option<GithubAppInstallation>,
        repositories: Vec<GithubAppRepository>,
        webhooks: RefCell<Vec<u64>>,
    }

    impl GithubAppApi for MockGithubApi {
        fn installation(&self, _: u64) -> Result<GithubAppInstallation, String> {
            self.installation
                .clone()
                .ok_or_else(|| "missing installation".to_string())
        }
        fn repositories(&self, _: u64) -> Result<Vec<GithubAppRepository>, String> {
            Ok(self.repositories.clone())
        }
        fn configure_webhook(&self, _: u64, repository_id: u64) -> Result<(), String> {
            self.webhooks.borrow_mut().push(repository_id);
            Ok(())
        }
    }

    fn api(status: GithubAppInstallationStatus) -> MockGithubApi {
        MockGithubApi {
            installation: Some(GithubAppInstallation {
                id: 7,
                organization_login: "acme".to_string(),
                status,
            }),
            repositories: vec![
                GithubAppRepository {
                    id: 11,
                    owner: "acme".to_string(),
                    name: "api".to_string(),
                },
                GithubAppRepository {
                    id: 12,
                    owner: "acme".to_string(),
                    name: "web".to_string(),
                },
            ],
            webhooks: RefCell::new(Vec::new()),
        }
    }

    #[test]
    fn completion_enrolls_only_selected_repositories_and_configures_webhooks() {
        let api = api(GithubAppInstallationStatus::Active);
        let mut onboarding = GithubAppOnboarding::new(
            "lachesi".to_string(),
            api,
            InMemoryGithubAppEnrollmentStore::default(),
        )
        .expect("onboarding");
        let enrollment = onboarding
            .complete_installation("tenant-acme", 7, &[12])
            .expect("complete");
        assert_eq!(
            enrollment
                .repositories
                .iter()
                .map(|repository| repository.id)
                .collect::<Vec<_>>(),
            vec![12]
        );
        assert!(onboarding
            .allows_repository("tenant-acme", 12)
            .expect("access"));
        assert!(!onboarding
            .allows_repository("tenant-acme", 11)
            .expect("access"));
    }

    #[test]
    fn completion_requires_explicit_selection_from_the_installation() {
        let mut onboarding = GithubAppOnboarding::new(
            "lachesi".to_string(),
            api(GithubAppInstallationStatus::Active),
            InMemoryGithubAppEnrollmentStore::default(),
        )
        .expect("onboarding");
        assert!(onboarding
            .complete_installation("tenant-acme", 7, &[])
            .is_err());
        assert!(onboarding
            .complete_installation("tenant-acme", 7, &[99])
            .is_err());
    }

    #[test]
    fn suspension_and_uninstall_disable_webhook_and_publication_access() {
        let mut onboarding = GithubAppOnboarding::new(
            "lachesi".to_string(),
            api(GithubAppInstallationStatus::Active),
            InMemoryGithubAppEnrollmentStore::default(),
        )
        .expect("onboarding");
        onboarding
            .complete_installation("tenant-acme", 7, &[11])
            .expect("complete");
        for status in [
            GithubAppInstallationStatus::Suspended,
            GithubAppInstallationStatus::Uninstalled,
        ] {
            onboarding
                .record_installation_status("tenant-acme", 7, status)
                .expect("status");
            assert!(!onboarding
                .allows_repository("tenant-acme", 11)
                .expect("denied"));
        }
    }

    #[test]
    fn repository_removal_revokes_enrollment_without_removing_installation() {
        let mut onboarding = GithubAppOnboarding::new(
            "lachesi".to_string(),
            api(GithubAppInstallationStatus::Active),
            InMemoryGithubAppEnrollmentStore::default(),
        )
        .expect("onboarding");
        onboarding
            .complete_installation("tenant-acme", 7, &[11, 12])
            .expect("complete");
        let enrollment = onboarding
            .remove_repository("tenant-acme", 11)
            .expect("remove");
        assert_eq!(enrollment.installation_id, 7);
        assert_eq!(enrollment.repositories.len(), 1);
        assert!(!onboarding
            .allows_repository("tenant-acme", 11)
            .expect("denied"));
    }
}
