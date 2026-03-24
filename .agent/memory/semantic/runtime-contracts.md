<!-- runtime-contracts.md — Retrieval-Aware Format -->
<!-- Updated: 2026-03-24 -->

## INDEX
- [Runtime Ports](#runtime-ports) — runtime, process, resource contracts
- [Builtin Runtime Adapter](#builtin-runtime-adapter) — execute/spawn/kill baseline
- [Security Adapter](#security-adapter) — command/file checks and audit persistence
- [Runtime Precedence](#runtime-precedence) — plan, CLI override, config
- [Sandbox Integration](#sandbox-integration) — provider runtime resolution and tracing

## <section id="runtime-ports"> Runtime Ports

Files:
- `src/ports/runtime.rs`
- `src/ports/process.rs`
- `src/ports/resource.rs`

Contracts:
- `RuntimePort`: `execute`, `spawn_background`, runtime identity
- `ProcessPort`: process spawn + kill
- `ResourcePort`: usage retrieval + limit enforcement hooks

Design intent:
- app/provider flows depend on ports
- concrete runtime logic lives in adapters

</section>

## <section id="builtin-runtime-adapter"> Builtin Runtime Adapter

File:
- `src/adapters/runtimes/builtin.rs`

Behavior:
- executes commands with timeout, captures stdout/stderr
- returns deterministic `ProcessExecutionResult`
- supports background spawn and kill via host process tools

Current limitation:
- CPU/RAM metrics are placeholder; wall-time is tracked

</section>

## <section id="security-adapter"> Security Adapter

Files:
- `src/ports/securable.rs`
- `src/adapters/security/local_securable.rs`
- `src/adapters/security/mod.rs`

Behavior:
- role-based command gating (`viewer` denied execution)
- path access checks for runtime workdir
- persistent audit logging using backend selection:
	- `file` backend (JSON-lines file)
	- `sqlite` backend (`security_audit_logs` table)
	- configured via `AGENTD_SANDBOX_AUDIT_BACKEND` and `AGENTD_SANDBOX_AUDIT_LOG_PATH`
- audit retrieval exposed through `SecurablePort::list_audit_events` and surfaced by CLI `audit-list`

</section>

## <section id="runtime-precedence"> Runtime Precedence

File:
- `src/domain/runtime_config.rs`

Resolution order:
1. plan step runtime override
2. CLI runtime override
3. config runtime

Purpose:
- deterministic runtime choice without global mutable side effects

</section>

## <section id="sandbox-integration"> Sandbox Integration

File:
- `src/adapters/providers/sandbox_provider.rs`

Behavior:
- resolves runtime via precedence config
- routes process runtime through builtin runtime adapter
- includes resolved runtime in tracing metadata

</section>
