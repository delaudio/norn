# Security Policy

## Reporting a Vulnerability

Please report security issues privately through GitHub Security Advisories for
this repository when available.

Do not open a public issue for vulnerabilities that expose credentials,
private repository data, or local machine paths.

## Secret Handling

Norn is a local desktop application. Repository credentials and integration
tokens should stay in the operating system keychain or environment variables;
they must not be committed to repository config, examples, or test fixtures.

If a credential is accidentally committed, rotate it immediately and remove it
from any public history before publishing the repository.
