# Skill 21 — Retry and Timeout Strategy

> Keep retries and timeouts explicit, bounded, and observable.

## Scope

This skill applies to:
- retry loop in `App::attach`
- provider error and timeout behavior
- timeout-aware scheduling and execution dispatch

## Strategy Rules

- Use bounded retries from explicit policy input (`retries`).
- Increment attempts before each provider invocation.
- Distinguish provider failures from timeout failures in both state and logs.
- Transition to terminal states only after retry budget is exhausted.

## State Outcomes

- provider error + retries exhausted -> `failed`
- timeout + retries exhausted -> `timed_out`
- success at any attempt -> `succeeded`

## Observability Rules

- provider failures must use category `provider_error`
- timeouts must use category `timeout`
- user-facing errors should include final attempt count

## Verification Checklist

- retries on provider errors are covered by tests
- retries on timeouts are covered by tests
- attempts counter matches policy + execution behavior
- error categories are asserted in integration tests

<!-- Updated: 2026-03-24 -->
