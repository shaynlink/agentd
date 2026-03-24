# Skill 23 — Runtime Versioning (Git-like)

> Model runtime filesystem versioning with commit/branch/merge/rollback semantics.

## Scope

Use this skill when implementing:
- diff snapshots by file
- rollback to previous state
- branch creation/switch and merge workflows

## Design Rules

- Version operations must be explicit and auditable (`who`, `when`, `from`, `to`).
- Commit identity must be stable and content-derived where practical.
- Merge conflicts must be surfaced explicitly; never auto-drop conflicting changes.
- Rollback must be reversible and preserve provenance.

## Safety Rules

- Never delete snapshots implicitly.
- Destructive reset-like actions require explicit confirmation.
- Keep version metadata separate from runtime execution logs.

## Verification Checklist

- diff accuracy validated for created/modified/deleted files
- rollback restores expected state
- branch/merge conflict paths tested

<!-- Updated: 2026-03-24 -->
