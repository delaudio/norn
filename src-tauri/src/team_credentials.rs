//! Encrypted credential broker for the optional team review service.
//!
//! Secret material is decrypted only while a caller executes an authorized
//! review or publication operation. References, audit events, and errors carry
//! no plaintext credential values.

use std::collections::BTreeMap;
use std::fmt;

use aes_gcm::aead::{Aead, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::{engine::general_purpose::STANDARD_NO_PAD, Engine as _};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const NONCE_BYTES: usize = 12;

#[derive(Clone, PartialEq, Eq)]
pub struct CredentialMasterKey([u8; 32]);

impl CredentialMasterKey {
    /// Loads an external deployment key encoded as base64. The key is never
    /// stored in SQLite, configuration records, or audit payloads.
    pub fn from_base64(value: &str) -> Result<Self, CredentialError> {
        let decoded = STANDARD_NO_PAD
            .decode(value)
            .map_err(|_| CredentialError::InvalidMasterKey)?;
        if decoded.len() != 32 {
            return Err(CredentialError::InvalidMasterKey);
        }
        let mut key = [0_u8; 32];
        key.copy_from_slice(&decoded);
        Ok(Self(key))
    }

    fn cipher(&self) -> Result<Aes256Gcm, CredentialError> {
        Aes256Gcm::new_from_slice(&self.0).map_err(|_| CredentialError::InvalidMasterKey)
    }
}

impl fmt::Debug for CredentialMasterKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("CredentialMasterKey([REDACTED])")
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CredentialReference {
    pub tenant_id: String,
    pub id: String,
    pub provider: String,
}

impl CredentialReference {
    pub fn validate(&self) -> Result<(), CredentialError> {
        for value in [&self.tenant_id, &self.id, &self.provider] {
            if value.trim().is_empty() || value.len() > 512 {
                return Err(CredentialError::InvalidReference);
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CredentialStatus {
    Active,
    Revoked,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CredentialUse {
    ReviewExecution,
    Publication,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CredentialError {
    InvalidMasterKey,
    InvalidReference,
    InvalidSecret,
    NotFound,
    Revoked,
    Unauthorized,
    EncryptionFailed,
    DecryptionFailed,
    Storage(String),
}

impl fmt::Display for CredentialError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::InvalidMasterKey => "credential master key is invalid",
            Self::InvalidReference => "credential reference is invalid",
            Self::InvalidSecret => "credential value is invalid",
            Self::NotFound => "credential reference was not found",
            Self::Revoked => "credential is revoked and cannot be used",
            Self::Unauthorized => "credential use is not authorized",
            Self::EncryptionFailed | Self::DecryptionFailed => "credential operation failed",
            Self::Storage(_) => "credential storage operation failed",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for CredentialError {}

/// Non-serializable plaintext that can only be observed by an authorized
/// execution closure. It intentionally has no Debug or string conversion.
pub struct ResolvedCredential(Vec<u8>);
impl ResolvedCredential {
    pub fn expose_for_provider(&self) -> &[u8] {
        &self.0
    }
}
impl fmt::Debug for ResolvedCredential {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ResolvedCredential([REDACTED])")
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EncryptedCredentialRecord {
    pub reference: CredentialReference,
    pub version: u64,
    pub status: CredentialStatus,
    pub nonce: Vec<u8>,
    pub ciphertext: Vec<u8>,
}

pub trait CredentialStore {
    fn put(&mut self, record: EncryptedCredentialRecord) -> Result<(), CredentialError>;
    fn current(
        &self,
        reference: &CredentialReference,
    ) -> Result<Option<EncryptedCredentialRecord>, CredentialError>;
    fn revoke(&mut self, reference: &CredentialReference) -> Result<(), CredentialError>;
}

pub trait CredentialAuthorizer {
    fn authorize(
        &self,
        reference: &CredentialReference,
        use_case: CredentialUse,
    ) -> Result<(), CredentialError>;
}

pub trait CredentialAuditSink {
    fn credential_changed(
        &self,
        reference: &CredentialReference,
        version: u64,
        status: CredentialStatus,
    ) -> Result<(), CredentialError>;
}

#[derive(Default)]
pub struct InMemoryCredentialStore {
    records: BTreeMap<(String, String), EncryptedCredentialRecord>,
}
impl CredentialStore for InMemoryCredentialStore {
    fn put(&mut self, record: EncryptedCredentialRecord) -> Result<(), CredentialError> {
        self.records.insert(
            (
                record.reference.tenant_id.clone(),
                record.reference.id.clone(),
            ),
            record,
        );
        Ok(())
    }
    fn current(
        &self,
        reference: &CredentialReference,
    ) -> Result<Option<EncryptedCredentialRecord>, CredentialError> {
        Ok(self
            .records
            .get(&(reference.tenant_id.clone(), reference.id.clone()))
            .cloned())
    }
    fn revoke(&mut self, reference: &CredentialReference) -> Result<(), CredentialError> {
        let record = self
            .records
            .get_mut(&(reference.tenant_id.clone(), reference.id.clone()))
            .ok_or(CredentialError::NotFound)?;
        record.status = CredentialStatus::Revoked;
        Ok(())
    }
}

/// Durable identifier and ciphertext store. The deployment master key remains
/// outside this database and all audit payloads.
#[derive(Default)]
pub struct SqliteCredentialStore;
impl CredentialStore for SqliteCredentialStore {
    fn put(&mut self, record: EncryptedCredentialRecord) -> Result<(), CredentialError> {
        crate::review_storage::put_team_credential(&record)
    }
    fn current(
        &self,
        reference: &CredentialReference,
    ) -> Result<Option<EncryptedCredentialRecord>, CredentialError> {
        crate::review_storage::current_team_credential(reference)
    }
    fn revoke(&mut self, reference: &CredentialReference) -> Result<(), CredentialError> {
        crate::review_storage::revoke_team_credential(reference)
    }
}

pub struct CredentialBroker<S, A, U> {
    master_key: CredentialMasterKey,
    store: S,
    authorizer: A,
    audit: U,
}
impl<S, A, U> CredentialBroker<S, A, U>
where
    S: CredentialStore,
    A: CredentialAuthorizer,
    U: CredentialAuditSink,
{
    pub fn new(master_key: CredentialMasterKey, store: S, authorizer: A, audit: U) -> Self {
        Self {
            master_key,
            store,
            authorizer,
            audit,
        }
    }
    pub fn create(
        &mut self,
        reference: CredentialReference,
        secret: &[u8],
    ) -> Result<u64, CredentialError> {
        self.write(reference, secret, 1)
    }
    pub fn rotate(
        &mut self,
        reference: CredentialReference,
        secret: &[u8],
    ) -> Result<u64, CredentialError> {
        let version = self
            .store
            .current(&reference)?
            .map(|record| record.version.saturating_add(1))
            .unwrap_or(1);
        self.write(reference, secret, version)
    }
    pub fn revoke(&mut self, reference: &CredentialReference) -> Result<(), CredentialError> {
        reference.validate()?;
        self.store.revoke(reference)?;
        let version = self
            .store
            .current(reference)?
            .ok_or(CredentialError::NotFound)?
            .version;
        self.audit
            .credential_changed(reference, version, CredentialStatus::Revoked)
    }
    pub fn with_for_use<T>(
        &self,
        reference: &CredentialReference,
        use_case: CredentialUse,
        operation: impl FnOnce(&ResolvedCredential) -> Result<T, CredentialError>,
    ) -> Result<T, CredentialError> {
        reference.validate()?;
        self.authorizer.authorize(reference, use_case)?;
        let record = self
            .store
            .current(reference)?
            .ok_or(CredentialError::NotFound)?;
        if record.status == CredentialStatus::Revoked {
            return Err(CredentialError::Revoked);
        }
        let secret = decrypt(&self.master_key, &record)?;
        operation(&ResolvedCredential(secret))
    }
    fn write(
        &mut self,
        reference: CredentialReference,
        secret: &[u8],
        version: u64,
    ) -> Result<u64, CredentialError> {
        reference.validate()?;
        if secret.is_empty() || secret.len() > 65_536 {
            return Err(CredentialError::InvalidSecret);
        }
        let (nonce, ciphertext) = encrypt(&self.master_key, secret)?;
        self.store.put(EncryptedCredentialRecord {
            reference: reference.clone(),
            version,
            status: CredentialStatus::Active,
            nonce,
            ciphertext,
        })?;
        self.audit
            .credential_changed(&reference, version, CredentialStatus::Active)?;
        Ok(version)
    }
}

fn encrypt(
    key: &CredentialMasterKey,
    secret: &[u8],
) -> Result<(Vec<u8>, Vec<u8>), CredentialError> {
    let mut nonce = [0_u8; NONCE_BYTES];
    aes_gcm::aead::rand_core::RngCore::fill_bytes(&mut OsRng, &mut nonce);
    let ciphertext = key
        .cipher()?
        .encrypt(Nonce::from_slice(&nonce), secret)
        .map_err(|_| CredentialError::EncryptionFailed)?;
    Ok((nonce.to_vec(), ciphertext))
}
fn decrypt(
    key: &CredentialMasterKey,
    record: &EncryptedCredentialRecord,
) -> Result<Vec<u8>, CredentialError> {
    if record.nonce.len() != NONCE_BYTES {
        return Err(CredentialError::DecryptionFailed);
    }
    key.cipher()?
        .decrypt(Nonce::from_slice(&record.nonce), record.ciphertext.as_ref())
        .map_err(|_| CredentialError::DecryptionFailed)
}

/// Stable non-secret audit/reference identifier. Never hash the secret itself.
pub fn credential_reference_id(tenant_id: &str, provider: &str, external_id: &str) -> String {
    hex::encode(Sha256::digest(
        format!("{tenant_id}\0{provider}\0{external_id}").as_bytes(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Default)]
    struct Authorizer {
        allowed: bool,
    }
    impl CredentialAuthorizer for Authorizer {
        fn authorize(
            &self,
            _: &CredentialReference,
            _: CredentialUse,
        ) -> Result<(), CredentialError> {
            if self.allowed {
                Ok(())
            } else {
                Err(CredentialError::Unauthorized)
            }
        }
    }
    #[derive(Default)]
    struct Audit {
        events: std::cell::RefCell<Vec<(u64, CredentialStatus)>>,
    }
    impl CredentialAuditSink for Audit {
        fn credential_changed(
            &self,
            _: &CredentialReference,
            version: u64,
            status: CredentialStatus,
        ) -> Result<(), CredentialError> {
            self.events.borrow_mut().push((version, status));
            Ok(())
        }
    }
    fn key() -> CredentialMasterKey {
        CredentialMasterKey::from_base64(&STANDARD_NO_PAD.encode([7_u8; 32])).expect("key")
    }
    fn reference() -> CredentialReference {
        CredentialReference {
            tenant_id: "tenant-acme".to_string(),
            id: "bitbucket-workspace".to_string(),
            provider: "bitbucket".to_string(),
        }
    }
    #[test]
    fn encrypts_rotates_and_never_exposes_plaintext_in_records() {
        let mut broker = CredentialBroker::new(
            key(),
            InMemoryCredentialStore::default(),
            Authorizer { allowed: true },
            Audit::default(),
        );
        assert_eq!(
            broker.create(reference(), b"first-secret").expect("create"),
            1
        );
        assert_eq!(
            broker
                .rotate(reference(), b"second-secret")
                .expect("rotate"),
            2
        );
        let record = broker
            .store
            .current(&reference())
            .expect("record")
            .expect("stored");
        assert!(!String::from_utf8_lossy(&record.ciphertext).contains("second-secret"));
        assert_eq!(
            broker
                .with_for_use(&reference(), CredentialUse::ReviewExecution, |credential| {
                    Ok(credential.expose_for_provider().to_vec())
                })
                .expect("resolve"),
            b"second-secret"
        );
    }
    #[test]
    fn revoked_and_unauthorized_credentials_cannot_be_resolved() {
        let mut broker = CredentialBroker::new(
            key(),
            InMemoryCredentialStore::default(),
            Authorizer { allowed: false },
            Audit::default(),
        );
        broker.create(reference(), b"secret").expect("create");
        assert_eq!(
            broker
                .with_for_use(&reference(), CredentialUse::Publication, |_| Ok(()))
                .expect_err("unauthorized"),
            CredentialError::Unauthorized
        );
        broker.authorizer.allowed = true;
        broker.revoke(&reference()).expect("revoke");
        assert_eq!(
            broker
                .with_for_use(&reference(), CredentialUse::Publication, |_| Ok(()))
                .expect_err("revoked"),
            CredentialError::Revoked
        );
    }
}
