# Skill 24 — Shell Injection and Host Replication

> Safely support custom shells and host shell replication in runtime environments.

## Scope

Use this skill when implementing:
- custom shell injection/configuration
- host shell profile replication
- shell runtime compatibility and fallback behavior

## Rules

- Shell selection must be explicit and validated before execution.
- Host replication should copy behavior intentionally, not blindly copy secrets.
- Shell config precedence must remain deterministic and documented.
- Unsupported shell capabilities must fail with actionable errors.

## Security Considerations

- Do not execute untrusted shell init scripts without policy checks.
- Avoid inheriting sensitive env vars by default.
- Record shell identity and source in execution traces.

## Verification Checklist

- selected shell is observable in runtime metadata
- shell fallback behavior is deterministic
- command execution works across default and injected shells

<!-- Updated: 2026-03-24 -->
