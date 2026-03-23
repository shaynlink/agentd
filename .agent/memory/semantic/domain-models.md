<!-- domain-models.md — Retrieval-Aware Format -->
<!-- Updated: 2026-03-24 -->

## INDEX
- [Agent Entity](#agent-entity) — state machine, attempts, lifecycle, transitions
- [Schedule Entity](#schedule-entity) — run-at, cron, dispatch, recurrence
- [Plan Entity](#plan-entity) — steps, provider defaults, timeout, retries

## <section id="agent-entity"> Agent Entity

Primary execution unit persisted in SQLite.

Fields:
- `id`: stable UUID for lookup and attach/stop commands
- `name`: user-facing alias
- `provider`: selected provider key (`mock`, `cli`, `http`)
- `prompt`: execution payload for provider
- `state`: lifecycle enum
- `attempts`: retry attempt counter
- timestamps: `created_at`, `updated_at`

Lifecycle states:
- `pending` -> `running` -> terminal (`succeeded`, `failed`, `timed_out`, `cancelled`)
- `running` <-> `paused`
- terminal states are non-transitionable

</section>

## <section id="schedule-entity"> Schedule Entity

Execution trigger record for one-shot or recurring orchestration.

Fields:
- `id`, `name`, `provider`, `prompt`
- `cron_expr` nullable (null means one-shot run-at schedule)
- `run_at` next dispatch timestamp
- policy: `timeout_secs`, `retries`
- state: `scheduled`, `running`, `succeeded`, `failed`

Runtime semantics:
- `schedule-dispatch-due` selects due schedules (`run_at <= now`)
- one-shot transitions to terminal state after run
- cron schedules are re-planned to next run and return to `scheduled`

</section>

## <section id="plan-entity"> Plan Entity

Serializable execution plan consumed by `run-plan`.

Fields:
- `Plan.name`
- ordered `steps[]`
- `PlanStep`: `id`, `name`, `prompt`, optional `provider`
- optional policy fields: `timeout_secs`, `retries`
- optional dependency metadata: `depends_on`

Current execution semantics:
- parsed from YAML/JSON file
- executed sequentially by current app service
- each step spawns an agent record for persistence and observability

</section>
