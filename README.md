# agentd

Provider-agnostic sub-agent orchestrator for CLI-driven workflows.

`agentd` lets you create agents, execute plan files, and track execution state/logs with SQLite persistence.

## Table of Contents

- [Overview](#overview)
- [Current Status](#current-status)
- [Prerequisites](#prerequisites)
- [Quickstart](#quickstart)
- [Configuration](#configuration)
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
| Provider `cli` | Implemented for agent execution and plan generation |
| Provider `http` | Implemented for agent execution (`run_agent`) |
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

## Configuration

By default, `agentd` reads `agentd.toml` at repository root.
You can override the path with `AGENTD_CONFIG`.

Example config:

```toml
default_provider = "mock"

[providers.cli]
command = "cat"
args = []
prompt_mode = "stdin"
prompt_flag = "--prompt"
runtime_dir = "./.agentd/runtime"

[providers.http]
endpoint = "http://localhost:8080/run-agent"
auth_mode = "none"
api_key_header = "x-api-key"
```

Precedence order:

1. Environment variables
2. `agentd.toml`
3. Built-in defaults

## CLI Commands

| Command | Purpose | Example |
| --- | --- | --- |
| `plan-generate` | Generate a plan from a goal | `cargo run -- plan-generate --goal "prepare report" --output ./plan.yaml` |
| `run-plan` | Execute a plan file | `cargo run -- run-plan --file ./plan.yaml` |
| `spawn` | Create an agent record (no execution) | `cargo run -- spawn --name "demo" --prompt "Analyze objective" --timeout-secs 60 --retries 0` |
| `attach` | Execute an existing agent | `cargo run -- attach --id <AGENT_ID> --timeout-secs 60 --retries 0 --stream true` |
| `list` | List agents | `cargo run -- list` |
| `status` | Show one agent status | `cargo run -- status --id <AGENT_ID>` |
| `logs` | Show agent logs | `cargo run -- logs --id <AGENT_ID> --limit 100` |
| `pause` | Set agent state to paused | `cargo run -- pause --id <AGENT_ID>` |
| `resume` | Set agent state to running | `cargo run -- resume --id <AGENT_ID>` |
| `stop` | Cancel an agent | `cargo run -- stop --id <AGENT_ID>` |
| `schedule-run-at` | Create one-shot schedule at a datetime | `cargo run -- schedule-run-at --name "nightly" --prompt "Run checks" --run-at "2026-03-24T23:00:00Z"` |
| `schedule-cron` | Create recurring schedule from cron expression | `cargo run -- schedule-cron --name "hourly" --prompt "Run checks" --cron "0 0 * * * * *"` |
| `schedule-list` | List schedules | `cargo run -- schedule-list --limit 100` |
| `schedule-dispatch-due` | Execute due schedules | `cargo run -- schedule-dispatch-due --limit 50` |

Notes:

- `spawn` only creates the record. It does not run execution.
- `--provider` is optional for `plan-generate`, `run-plan`, and `spawn`.
  If omitted, `default_provider` from config is used.
- `run-plan` accepts YAML by default; JSON is used when file extension is `.json`.
- `attach --stream true` enables live output while the provider runs.
- `attach --json-lines true` emits structured JSON lines for streamed output.
- `schedule-run-at --run-at` expects an RFC3339 datetime (UTC recommended).
- `schedule-cron --cron` expects a cron expression (seconds precision).
- `schedule-dispatch-due` executes schedules where state is `scheduled` and `run_at <= now`.
- recurring cron schedules are automatically re-planned to their next run after dispatch.
- `http` does not implement `plan-generate` yet.
- restart recovery is automatic at startup: stale `running` agents are moved back to `pending`.
- duplicate concurrent executions of the same agent are prevented by an execution lock.

### Output Modes (Shell-Friendly)

Global options available on all commands:

- `--output text|json|jsonl|tsv` (default: `text`)
- `--quiet` to suppress successful output

Recommended usage in scripts:

```bash
cargo run -- --output json spawn --name "job" --provider mock --prompt "hello"
```

```bash
cargo run -- --output jsonl list --state pending --provider sandbox
```

TSV mode is ideal for combining with Unix tools like `cut` and `awk`:

```bash
# Get all agent IDs from TSV output
cargo run -- --output tsv list | cut -f1

# Filter agents by provider and copy specific columns
cargo run -- --output tsv list --provider sandbox | awk -F'\t' '{print $1, $2, $4}'
```

Additional options for broader use-cases:

- `list --state <state> --provider <provider> --limit <n> --ids-only --sort-by <created_at|state|provider>`
- `logs --id <agent_id> --limit <n> --level <info|warn|error> --contains <text>`
- `audit-list --limit <n> --role <admin|operator|viewer> --allowed <true|false>`

Sandbox security/audit environment options:

- `AGENTD_SANDBOX_ROLE=admin|operator|viewer`
- `AGENTD_SANDBOX_AUDIT_BACKEND=file|sqlite`
- `AGENTD_SANDBOX_AUDIT_LOG_PATH=./.agentd/audit.log` (or `./.agentd/audit.db` for sqlite backend)

### CLI Provider Configuration

The `cli` provider reads environment variables:

| Variable | Default | Description |
| --- | --- | --- |
| `AGENTD_CLI_COMMAND` | `cat` | Executable used to run the agent process |
| `AGENTD_CLI_ARGS_JSON` | `[]` | JSON array of CLI arguments |
| `AGENTD_CLI_PROMPT_MODE` | `stdin` | Prompt transport mode: `stdin` or `arg` |
| `AGENTD_CLI_PROMPT_FLAG` | `--prompt` | Flag name used when mode is `arg` |
| `AGENTD_CLI_RUNTIME_DIR` | `./.agentd/runtime` | PID file directory used for cancellation |
| `AGENTD_CLI_PLAN_COMMAND` | value of `AGENTD_CLI_COMMAND` | Executable used for `plan-generate` |
| `AGENTD_CLI_PLAN_ARGS_JSON` | value of `AGENTD_CLI_ARGS_JSON` | JSON array of args used for `plan-generate` |
| `AGENTD_CLI_PLAN_GOAL_MODE` | value of `AGENTD_CLI_PROMPT_MODE` | Goal transport mode for `plan-generate`: `stdin` or `arg` |
| `AGENTD_CLI_PLAN_GOAL_FLAG` | value of `AGENTD_CLI_PROMPT_FLAG` | Flag name used when `AGENTD_CLI_PLAN_GOAL_MODE=arg` |
| `AGENTD_CLI_PLAN_OUTPUT_FORMAT` | `yaml` | Output format returned by planner CLI: `yaml` or `json` |

Example (`stdin` mode):

```bash
AGENTD_CLI_COMMAND=cat cargo run -- attach --id <AGENT_ID>
```

Example (`arg` mode):

```bash
AGENTD_CLI_COMMAND=my-agent-cli \
AGENTD_CLI_ARGS_JSON='["run"]' \
AGENTD_CLI_PROMPT_MODE=arg \
AGENTD_CLI_PROMPT_FLAG=--prompt \
cargo run -- attach --id <AGENT_ID>
```

Example (`plan-generate` with a dedicated CLI command):

```bash
AGENTD_CLI_PLAN_COMMAND=my-planner \
AGENTD_CLI_PLAN_ARGS_JSON='["plan","generate"]' \
AGENTD_CLI_PLAN_GOAL_MODE=arg \
AGENTD_CLI_PLAN_GOAL_FLAG=--goal \
AGENTD_CLI_PLAN_OUTPUT_FORMAT=yaml \
cargo run -- plan-generate --provider cli --goal "Préparer un rapport" --output ./plan.yaml
```

### HTTP Provider Configuration

The `http` provider reads environment variables:

| Variable | Default | Description |
| --- | --- | --- |
| `AGENTD_HTTP_ENDPOINT` | `http://localhost:8080/run-agent` | Target endpoint for `run_agent` |
| `AGENTD_HTTP_AUTH_MODE` | `none` | Auth mode: `none`, `bearer`, or `api-key` |
| `AGENTD_HTTP_BEARER_TOKEN` | unset | Bearer token when mode is `bearer` |
| `AGENTD_HTTP_API_KEY` | unset | API key when mode is `api-key` |
| `AGENTD_HTTP_API_KEY_HEADER` | `x-api-key` | Header name for API key mode |

Request payload sent by `http` provider:

```json
{
  "agent_id": "<AGENT_ID>",
  "prompt": "<PROMPT>",
  "timeout_secs": 60
}
```

Accepted response formats:

- JSON with `output`
- JSON with `result.output`
- JSON with `data.output`
- plain text body (fallback)

## Plan File Format

Example `plan.yaml`:

```yaml
name: demo-plan
steps:
  - id: step-1
    name: analyze
    prompt: "Analyze objective"
    provider: mock
    runtime: process
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
- `runtime` (string, optional): runtime override (`process`, `docker`, etc.)
- `depends_on` (string array, optional)
- `timeout_secs` (u64, optional)
- `retries` (u32, optional)

Runtime selection precedence is:

1. plan step `runtime`
2. CLI runtime override (for example `attach --sandbox-runtime`)
3. configuration file (`[providers.sandbox].runtime`)

## Agent State Model

Possible states:

- `pending`
- `running`
- `paused`
- `succeeded`
- `failed`
- `cancelled`
- `timed_out`

State transitions are enforced:

- terminal states (`succeeded`, `failed`, `cancelled`, `timed_out`) cannot transition to another state
- invalid transitions return an explicit runtime error

## Known Limitations

- `plan-generate` is implemented by `mock` and `cli`.
- scheduler is implemented, but there is no always-on daemon loop in this MVP.

Scheduler status:

- one-shot scheduling (`schedule-run-at`) is implemented
- due dispatch (`schedule-dispatch-due`) is implemented
- cron scheduling (`schedule-cron`) is implemented

Streaming status:

- real-time output streaming is implemented for `attach` with provider `cli`
- optional structured output is available via `--json-lines true`

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
