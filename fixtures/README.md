# Fixtures

Invented data in the exact shapes Claude Code writes (see `docs/data-contract.md`, section 1). Nothing here
comes from a real transcript. All three fixtures are projections of one timeline built by `generate.py`:

| fixture | what it is |
|---|---|
| `claude-small/` | a Claude dir at the end of the timeline: two sessions, three plain subagents, one finished workflow run with its run file |
| `claude-inflight/` | the same Claude dir 60 s earlier (`T0`): the workflow is mid-run, there is no run file, one agent has an open tool call, one agent has not started |
| `recording-small/` | a format-1 recording of `claude-inflight` turning into `claude-small`, 60 s, polled every 500 ms |
| `expected.json` | what a correct reader concludes from each, machine-readable; tests should assert against it |

Regenerate with `python3 fixtures/generate.py` (deterministic; do not edit the generated files by hand).
`T0 = 1794650000000` (2026-11-14T09:53:20Z); the timeline ends at `T0 + 60000`.

## The story

Session 1 (`11111111-1111-4111-8111-111111111111`, pid 4242, cwd `/home/dev/projects/orchard`) adds stock
adjustment endpoints. Its main thread reads a file, launches two subagents, later launches the workflow
`orchard-stock` (run `wf_0a1b2c3d-4e5`, phases Plan and Build).

Session 2 (`22222222-2222-4222-8222-222222222222`, pid 99999999, cwd `/home/dev/projects/kiln`) crashed
long ago; its session file still says `busy`.

| agentId | kind | label | parent | phase | model |
|---|---|---|---|---|---|
| `main` | main | Stock adjustment endpoints | none | | claude-opus-5-5 |
| `a3f1c09d2b7e4a611` | subagent | Survey inventory module | `main` | | claude-opus-5-5 |
| `a7c2e15f8d0b3a922` | subagent | Trace stock adjustments | `a3f1c09d2b7e4a611` (nested, depth 2) | | claude-opus-5-5 |
| `a9d4b27a6c1e5f033` | subagent | Draft changelog entry | `main` | | claude-sonnet-5-5 |
| `ab10c4d7e2f6a8101` | workflow | plan:outline | `main` | 1 Plan | claude-opus-5-5 |
| `ab20d5e8f3a7b9202` | workflow | build:api | `main` | 2 Build | claude-fable-5-2 |
| `ab30e6f9a4b8c0303` | workflow | build:ui | `main` | 2 Build | claude-fable-5-2 |
| `ab40f7a0b5c9d1404` | workflow | build:tests | `main` | 2 Build | claude-sonnet-5-5 |

## Liveness in tests

A fixture cannot contain a running process. Tests inject the pid probe: **4242 is alive, 99999999 is
stale**. 99999999 is above the pid range of macOS and Linux and is not a valid Windows pid, so the real
probe also reports it stale. In replay every session's liveness is `unknown`.

## What a correct reader concludes

### `claude-small/`

- 2 sessions. Session 1: `alive`, status `idle`, title "Stock adjustment endpoints", 8 agents, 1 run.
  Session 2: `stale`, title "Firing log cooldown rows", 1 agent (`main`, state `stopped`, reason
  `sessionGone`, 1 tool call), no runs.
- Session 1 agent states: `main` `waiting` (session idle); `a3f1…` `done`; `a7c2…` `done`; `a9d4…` `stopped`
  (reason `stoppedByUser`, with one tool call that never got a result); all four workflow agents `done`.
- Tool calls: main 4, `a3f1…` 3, `a7c2…` 2, `a9d4…` 2, plan:outline 3, build:api 6 (one Bash result has
  `is_error: true`), build:ui 6, build:tests 5.
- Run `wf_0a1b2c3d-4e5`: `final: true`, status `completed`, name `orchard-stock`, parent `main`,
  `startedAt = T0 - 120000`, `endedAt = T0 + 46000`, phases `[1 Plan: done, 2 Build: done]`, 4 agents,
  `totalToolCalls` 20, `totalTokens` 71896, `declaredAgentCount` 4.
- Tokens of workflow agents come from the run file (15495, 19231, 19228, 17942), not from the transcripts
  (15673, 19409, 19406, 18120): `expected.json` has both as `tokens` and `transcriptTokens`.
- Files touched by more than one agent:
  - `/home/dev/projects/orchard/src/stock.ts`: 6 agents (main, `a3f1…`, `a7c2…`, plan:outline, build:tests
    read it; build:api wrote it twice)
  - `/home/dev/projects/orchard/docs/stock-plan.md`: 3 (plan:outline wrote; build:api and build:ui read)
  - `/home/dev/projects/orchard/src/routes/adjust.ts`: 3 (build:api wrote; build:ui and build:tests read)
- Bash detection: `a3f1…` writes `/tmp/orchard-adjust.txt` (a redirection); build:tests reads
  `/home/dev/projects/orchard/package.json` (`cat package.json`, resolved against `cwd`). `npm test` and
  `npm run build` produce no touches.
- Tasks: workflow agents have framed prompts; e.g. plan:outline's task is "Outline the stock adjustment
  work for stock." (the harness frame is stripped and the text is cut at the first blank line).
- Tolerance checks built in: the main transcript has a `future-record-type` record and records without
  timestamps; each assistant message with thinking is two records sharing a `message.id`; metas carry a
  `pickedEffortModel` that is not the agent's model.

### `claude-inflight/`

Same as above except:

- Session 1 status `busy`, so `main` is `running`; 7 agents (build:tests does not exist yet).
- No run file: the run is reconstructed. `final: false`, status `running`, `endedAt: null`, totals `null`,
  phases `[1 Plan: done, 2 Build: running]` parsed from the inline `script` of the Workflow call (the same
  text is in `workflows/scripts/`), 3 agents.
- build:ui is `running` with 3 tool calls, the last one a Bash call without a result. plan:outline and
  build:api are `done` because the journal has their `result` lines.
- Tokens are transcript-derived for everybody (`transcriptTokens` in `expected.json`).
- `src/routes/adjust.ts` has only one toucher so far; `src/stock.ts` has 5.

### `recording-small/`

- Timeline `[T0, T0 + 60000]`, 46 fs-events, the first 17 are the initial scan.
- At the start position the state equals `claude-inflight`; at the end it equals `claude-small`. In both
  cases liveness is `unknown`, and session 2 has only its session file (the recorder watches one project):
  `hasTranscript: false`, title `null`, a `main` agent with no tool calls whose state is `running`.
- Milestones (fs-event time, up to 500 ms after the write): build:ui done at +15 s; build:tests appears at
  +16 s and is done at +45 s; the run file appears at +46 s (`runFinalized`); session 1 turns `idle` at
  +50 s.
- At +20 s: 4 workflow agents known, build:tests `running`, the run not final.
- fs-event 34 (+30 s) records a size that ends in the middle of a line of build:tests' transcript; the
  line completes with event 35. A reader must not fail or count that line twice.
- Seeking to any time and playing from the start to that time must give the same snapshot.
