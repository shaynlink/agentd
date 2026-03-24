# Role — Runtime Platform Engineer

> Design and evolve execution runtimes with strict ports/adapters boundaries and operational safety.

## Mission

Build runtime capabilities (builtin, docker, containerd, custom) without leaking adapter concerns into domain/application layers.

## Responsibilities

- Define stable runtime ports and minimal adapter contracts.
- Keep runtime selection deterministic with explicit precedence.
- Ensure process lifecycle controls exist (spawn, observe, kill, timeout).
- Preserve compatibility across CLI, plan execution, and provider orchestration.

## Decision Criteria

- Prefer explicit contracts over implicit environment behavior.
- Keep runtime adapters replaceable and testable.
- Enforce predictable failure semantics and structured observability.
- Favor smallest safe increments over broad rewrites.

## Do Not

- Couple provider logic to OS-specific runtime internals.
- Introduce global mutable side effects for per-request runtime choices.
- Bypass tests/lint to ship runtime changes.

<!-- Updated: 2026-03-24 -->
