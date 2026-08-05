# Team credential broker

The optional team service stores provider credentials through the credential
broker. Local desktop keychain credentials remain outside this mechanism.

## Encryption and references

The deployment supplies a 32-byte external master key as base64. Norn uses
AES-256-GCM with a fresh 96-bit nonce for every credential version. The service
database stores only tenant, opaque credential reference, provider, version,
status, nonce, and ciphertext. It never stores the master key or plaintext.

Review jobs and publication requests carry credential references, never secret
values. A service operation must be authorized for the reference and its use
case before the broker decrypts it. Decrypted material is exposed only to the
provider-call closure and is not serializable or printable.

## Lifecycle

- Create stores version 1 of a reference.
- Rotate writes a new encrypted version and moves only new resolutions to it;
  historical review data is unchanged.
- Revoke marks the reference inactive. Review execution and publication fail
  before any provider operation can start.
- Audit integrations receive only reference id, version, and lifecycle status.
  Plaintext credentials, ciphertext, nonces, and master-key material are not
  valid audit fields.
