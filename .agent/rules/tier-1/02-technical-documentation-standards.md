# Skill 02 — Technical Documentation Standards

> Write technical documentation that is clear, structured, and directly actionable for `agentd` contributors and operators.

## Use When

Use this skill when the request contains terms such as:
- `documentation`, `README`, `guide`, `how-to`, `tutoriel`
- `explain command`, `usage`, `configuration`, `troubleshooting`

## Documentation Goals

- Make the "what", "why", and "how" explicit.
- Minimize ambiguity and hidden assumptions.
- Keep content concise while preserving operational completeness.

## Required Structure

For operational docs, follow this order:
1. Context
2. Prerequisites
3. Steps
4. Examples
5. Failure modes and troubleshooting
6. Verification

For reference docs, follow this order:
1. Purpose
2. Interface (arguments/options/inputs)
3. Behavior and outputs
4. Edge cases and limits
5. Related files/commands

## Writing Rules

- Use short sentences and stable terminology.
- Introduce acronyms only once, then keep consistent naming.
- Include copy-pastable commands and expected outcome summaries.
- Distinguish defaults from optional overrides.
- Prefer concrete examples from this repo over generic samples.

## Accuracy Rules

- Do not document behavior that is not implemented.
- Cross-check command names and options with `src/cli.rs`.
- Cross-check runtime behavior with `src/app.rs` and relevant adapters.
- Explicitly mark known limitations and work-in-progress features.

## Quality Gate

Before finalizing documentation, verify:
- a new contributor can execute the documented flow end-to-end
- commands and file paths are valid in the repository
- known errors have at least one troubleshooting note

<!-- Updated: 2026-03-23 -->
