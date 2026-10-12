# Native provider review ownership

Native review storage uses a provider-qualified key with byte-length framed
workspace and repository names, plus the PR number. Runtime session lookup and
cancellation use the same explicit scope. GitHub and Bitbucket PRs with identical
names and numbers own different SQLite rows and session records.

Legacy PR rows remain separate. Native reads project only runs whose saved
provider, workspace, repository and PR number match the requested scope. A
conversation is available only when a matching run proves its association; a
legacy thread ID associated with another scope is ambiguous and cannot supply
messages or an agent resume token. Unknown legacy conversations are preserved
for history reconciliation. Reads do not copy runs into a native row.

The native row owns new runs. A native reply may retain the thread ID and own
its updated conversation while the original run remains in its legacy row.
Reading combines these sources, preferring the native conversation, and rejects
multiple owning rows for the same scoped run ID. Native rows reject foreign run
provenance, missing/duplicate identities, invalid active conversation references,
corrupt JSON or mismatched SQLite scope, and have a 16 MiB storage/read bound.
Explicitly requested foreign IDs keep the existing stale-target error without
returning foreign messages, findings or resume tokens.

Finding staging and publication read the same scoped projection. A confirmed
publication resolves exactly one owning row under the SQLite write transaction
and applies only its narrow metadata/feedback update; it does not serialize the
combined projection back over either source. Missing or ambiguous ownership
stops the update. Provider identity is also checked on persisted feedback.

The library exposes `ProviderReviewScope`, `SavedReviewProvider`, scoped load/save
and scoped session lookup/cancellation helpers. Legacy CLI and desktop helpers
retain their existing storage/key formats. Legacy stale-review cleanup only owns
legacy keys and cannot remove provider-qualified native rows, including when its
keep list is empty.

Native initial and reply execution now use the explicit qualified scope and
canonical configured clone. A reply can import its proved legacy conversation
without copying any legacy run. Provider configuration order does not choose the
clone or agent session. Scoped fix, commit, push and branch merge lookup includes
the provider and conversation identity; legacy desktop wrappers retain their
existing identity. All filesystem actions still require their separate previews
and approvals.

Profiles are applied by the shared Rust resolver, including organization policy.
Unknown explicit profiles fail before agent execution. Native agents execute in
isolated context; required local analyzers need a clean checkout at the pinned
reviewed head. Destination approval pins remain separate from optional actual
diff-base pins, including on replies. This engine integration does not yet start
reviews from a fresh provider preview: durable cached evidence is still required
before preview Enter can be enabled.

Owned core/SQLite and real stdio subprocess fixtures exercise two providers with
the same PR identity, run/thread collisions, corrupt scope, legacy preservation,
transactional publication ownership, independent cancellation and restart.
They use temporary data and repositories, never live provider accounts.
