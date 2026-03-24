# Skill 04 — Security and Permissions Standards

> Enforce least-privilege runtime execution with auditable policy checks.

## Scope

This skill applies to:
- command/file permission checks in runtime/sandbox flows
- ACL/RBAC policy modeling in domain and port contracts
- command execution logs and security event logs

## Security Rules

- Command execution must be policy-gated before process spawn.
- File read/write access must be evaluated against explicit allowed sets.
- Sensitive operations require explicit user intent and clear error messages.
- Security checks must fail closed (deny on uncertainty/errors where possible).

## Logging Rules

- Log security decisions (`allowed`/`denied`) with context.
- Never log secrets (tokens, passwords, API keys).
- Keep logs structured for filtering/incident triage.

## Access Model Guidance

- Prefer hybrid ACL + RBAC modeling for complex runtime use cases.
- Keep policy definition in domain, policy enforcement in adapters.
- Add integration tests for both allowed and denied paths.

## Verification Checklist

- denied command path tested
- allowed command path tested
- no credential leakage in log messages

<!-- Updated: 2026-03-24 -->
