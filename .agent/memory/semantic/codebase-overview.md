<!-- codebase-overview.md — Retrieval-Aware Format -->
<!-- Updated: 2026-03-23 -->

## INDEX
- [CLI Surface](#cli-surface) — clap, commands, args, parsing
- [Application Flow](#application-flow) — app service, run plan, spawn/attach
- [Provider Layer](#provider-layer) — provider port, mock/cli/http adapters
- [Runtime Layer](#runtime-layer) — runtime ports, builtin adapter, precedence
- [Persistence Layer](#persistence-layer) — sqlite store, agents, logs, state

## <section id="cli-surface"> CLI Surface

- Entrypoint: `src/main.rs` calls `agentd::cli::run()`.
- CLI definitions: `src/cli.rs`.
- Current commands include: `run-plan`, `plan-generate`, `spawn`, `attach`, `list`, `pause`, `resume`, `stop`, `status`, `logs`, schedule commands.
- `--db-path` defaults to `./.agentd/state.db`.

</section>

## <section id="application-flow"> Application Flow

- `src/app.rs` wires use cases and ports.
- Flow style:
  - parse CLI command
  - execute app method
  - interact with provider/store adapters
  - report status/logs

</section>

## <section id="provider-layer"> Provider Layer

- Port: `src/ports/provider.rs`.
- Adapters: `src/adapters/providers/`.
- Status:
  - `mock.rs` operational
  - `cli_provider.rs` operational
  - `http_provider.rs` operational
  - `sandbox_provider.rs` operational with ACL and diff tracing hooks
- Expected output mapping: provider response -> domain/app result + logs.

</section>

## <section id="runtime-layer"> Runtime Layer

- Ports: `src/ports/runtime.rs`, `src/ports/process.rs`, `src/ports/resource.rs`.
- Domain: `src/domain/runtime_config.rs`, `src/domain/process_handle.rs`, `src/domain/resource_limit.rs`.
- Adapter factory: `src/adapters/runtimes/mod.rs`.
- Built-in adapter: `src/adapters/runtimes/builtin.rs`.
- Runtime precedence is resolved explicitly: plan step runtime -> CLI runtime override -> config runtime.

</section>

## <section id="persistence-layer"> Persistence Layer

- Port: `src/ports/store.rs`.
- Adapter: `src/adapters/store/sqlite.rs`.
- DB file: default `./.agentd/state.db`.
- Persists agent records and execution logs.
- Must remain coherent across pause/resume/stop and retries.

</section>
