# agentd

Provider-agnostic sub-agent orchestrator for CLI-driven workflows.

`agentd` lets you create agents, execute plan files, and track execution state/logs with SQLite persistence.

## Table of Contents

- [Overview](#overview)
- [Current Status](#current-status)
- [Prerequisites](#prerequisites)
- [Quickstart](#quickstart)
- [CLI Commands](#cli-commands)
- [Plan File Format](#plan-file-format)
- [Agent State Model](#agent-state-model)
- [Known Limitations](#known-limitations)
- [Troubleshooting](#troubleshooting)
- [Quality Checks](#quality-checks)
- [Project Structure](#project-structure)

## Overview

Core capabilities:

- Generate plans from a goal.
- Execute YAML or JSON plans.
- Spawn agents and attach execution later.
- Inspect status and logs.
- Persist everything in SQLite.

## Current Status

| Area | Status |
| --- | --- |
| Provider `mock` | Implemented |
| Provider `cli` | Stub |
| Provider `http` | Stub |
| Persistence | SQLite (`./.agentd/state.db` by default) |

## Prerequisites

- Rust (Edition 2024)
- Cargo

## Quickstart

Build:

```bash
cargo build
```

Show CLI help:

```bash
cargo run -- --help
```

List agents:

```bash
cargo run -- list
```

Use a custom database path (all commands support this):

```bash
cargo run -- --db-path ./.agentd/state.db list
```

## CLI Commands

| Command | Purpose | Example |
| --- | --- | --- |
| `plan-generate` | Generate a plan from a goal | `cargo run -- plan-generate --goal "prepare report" --provider mock --output ./plan.yaml` |
| `run-plan` | Execute a plan file | `cargo run -- run-plan --file ./plan.yaml --provider mock` |
| `spawn` | Create an agent record (no execution) | `cargo run -- spawn --name "demo" --prompt "Analyze objective" --provider mock --timeout-secs 60 --retries 0` |
| `attach` | Execute an existing agent | `cargo run -- attach --id <AGENT_ID> --timeout-secs 60 --retries 0` |
| `list` | List agents | `cargo run -- list` |
| `status` | Show one agent status | `cargo run -- status --id <AGENT_ID>` |
| `logs` | Show agent logs | `cargo run -- logs --id <AGENT_ID> --limit 100` |
| `pause` | Set agent state to paused | `cargo run -- pause --id <AGENT_ID>` |
| `resume` | Set agent state to running | `cargo run -- resume --id <AGENT_ID>` |
| `stop` | Cancel an agent | `cargo run -- stop --id <AGENT_ID>` |

Notes:

- `spawn` only creates the record. It does not run execution.
- `run-plan` accepts YAML by default; JSON is used when file extension is `.json`.
- Using providers `cli` or `http` currently returns a stub error.

## Plan File Format

Example `plan.yaml`:

```yaml
name: demo-plan
steps:
  - id: step-1
    name: analyze
    prompt: "Analyze objective"
    provider: mock
    depends_on: []
    timeout_secs: 5
    retries: 1
  - id: step-2
    name: execute
    prompt: "Execute objective"
    provider: mock
    depends_on:
      - step-1
    timeout_secs: 10
    retries: 1
```

Step schema:

- `id` (string, required)
- `name` (string, required)
- `prompt` (string, required)
- `provider` (string, optional)
- `depends_on` (string array, optional)
- `timeout_secs` (u64, optional)
- `retries` (u32, optional)

## Agent State Model

Possible states:

- `pending`
- `running`
- `paused`
- `succeeded`
- `failed`
- `cancelled`
- `timed_out`

## Known Limitations

- Only `mock` is fully operational.
- `cli` and `http` providers are present but not implemented.
- Scheduler and restart-recovery are not implemented yet.

## Troubleshooting

### `unknown provider`

Cause: unsupported provider name.

Resolution:

- Use one of `mock`, `cli`, `http`.
- For real execution today, use `mock`.

### `agent not found`

Cause: invalid ID or wrong database path.

Resolution:

- Check ID with `list`.
- Ensure the same `--db-path` is used.

### Plan parsing failure

Cause: invalid YAML/JSON or missing required fields.

Resolution:

- Validate syntax.
- Verify required fields (`id`, `name`, `prompt`).

## Quality Checks

```bash
cargo fmt -- --check
cargo clippy -- -D warnings
cargo test
```

## Project Structure

```text
src/
  main.rs
  cli.rs
  app.rs
  domain/
  ports/
  adapters/

docs/
  TODO.md

.agent/
AGENTS.md
```
