# Release Process

Altru Browser uses evidence-gated releases.

## 1. Candidate

Create a release candidate from reviewed source.

## 2. Audit

Run:

```bash
scripts/audit-community-release.sh
```

The audit must pass on the exact candidate commit.

## 3. Review

Confirm:

- changelog;
- dependency/license review;
- security notes;
- conformance claims;
- platform matrix;
- unsupported capabilities;
- release notes.

## 4. Tag

Only a commit that passed the release gate may be tagged.

## 5. Publish

Publish source/releases with capability claims that match the recorded evidence.

## 6. Post-release

If evidence is later invalidated, document the problem, correct the affected claim and issue a new release rather than rewriting history.
