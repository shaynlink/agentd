# Skill 03 — Runtime Adapter Standards

> Build runtime capabilities with strict hexagonal boundaries and deterministic precedence.

## Scope

This skill applies to:
- `src/ports/runtime.rs`, `src/ports/process.rs`, `src/ports/resource.rs`
- `src/adapters/runtimes/*`
- runtime flow integration in `src/app.rs` and sandbox/provider adapters

## Port/Adapter Rules

- Domain and app code MUST depend on runtime ports, never concrete adapter internals.
- Runtime adapters MUST expose explicit lifecycle operations (execute, spawn, kill/terminate).
- Adapter factories MUST fail fast on unknown runtime keys with actionable errors.
- Runtime selection MUST be resolved explicitly (plan -> CLI -> config).

## Execution Rules

- Timeouts are mandatory for process execution paths.
- Exit codes and output must be surfaced deterministically.
- Avoid global mutable state for per-request runtime decisions.
- Keep background process APIs explicit and observable.

## Observability Rules

- Include runtime identifier in trace metadata.
- Propagate execution duration/resource signals when available.
- Errors must preserve context (runtime, command, target workdir).

## Verification Checklist

- runtime precedence covered by tests
- unknown runtime produces deterministic failure
- adapter paths pass `cargo check`, `clippy -D warnings`, and targeted tests

<!-- Updated: 2026-03-24 -->
