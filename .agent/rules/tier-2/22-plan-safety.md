# Skill 22 — Plan Safety

> Execute plans deterministically and reject malformed plan inputs early.

## Scope

This skill applies to:
- `run-plan` file parsing (YAML/JSON)
- provider fallback behavior for plan steps
- step-level timeout/retry policy defaults

## Safety Rules

- Parse by extension and emit precise parse errors (`JSON` vs `YAML`).
- Require explicit step identity (`id`, `name`) and prompt content.
- Default missing provider to requested command/provider context.
- Use conservative defaults when timeout/retry fields are absent.

## Execution Rules

- Execute steps in plan order unless an explicit DAG engine is introduced.
- Preserve per-step timeout/retry overrides when present.
- Persist each executed step as an agent record for traceability.

## Verification Checklist

- YAML and JSON plan files are both covered by integration tests
- plan parse failures surface validation errors to CLI stderr category output
- executed plan steps are visible via persisted agents/logs

<!-- Updated: 2026-03-24 -->
