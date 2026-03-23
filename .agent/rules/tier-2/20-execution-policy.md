# Skill 20 — Execution Policy

> Enforce safe execution invariants for agent lifecycle and scheduling.

## Scope

This skill applies to:
- attach/spawn/stop execution flows in `src/app.rs`
- execution lock coordination (`try_acquire_execution_lock`, `release_execution_lock`)
- restart recovery and idempotence semantics

## Policy Rules

- Never execute the same agent concurrently when an execution lock exists.
- Every attach path must release lock on terminal outcomes (`succeeded`, `failed`, `timed_out`, `cancelled`).
- Recovery must repair stale in-progress states before accepting new work.
- Lifecycle logs must preserve context (`context`, `provider`, `category`, `message`).

## Failure Handling

- When lock acquisition fails, return a deterministic user-facing error.
- If state update fails after lock acquisition, release the lock immediately.
- Cancellation paths should be best-effort for provider kill, then enforce state consistency in store.

## Verification Checklist

- duplicate attach is rejected
- restart recovery restores a runnable state
- lock lifecycle is tested in integration tests
- structured observability remains present on all critical paths

<!-- Updated: 2026-03-24 -->
