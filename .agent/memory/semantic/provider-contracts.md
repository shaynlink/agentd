<!-- provider-contracts.md — Retrieval-Aware Format -->
<!-- Updated: 2026-03-24 -->

## INDEX
- [Provider Port](#provider-port) — trait, run request/result, cancel semantics
- [Mock Provider](#mock-provider) — deterministic local behavior
- [CLI Provider](#cli-provider) — process execution, stream, cancellation
- [HTTP Provider](#http-provider) — endpoint/auth mapping and output extraction

## <section id="provider-port"> Provider Port

Port file: `src/ports/provider.rs`

Contract:
- trait methods:
  - `generate_plan(goal)`
  - `run_agent(request)`
  - `cancel(agent_id)` (default no-op)
- run request fields:
  - `agent_id`, `prompt`, `timeout_secs`, `stream_output`, `json_lines`
- run result:
  - `output` string

General rules:
- providers must return `anyhow::Result`
- provider-specific failures are surfaced to app retry policy
- cancellation should be idempotent where possible

</section>

## <section id="mock-provider"> Mock Provider

Adapter file: `src/adapters/providers/mock.rs`

Behavior:
- deterministic synthetic plan generation for tests/dev
- run path returns synthetic success output after short async wait

Best use:
- integration tests without external dependencies
- baseline command flow validation

</section>

## <section id="cli-provider"> CLI Provider

Adapter file: `src/adapters/providers/cli_provider.rs`

Behavior:
- spawns configurable process with command/args
- prompt delivery via stdin or argument mode
- supports streaming stdout/stderr and optional JSON-lines emission
- writes pid file for cancellation path
- cancel sends TERM then KILL fallback

Failure modes:
- process spawn failure
- non-zero status with stderr/stdout detail
- timeout managed by app-level wrapper

</section>

## <section id="http-provider"> HTTP Provider

Adapter file: `src/adapters/providers/http_provider.rs`

Behavior:
- POST JSON payload to configured endpoint
- supports auth modes: none, bearer, api-key
- extracts output from JSON body (`output`, nested result/data output) or raw text

Failure modes:
- transport errors
- non-success HTTP status
- empty response body
- misconfigured auth inputs

</section>
