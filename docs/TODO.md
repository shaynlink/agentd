# Next Steps

This document lists the next milestones required to evolve the current MVP into a full CLI for sub-agent orchestration.

## Current Status

- Working CLI commands: `run-plan`, `plan-generate`, `spawn`, `attach`, `list`, `pause`, `resume`, `stop`, `status`, `logs`
- Ports/adapters architecture is in place
- SQLite persistence is in place (agents + logs)
- `mock` provider is operational
- `cli` provider is operational for execution (`run_agent`)
- `http` provider is operational for execution (`run_agent`)
- provider-agnostic config is available via `agentd.toml` + env overrides

## Priority P0 (Do First)

- [x] Implement concrete CLI provider
  - Target file: `src/adapters/providers/cli_provider.rs`
  - Minimum support:
    - spawn a configurable process (command + args)
    - send prompt via stdin or argument
    - capture stdout/stderr
    - graceful cancellation (`cancel`)
  - Acceptance criteria:
    - `--provider cli` executes a real agent

- [x] Implement concrete HTTP provider
  - Target file: `src/adapters/providers/http_provider.rs`
  - Minimum support:
    - configurable endpoint
    - auth with bearer/api-key header
    - `run_agent` request handling
    - response mapping to `ProviderRunResult`
  - Acceptance criteria:
    - `--provider http` executes an agent against a remote API

- [x] Add provider-agnostic configuration
  - Proposed new files:
    - `src/config.rs`
    - `agentd.toml` (optional)
  - Minimum content:
    - default provider profile
    - provider -> options mapping (endpoint, token env var, CLI command)
  - Acceptance criteria:
    - providers are no longer hardcoded

## Priority P1 (Runtime Stabilization)

- [x] Add `run-at` and `cron` scheduler
  - Proposed new modules:
    - `src/scheduler/mod.rs`
    - `src/scheduler/engine.rs`
  - SQLite extension:
    - `schedules` table
    - `schedule_runs` table
  - Acceptance criteria:
    - schedule command creates and triggers planned runs
  - Progress:
    - `schedule-run-at` implemented
    - `schedule-cron` implemented
    - `schedule-list` implemented
    - `schedule-dispatch-due` implemented

- [x] Add restart recovery
  - Extend SQLite schema to store in-progress execution
  - Rehydrate agents on startup
  - Prevent duplicate execution (idempotence)
  - Acceptance criteria:
    - after kill/restart, state remains consistent and execution can resume
  - Progress:
    - `execution_locks` table added
    - startup recovery moves `running` agents to `pending`
    - duplicate concurrent `attach` prevented by execution lock

- [x] Add real-time streaming
  - Live output while running `attach`
  - Optional structured output (JSON lines)
  - Acceptance criteria:
    - continuous provider output is visible in real time
  - Progress:
    - `attach` supports `--stream` and `--json-lines`
    - live streaming implemented for `cli` provider

## Priority P2 (Quality and Hardening)

- [x] Enforce strict state transitions
  - Target file: `src/domain/agent.rs`
  - Block invalid transitions (`Succeeded -> Running`, etc.)

- [ ] Add unit and integration tests
  - Proposed folder: `tests/`
  - Target coverage:
    - JSON/YAML plan parsing
    - retries/timeouts
    - core CLI flows
    - mock/stub providers
  - Progress:
    - state transition enforcement tests added
    - pause/resume transition rule tests added
    - restart recovery/lock release tests added
    - attach retries on provider error integration test added
    - attach timeout retries integration test added
    - scheduler dispatch (run-at) integration test added
    - scheduler cron replan integration test added

- [ ] Improve error handling and observability
  - clearer error categories
  - structured logs (level, context, provider)

- [ ] Add end-user documentation
  - add `README.md` with:
    - installation
    - command examples
    - YAML/JSON plan examples
    - provider configuration

## Recommended Execution Order

1. Concrete CLI provider
2. Concrete HTTP provider
3. Provider-agnostic configuration
4. Scheduler (`run-at`/`cron`)
5. Restart recovery
6. Real-time streaming
7. Testing and hardening
8. Complete README

## Definition of Done (MVP+)

- All three providers (`mock`, `cli`, `http`) are executable
- Scheduled runs survive restart
- CLI exposes readable output and optional JSON output
- Critical paths are covered by tests

## Agent OS Integration TODO

- [x] Add Agent OS scaffold files: `.agent/` + `AGENTS.md`
- [x] Configure `AGENTS.md` for `agentd` (stack, architecture, repository map)
- [x] Add initial Tier-1 Rust skill (`.agent/rules/tier-1/01-rust-and-cli-standards.md`)
- [x] Add default role (`.agent/roles/rust-orchestrator.md`)
- [x] Add semantic memory map (`.agent/memory/semantic/codebase-overview.md`)
- [ ] Add project-specific Tier-2 domain skills (execution policy, retry strategy, plan safety)
- [ ] Add semantic maps for `domain-models`, `provider-contracts`, `sqlite-schema`
- [ ] Add episodic memory format for postmortems/RCA after failed runs
- [ ] Expand Task Detection Table with concrete signal patterns from real tickets
