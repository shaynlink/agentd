# Skill 01 — Rust and CLI Standards

> Build robust, async-safe CLI orchestration code in `agentd`.

## Scope

This skill applies to:
- Rust code under `src/`
- command and argument design in `src/cli.rs`
- provider implementations (`src/adapters/providers/*`)
- persistence and state transitions

## Architecture Rules

- Keep domain logic in `src/domain/*`.
- Keep integration details in adapters (`src/adapters/*`).
- Depend on ports from application/domain, not adapter internals.
- Prefer explicit error propagation with `anyhow::Result`.

## CLI Rules

- New behavior should be wired via explicit `clap` subcommands/flags.
- Keep command names action-oriented and consistent with existing verbs.
- Validate input early and return actionable errors.

## Async and Process Rules

- Never block the async runtime with long synchronous operations.
- Use `tokio::process::Command` for external process execution.
- Always capture stdout/stderr and map failures into typed/structured messages.
- Handle cancellation and timeout paths explicitly.

## Quality Gates

- `cargo fmt -- --check`
- `cargo clippy -- -D warnings`
- `cargo test`

## Completion Checklist

- behavior implemented
- edge cases handled (timeout/cancel/retry)
- storage state remains coherent
- quality gates pass

<!-- Updated: 2026-03-23 -->
