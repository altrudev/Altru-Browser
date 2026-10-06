# Security Policy

Altru Browser is experimental and is not yet recommended for protecting high-value browsing sessions.

## Reporting a vulnerability

Please do **not** publish exploitable security issues in a public GitHub issue.

Use GitHub's private vulnerability-reporting / Security Advisory flow for this repository when available. If that path is unavailable, contact the project owner privately through Altru.dev.

Include reproduction steps, affected revision, impact, and only the proof-of-concept material needed to understand the issue.

## Security model

The project aims to:

- fail closed on unsupported or ambiguous execution;
- isolate platform-specific authority behind explicit interfaces;
- minimize permissions and unnecessary data collection;
- bind execution evidence to the exact artifact and implementation state;
- keep private assurance state and credentials outside the public repository.

See `docs/THREAT-MODEL.md` and `docs/PRIVACY.md`.

Security maturity is tracked separately from feature completeness. Passing functional tests does not imply production security.
