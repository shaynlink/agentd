<!-- sqlite-schema.md — Retrieval-Aware Format -->
<!-- Updated: 2026-03-24 -->

## INDEX
- [Agents and Logs](#agents-and-logs) — primary execution persistence
- [Schedules and Runs](#schedules-and-runs) — dispatch history and recurrence
- [Execution Locks](#execution-locks) — idempotence and anti-duplication

## <section id="agents-and-logs"> Agents and Logs

Core tables managed by SQLite adapter:
- `agents`
  - stores record identity, provider, prompt, state, attempts, timestamps
- `logs`
  - append-only event log by `agent_id`
  - fields include timestamp, level, message

Current logging payload convention:
- message content is structured JSON where possible
- includes `context`, `provider`, `category`, `message`

</section>

## <section id="schedules-and-runs"> Schedules and Runs

Scheduler tables:
- `schedules`
  - one-shot and cron descriptors
  - next run timestamp and state tracking
- `schedule_runs`
  - execution history per schedule occurrence
  - status + optional agent_id + optional error text

Dispatch semantics:
- due schedules selected by timestamp
- one-shot schedules complete to terminal
- cron schedules re-plan `run_at` and return to `scheduled`

</section>

## <section id="execution-locks"> Execution Locks

Concurrency safety table:
- `execution_locks`
  - one lock per `agent_id`
  - stores lock owner metadata (pid-based owner currently used)

Guarantees:
- prevents duplicate concurrent attach execution for same agent
- lock released on terminal execution outcomes
- startup recovery clears stale in-progress execution state

</section>
