# Virgil data contract

Version 1. This document is the source of truth for three things:

- what Virgil reads from the Claude config dir (sections 1 and 4),
- the model, events and commands that cross the Rust / UI boundary (sections 2, 3, 5, 7),
- how a recording is replayed (section 6).

The Rust core owns sections 1, 4 and 6. The UI only ever sees sections 2, 3, 5 and 7.
If code has to deviate, change this file in the same piece of work.

Conventions used throughout:

- **Times** are epoch milliseconds (`EpochMs`, a JSON number) everywhere past the adapter. Source files use
  both epoch milliseconds and ISO-8601 strings; the adapter converts.
- **JSON** across the boundary is camelCase. Optional values are present and `null`, never omitted, unless a
  type says otherwise. Unknown fields must be ignored by both sides.
- **Verified** means checked on this machine against Claude Code 2.1.319 to 2.1.323 files (673 agent
  metas, 14 workflow journals, 13 run files, 26 projects). **Inferred** means a reasonable guess that was not
  observed. The formats are undocumented and will change: all parsing lives in one adapter module
  (`crates/core/src/adapter/`), every field is optional to the parser unless this document says "required",
  and an unknown record type or an unparseable line is skipped, never an error.

---

## 1. Source files as observed

All paths are relative to the Claude dir: `$CLAUDE_CONFIG_DIR` if set, else `~/.claude`
(`%USERPROFILE%\.claude` on Windows). Virgil opens these files read-only and never writes there.

```
sessions/<pid>.json                                              one per running session
projects/<slug>/<sessionId>.jsonl                                main transcript
projects/<slug>/<sessionId>/subagents/agent-<agentId>.jsonl      subagent transcript
projects/<slug>/<sessionId>/subagents/agent-<agentId>.meta.json  subagent meta
projects/<slug>/<sessionId>/subagents/workflows/<runId>/agent-<agentId>.jsonl      workflow agent transcript
projects/<slug>/<sessionId>/subagents/workflows/<runId>/agent-<agentId>.meta.json  workflow agent meta
projects/<slug>/<sessionId>/subagents/workflows/<runId>/journal.jsonl              workflow journal (live!)
projects/<slug>/<sessionId>/workflows/scripts/<workflowName>-<runId>.js            copy of the script
projects/<slug>/<sessionId>/workflows/<runId>.json               run file, written once at the end
projects/<slug>/<sessionId>/tool-results/*                       ignored
sessions/<pid>.<hash>.key                                        ignored, never read
```

Identifier shapes (verified): `sessionId` is a UUID. `runId` is `wf_` + 8 hex + `-` + 3 hex
(`wf_0a1b2c3d-4e5`). `agentId` is usually `a` + 16 hex (`ab10c4d7e2f6a8101`), but named agents exist
(`afix-the-b-0c1d2e3f4a5b6c7d`), so **take the id from the file name**: everything between `agent-` and
`.jsonl` / `.meta.json`. Tool use ids are `toolu_` + 24 base58 characters.

`<slug>` is the session's `cwd` with every character outside `[A-Za-z0-9]` replaced by `-` (verified for
plain paths, inferred for dots, spaces and Windows drive letters). Do not compute it: find the transcript
with the glob `projects/*/<sessionId>.jsonl`.

### 1.1 `sessions/<pid>.json` (verified, 10 files)

Rewritten in place whenever status changes. One JSON object.

| field | type | notes |
|---|---|---|
| `pid` | number | required; equals the file name |
| `sessionId` | string | required |
| `cwd` | string | absolute path |
| `name` | string | short derived name, e.g. the directory name |
| `status` | string | `busy`, `shell`, `idle` and `waiting` observed (`shell`: a shell command is running; `waiting`: the session asks the person something); treat as an open set |
| `kind` | string | `interactive` observed |
| `startedAt`, `updatedAt`, `statusUpdatedAt`, `nameSince` | number (ms) | |
| `version` | string | Claude Code version |
| `procStart` | string | local time, `Thu Nov 12 02:19:28 2026`; not parsed |
| `entrypoint`, `pidDomain`, `nameSource`, `peerProtocol`, `peerFeatures`, `messagingSocketPath`, `asksForConvertedSkills` | | ignored |

Surprises:

- The file survives a crash, so `status: "busy"` can be a lie. The core checks the pid (section 2.2).
- Sessions that ended cleanly have no file. Live mode therefore shows only sessions that still have a pid
  file; older sessions are out of scope for version 1.
- Live mode hides a `stale` session whose newest of `updatedAt`, `statusUpdatedAt` and the pid file's mtime is
  older than the recent window (default 24 hours, configurable), so startup stays fast on a large history. An
  `alive` session is always shown. Replay shows every session.
- Two pid files with the same `sessionId` were not observed but are possible after a resume (inferred): the
  one with the greater `updatedAt` wins.

### 1.2 Transcripts (`*.jsonl`, main and agent alike; verified)

Append-only, one JSON object per line, UTF-8. A reader keeps a byte offset per file and parses only up to the
last `\n`; a trailing partial line is left for the next read. Lines can be megabytes long (`api-request-blob`
records), and a main transcript reached 163 MB, so parse incrementally and never hold whole files: the
engine reads at most 4 MB at a time and parses that slice's complete lines before reading the next, and in
live mode a large file yields part-way once the tick's 50 ms budget is used up and is read on from there on
the next tick. If a file
becomes shorter than the stored offset, drop everything derived from it and read it again from zero
(inferred; not observed).

Record types seen in the main transcript: `user`, `assistant`, `attachment`, `system`, `ai-title`,
`queue-operation`, `api-request`, `api-request-blob`, `api-request-shape`, `last-prompt`, `mode`,
`permission-mode`, `session-owner`, `atis-latch`, `frame-link`, `file-history-snapshot`,
`file-history-delta`, `tools-used`, `cost-state`, and several `artifact-*`. In agent transcripts: `user`,
`assistant`, `attachment`, `api-request*`, `fork-context-ref`. **The app reads only `user`, `assistant`,
`ai-title` and `queue-operation`**; every other type is skipped. Many types have no `timestamp` and no `uuid`.

Fields read from `user` and `assistant` records:

| field | type | notes |
|---|---|---|
| `type` | string | required |
| `timestamp` | string | ISO-8601 UTC with milliseconds, `2026-11-13T09:29:27.993Z` |
| `uuid`, `parentUuid` | string, string or null | not needed by the model; `parentUuid` is null on an agent's first record |
| `isSidechain` | bool | false in the main transcript, true in agent transcripts |
| `agentId` | string | only in agent transcripts; equals the file name id |
| `sessionId`, `cwd` | string | `cwd` is what relative paths in tool inputs resolve against |
| `message.role` | string | |
| `message.content` | string or array of blocks | see below |
| `message.id` | string | assistant only; the API message id |
| `message.model` | string | assistant only; the resolved model id, e.g. `claude-opus-5-5` |
| `message.stop_reason` | string or null | `tool_use`, `end_turn`, `stop_sequence` observed |
| `message.usage` | object | `input_tokens`, `cache_creation_input_tokens`, `cache_read_input_tokens`, `output_tokens` (numbers) |
| `toolUseResult` | object, string or array | user records that carry a tool result; shape depends on the tool |
| `isApiErrorMessage` | bool | assistant records that are an error notice, not model output |

Content blocks: `text {text}`, `thinking {thinking, signature}`, `tool_use {id, name, input, caller}` in
assistant records; `tool_result {tool_use_id, content, is_error?}` and `text` in user records.
`tool_result.content` is a string or an array of blocks. `is_error` is often absent, which means false.

Surprises:

- **One assistant record per content block.** A message with a thinking block and a tool call is two
  records with the same `message.id` and the same `usage`. Summing `usage` over records double counts:
  group by `message.id` (verified on 496 records, none had two blocks).
- **Tool calls and their results are matched by id**, `tool_use.id` to `tool_result.tool_use_id`, within the
  same file. A stopped agent leaves tool calls without a result.
- A subagent's first record is a `user` record whose content is the prompt string. Forked agents
  (`agentType: "fork"`) start with a `fork-context-ref` record instead, followed later by their `user` prompt.
- **Workflow agents get a framed prompt** in two `user` string records. The first starts with
  `[Workflow harness — user request]`. The second starts with `[Workflow harness — computed task]`, and the
  real task follows its first line break, with every line indented by two spaces. A third `user` string
  record starting with `<system-reminder>` may follow. See the task rule in section 2.4.
- Agent transcripts are created with content already in them: the first time a file is seen it can be
  hundreds of kilobytes.

**Launches in a transcript** (main or agent; verified):

- `tool_use` with `name: "Agent"` and input `{description, subagent_type, prompt, model?, isolation?}`. The
  matching user record has `toolUseResult {status: "async_launched", agentId, description, resolvedModel,
  isAsync, ...}` for background agents. Foreground agents return their final text instead, with no
  `agentId` (inferred from `requestShape: "foreground"` metas; not sampled).
- `tool_use` with `name: "Workflow"` and input **either** `{script, args?}` (inline source, 9 of 14) **or**
  `{scriptPath, args?}` (a path in the user's project, 5 of 14). The matching user record follows within a
  second and has `toolUseResult {status: "async_launched", taskType: "local_workflow", taskId, runId,
  workflowName, summary, transcriptDir, scriptPath}`. Here `scriptPath` is always the copy inside the Claude
  dir (`.../workflows/scripts/<workflowName>-<runId>.js`), and `taskId` starts with `w`.

**Completion notices in a transcript** (verified): `queue-operation` records with `operation: "enqueue"`,
a `timestamp`, and `content` holding a pseudo-XML string:

```
<task-notification>
<task-id>a3f1c09d2b7e4a611</task-id>        agentId, or the workflow taskId (w...), or a shell task (b...)
<tool-use-id>toolu_...</tool-use-id>        the launching tool_use; absent in about a fifth
<status>completed</status>                  completed | failed | killed | stopped; absent on progress notices
<summary>...</summary> <result>...</result>
<usage><subagent_tokens>21077</subagent_tokens><tool_uses>3</tool_uses><duration_ms>79900</duration_ms></usage>
</task-notification>
```

Workflow notices add `<agent_count>`, `<agents_done>`, `<agents_error>`, `<agents_skipped>` inside `<usage>`.
The same text appears again in `operation: "remove"` records and inside `attachment` records; read only
`enqueue`. Extract tags with a tolerant regular expression, not an XML parser. Notices for nested agents
arrive in the parent agent's transcript as a `user` record with string content starting with
`<task-notification>` (inferred from the shape). The core reads both forms from every transcript of the
session and keeps the newest notice per `task-id`, session-wide; such a `user` record is never taken as a
task.

`ai-title` records are `{type, aiTitle, sessionId}`; the last one wins as the session title.

### 1.3 `agent-<id>.meta.json` (verified, 482 plain and 191 workflow metas)

Written when the agent starts, rewritten when it ends. One JSON object.

| field | plain subagent | workflow agent | notes |
|---|---|---|---|
| `agentType` | always | always `workflow-subagent` | `general-purpose`, `fork`, `Explore`, `Plan`, custom names |
| `description` | always | always | plain: the Agent call's `description`. workflow: the agent's label, e.g. `plan:outline` |
| `toolUseId` | 481 of 482 | **never** | the `tool_use.id` of the launching Agent call |
| `parentAgentId` | only when nested (18) | never | the launching agent; absent means the main thread |
| `spawnDepth` | always (1 or 2) | always 1 | |
| `workflowPhase` | never | always | the phase **title**, not an index |
| `model` | 213 of 482 | 189 of 191 | the requested **alias**: `opus`, `sonnet`, `fable`, `inherit` |
| `pickedEffortModel` | 325 of 482 | always | **not the agent's model**, see below |
| `finished` | `true` when ended, else absent | **never** | |
| `stoppedByUser` | `true` when stopped (4), without `finished` | never | |
| `name` | 1 of 482 | never | set for named agents |
| `worktreePath`, `worktreeBranch` | 37 of 482 | never | agent ran in a git worktree |
| `revivedAt` | 2 of 482 (ms) | never | the agent was resumed with SendMessage after finishing |
| `isBuiltIn`, `isFork`, `requestShape`, `requestNonInteractive`, `launchedWithDefaults`, `spawnMode`, `spawnedWithWorktree`, `worktreeCleanlyRemoved` | | | ignored |

Surprises:

- **Workflow agent metas carry no link and no end state**: no `toolUseId`, no `parentAgentId`, no
  `finished`. The link to the run is the directory name; the end state comes from the journal (1.4) and
  later the run file (1.5).
- **`pickedEffortModel` is misleading.** A workflow agent with `model: "sonnet"` had
  `pickedEffortModel: "claude-fable-5-2"` while its transcript and the run file both said
  `claude-sonnet-5-5`. Never show it. The model order is in section 2.4.
- Nested agents live flat in the same `subagents/` directory as their parent; `toolUseId` then points into
  the **parent agent's** transcript, not the main one (verified on 4).
- `finished` is absent on 25 of 482 plain metas: still running, stopped, or the session died.

### 1.4 `journal.jsonl` (verified, 14 journals; not in the original brief)

Appended live by the workflow runner, in the run's `subagents/workflows/<runId>/` directory. **It has no
timestamps.** This file, not the transcripts, says when a workflow agent ends.

| line | fields | meaning |
|---|---|---|
| `{"type":"launched"}` | | first line, once |
| `{"type":"started", key, agentId, label, phase}` | all strings | an agent attempt started; `phase` is the phase title; `key` is `v2:` + 64 hex and identifies the script's call site |
| `{"type":"result", key, agentId, result, toolsUsed, ...}` | `result` is a string or an object | that attempt finished successfully |
| `{"type":"failed", key, agentId}` | `agentId` can be `""` | that attempt failed; an empty id means it never started |

Surprises:

- **Retries.** When an attempt is abandoned (for example a usage limit), a second `started` line appears with
  the same `key` and `label` and a new `agentId`. The first attempt gets no `result` and no `failed` line,
  and the run file later lists only the last attempt. So a run directory can hold more agent files than the
  run file's `agentCount` (46 files against 45 in one run).
- `started` lines appear in the order the transcripts start (verified on 46).

### 1.5 `workflows/<runId>.json` (verified, 13 files)

Written once, when the run ends; its mtime is within 50 ms of `startTime + durationMs`. Until then the file
does not exist. A run whose session died has a journal and no run file (1 of 14).

| field | type | notes |
|---|---|---|
| `runId`, `taskId`, `workflowName`, `summary` | string | |
| `status` | string | only `completed` observed, even for a run whose single agent errored |
| `startTime` | number (ms) | |
| `durationMs` | number | |
| `timestamp` | string (ISO) | when the file was written |
| `phases` | array of `{title, detail?, model?}` | as declared in the script's `meta.phases` |
| `agentCount`, `totalTokens`, `totalToolCalls` | number | the totals equal the sums over `workflowProgress` |
| `defaultModel` | string | |
| `workflowProgress` | array | entries below |
| `script`, `scriptPath`, `args`, `logs`, `result` | | `args` absent when none; `result` is an array or an object; not read except `logs` length |

`workflowProgress` entries, all phases first, then agents:

- `{type: "workflow_phase", index, title}`. **`index` is 1-based.**
- `{type: "workflow_agent", index, label, phaseIndex, phaseTitle, agentId, model, state, queuedAt,
  startedAt, attempt, tokens, toolCalls, durationMs, lastToolName, lastToolSummary, promptPreview,
  promptFramed, resultPreview, lastProgressAt, lastAttemptReason?, error?}`.

Notes on `workflow_agent` (verified unless marked):

- `state` is `done` (181) or `error` (2). Other values such as `queued`, `running`, `skipped` are inferred.
- `agentId`, `startedAt`, `attempt`, `tokens`, `toolCalls`, `durationMs`, `lastToolName` are **absent** when
  the agent never started (1 of 183: it has `state: "error"`, `error`, `queuedAt`, `lastProgressAt` only).
  `lastToolSummary` is absent in a quarter of entries; `resultPreview` is absent on errors.
- `model` is the resolved model id. `phaseIndex` matches a `workflow_phase.index`; agents are **not** grouped
  by phase in the array (a later Fix round interleaves phases).
- `startedAt` equals the first transcript timestamp or is up to 2.3 s earlier. `lastProgressAt` is within
  5 ms of the last transcript timestamp. `durationMs` equals `lastProgressAt - startedAt`.
- `toolCalls` equals the number of `tool_use` blocks in the transcript (verified on 12 agents).
- `tokens` is the context size at the agent's last request: within 10 of `input_tokens +
  cache_creation_input_tokens + cache_read_input_tokens` of the last message. It is not a sum over the run.
- On a retry, `label` gets a suffix (`review:x:1 (after usage limit)`), `attempt` is 2 and
  `lastAttemptReason` is set (`throttled`).

### 1.6 Workflow script (`workflows/scripts/<workflowName>-<runId>.js`, verified on 9)

JavaScript, not JSON. It starts with an object literal that holds the declared phases:

```js
export const meta = {
  name: 'orchard-stock',
  description: '...',
  phases: [
    { title: 'Plan', detail: 'one outline for everybody', model: 'opus' },
    { title: 'Build' },
  ],
}
```

The same text is the `script` input of the Workflow call when the script was passed inline. The extraction
rule is in section 4, step 2.

---

## 2. Domain model

### 2.1 Entities and ids

```
Snapshot
└── Session            id: sessionId
    ├── Agent          id: agentId, unique within its session. The main thread is the agent "main".
    │   └── ToolCall   id: toolUseId
    ├── WorkflowRun    id: runId
    │   └── Phase      id: index (1-based, as in the run file)
    └── FileEntry      id: normalised absolute path
        └── FileTouch  id: agentId
```

Links:

- `Agent.parentAgentId`: who launched it. `null` only for `main`. For a plain subagent it is the meta's
  `parentAgentId`, or `"main"`. For a workflow agent it is the run's `parentAgentId`.
- `Agent.runId` and `Agent.phaseIndex`: set for workflow agents only.
- `WorkflowRun.parentAgentId`: the agent whose transcript holds the Workflow call (`"main"` in every
  observed run). `WorkflowRun.agentIds` lists its agents in first-seen order.
- The **parent chain** of an agent is `parentAgentId` followed until `null`. The **graph** is: session →
  main; agent → child agents with `runId == null`; agent → runs it launched; run → phases → agents.
- The UI must tolerate a `parentAgentId` that is not in the snapshot yet and draw that agent under `main`
  until the parent arrives.

The main thread is modelled as an agent so that its tool calls, tokens and file touches need no special
case. Its `agentId` is the literal string `"main"`, its `kind` is `"main"`. It is created together with its
session (`sessionUpserted`, then `agentSpawned`), even when no transcript is found.

### 2.2 Session

| field | source |
|---|---|
| `sessionId`, `pid`, `cwd`, `name`, `status`, `kind`, `version`, `startedAt`, `updatedAt`, `statusUpdatedAt` | `sessions/<pid>.json` |
| `title` | last `ai-title` record of the main transcript, else `null` |
| `projectSlug` | directory holding the main transcript, else `null` |
| `hasTranscript` | whether `projects/*/<sessionId>.jsonl` was found |
| `liveness` | `alive`, `stale` or `unknown`, below |

Liveness (live mode): `alive` if a process with that pid exists, else `stale`. Use the `sysinfo` crate or
`kill(pid, 0)` on Unix and `OpenProcess` on Windows, behind a `PidProbe` trait so tests can inject the
answer. Re-check every 5 s. A pid above the platform's maximum is `stale`. In replay mode every session is
`unknown`, because the recorded machine's processes cannot be checked; `unknown` behaves like `alive` in
every rule of this document.

### 2.3 Agent state

Exactly one of `queued`, `running`, `waiting`, `done`, `failed`, `stopped`. `stateReason` is a short
machine string or `null`. Evaluate the rules top to bottom; the first match wins.

**Main agent**

1. session `liveness == "stale"` → `stopped`, reason `sessionGone`
2. session `status` is `busy` or `shell`, or missing → `running`
3. any other status (`idle`, `waiting`, unknown values) → `waiting`

**Plain subagent**

1. meta `stoppedByUser == true` → `stopped`, reason `stoppedByUser`
2. a completion notice for this `agentId` exists in the parent's transcript, and the agent's transcript has
   no assistant record more than 1 s newer than it: `completed` → `done`; `failed` → `failed`, reason
   `failed`; `killed` or `stopped` → `stopped`, reason is the status word. Exception: a `completed`
   notice whose text says the agent "is waiting on its own background work" (or "background work of its
   own still running") means it paused and will resume → `waiting`, reason `backgroundWork`, unless the
   session is stale or the quiet rule applies. Observed on 2.1.322: such an agent wakes on each of its
   background tasks, works, and stops again with a new notice.
3. meta `finished == true` → `done`, unless a notice exists and the agent has an assistant record more
   than 1 s newer than it: then it resumed (the meta file keeps `finished: true` from the earlier stop),
   so skip this rule
4. session `liveness == "stale"` → `stopped`, reason `sessionGone`
5. the quiet rule → `stopped`, reason `noActivity`
6. otherwise the activity rule

**Workflow agent**

1. the run is final (run file read): from the run file's `state`: `done` → `done`; `error` → `failed`,
   reason is the `error` text cut to 200 characters; `queued` → `queued`; `running` → `running`; anything
   else → `stopped`, reason is the raw state. An agent that the run file does not list → `failed`, reason
   `retried` if rule 4 applies, else `stopped`, reason `notInRunFile`.
2. journal has `result` for this `agentId` → `done`
3. journal has `failed` for this `agentId` → `failed`, reason `failed`
4. journal has a later `started` with the same `key` and another `agentId` → `failed`, reason `retried`,
   and `superseded = true`
5. the run's status is not `running` → `stopped`, reason `runEnded`
6. session `liveness == "stale"` → `stopped`, reason `sessionGone`
7. the quiet rule → `stopped`, reason `noActivity`
8. otherwise the activity rule

**Quiet rule.** The newest `user` or `assistant` timestamp of the agent and of all its descendants (by
`parentAgentId`) is more than 30 minutes before the source's `now` (wall clock in live, the replay
position in replay). Plain subagents that were killed without a completion notice or `finished` flag
(observed: a subagent whose transcript ended a week earlier while its session was still alive) would
otherwise stay `running` for ever. A child that is still writing keeps its parent alive. The rule is
re-evaluated on every tick, so an agent turns `stopped` without any file changing, and back to `running`
if its transcript grows again.

**Activity rule** (non-terminal agents): `waiting` if the newest assistant record has a `stop_reason` other
than `tool_use` (the agent ended a turn and is waiting for children or messages), or if every tool call
without a result is an `Agent` or `Workflow` call. Otherwise `running`.

`queued` is never produced by reconstruction, because an agent's files appear only when it starts. It
exists for run files and for forward compatibility.

A terminal agent whose transcript grows again (SendMessage revival) is re-evaluated from rule 1; a state can
therefore go from `done` back to `running`.

### 2.4 Other derived agent values

| field | rule |
|---|---|
| `kind` | `main`, `subagent` (file directly in `subagents/`) or `workflow` (file in `subagents/workflows/<runId>/`) |
| `label` | short name. main: session `title`, else session `name`, else `"main"`. Others: meta `description`, else journal `label`, else the `agentId`. After finalisation: the run file's `label` |
| `task` | the high-level task, from the prompt, rule below. main: text of the first `user` record with string content that does not start with `<`. Empty string when unknown |
| `agentType`, `name`, `worktreePath`, `spawnDepth` | meta, else `null` (`spawnDepth`: 0 for main, else 1) |
| `spawnToolUseId` | meta `toolUseId`; for workflow agents the run's `toolUseId`; else `null` |
| `model` | first assistant record's `message.model` that does not start with `<` (error notices carry a placeholder such as `<synthetic>`; inferred); else the run file's `model`; else the meta `model` alias unless it is `inherit`; else `null`. Never `pickedEffortModel` |
| `tokens` | context size now: for the newest `message.id`, `input_tokens + cache_creation_input_tokens + cache_read_input_tokens + output_tokens`. 0 before the first assistant record. Replaced by the run file's `tokens` at finalisation (a few percent lower; it omits the last output) |
| `outputTokens` | sum of `output_tokens` over distinct `message.id` |
| `toolCalls` | number of distinct `tool_use` ids. Replaced by the run file's `toolCalls` at finalisation |
| `lastTool` | `{name, summary, at}` of the newest tool call, `null` before the first. At finalisation `name` and `summary` come from the run file when present |
| `recentTools` | the newest 8 tool calls, oldest first. The full list (up to 2000 per agent) comes from `get_tool_history` (5.1) |
| `startedAt` | first `user` or `assistant` record that has a timestamp (other record types are skipped unparsed); replaced by the run file's `startedAt` |
| `lastActivityAt` | newest `user` or `assistant` timestamp |
| `endedAt` | `null` while `queued`, `running` or `waiting`. While terminal: `lastActivityAt` (the time it was observed when there is none yet). At finalisation: run file `startedAt + durationMs` when both exist |
| `queuedAt`, `attempt`, `resultPreview` | run file only, else `null` |

Duration is not stored: the UI computes `(endedAt ?? now) - startedAt`, where `now` is the wall clock in
live mode and `SourceStatus.replay.position` in replay.

**Task rule.** Take the agent's first `user` record with string content (skip `fork-context-ref`). If it
starts with `[Workflow harness`, instead take the first `user` string record that starts with
`[Workflow harness — computed task]`, drop everything up to and including its first line break, and remove
two leading spaces from each line. Then: trim, cut at the first blank line, collapse whitespace runs to one
space, and cut to 280 characters, ending with `…` when cut. Agent prompts are private content: the task
text lives in memory only and is never logged or written to disk.

### 2.5 ToolCall

`{toolUseId, name, summary, startedAt, endedAt, isError}`. `startedAt` is the assistant record's timestamp,
`endedAt` the matching `tool_result` record's timestamp or `null`, `isError` is `null` until the result
arrives, then `is_error === true`.

`summary` is at most 120 characters, cut with `…`, from the tool input, with whitespace runs collapsed to one
space:

| tool | summary |
|---|---|
| `Bash` | `description` if present, else the first line of `command` |
| `Read`, `Edit`, `Write`, `MultiEdit`, `NotebookEdit` | the path, relative to the record's `cwd` when inside it |
| `Agent` (and the older name `Task`) | `description` |
| `Workflow` | the workflow name when known, else `"workflow"` |
| `Grep`, `Glob` | `pattern` |
| `WebFetch` | `url`; `WebSearch`: `query`; `Skill`: `skill`; `SendMessage`: `summary` |
| anything else | empty string |

### 2.6 WorkflowRun and Phase

| field | before finalisation | after finalisation |
|---|---|---|
| `name` | tool result `workflowName`, else the script's `meta.name`, else the `runId` | run file `workflowName` |
| `summary` | tool result `summary`, else `null` | run file `summary` |
| `status` | `running`; or from the workflow's completion notice (`completed`; `failed`; `killed`/`stopped` → `stopped`); or `stopped` when the session is `stale` | run file `status`: `completed` → `completed`; `failed`, `error` → `failed`; `stopped`, `killed` → `stopped`; anything else (or missing) → `stopped` with `error` set to the raw status (`"unknown"` when missing) |
| `final` | `false` | `true` |
| `parentAgentId`, `toolUseId`, `taskId` | from the launch records; `parentAgentId` defaults to `"main"` | `taskId` from the run file |
| `startedAt` | timestamp of the launch result record, else the earliest agent `startedAt`, else the time the run was first observed | run file `startTime` |
| `endedAt` | `null`, or the notice's timestamp | `startTime + durationMs` |
| `phases` | section 4 | run file `phases`, with `index` from `workflow_phase`; a `phaseIndex` used by an agent entry but missing there is added with the entry's `phaseTitle` |
| `agentIds` | agents seen, first-seen order | the same, plus synthetic ids (section 4, step 9) |
| `totalTokens`, `totalToolCalls`, `declaredAgentCount` | `null` (the UI sums over `agentIds`) | run file `totalTokens`, `totalToolCalls`, `agentCount` |
| `defaultModel`, `error` | `null` | run file `defaultModel`; `error` stays `null` unless the status was unknown, then the raw status |

Phase: `{index, title, detail, model, state}`. `state`, evaluated over the phase's agents that are not
`superseded`:

- no agents → `pending`; except that when the run is terminal it is `done`
- any agent `queued`, `running` or `waiting` → `running`
- any agent `failed` → `failed`
- otherwise → `done`

While the run is `running`, `done` is provisional: a script can add agents to an earlier phase.

### 2.7 FileEntry and FileTouch

A file touch is one agent reading or writing one path through one tool call. They are aggregated per
session: `Session.files[path].touches[agentId] = {agentId, reads, writes, firstAt, lastAt, viaBash}`.
"Which agents touched the same file" is `Object.keys(files[path].touches)`; a write conflict candidate is a
path where two or more agents have `writes > 0`.

Paths are normalised without touching the file system: relative paths are joined to the record's `cwd`;
`\` becomes `/`; `.` and `..` segments are resolved lexically; no symlink resolution; case is kept.

From file tools (`source: "tool"`):

| tool | field | op |
|---|---|---|
| `Read` | `file_path` | read |
| `Edit`, `Write`, `MultiEdit` | `file_path` | write |
| `NotebookEdit` | `notebook_path` | write |

From `Bash` (`source: "bash"`, sets `viaBash`). This is deliberately conservative. Split `command` into
words with shell quoting rules, without expansion; stop at the first `<<` (heredoc). Split into simple
commands at `&&`, `||`, `;`, `|` and line breaks. In each simple command:

1. A redirection `>`, `>>`, `1>`, `2>`, `&>`, `&>>` (attached or followed by a word) writes its target, unless
   the target starts with `/dev/` or `&`. An input redirection `<` consumes its target and touches nothing.
2. Skip leading `VAR=value` words and `sudo`. Then by command name (its last `/` segment, so `/bin/cat` is
   `cat`):
   `cat head tail less more wc stat file source .` read every path argument;
   `touch rm tee` write every path argument; `mv` writes every path argument;
   `cp` with at least two path arguments reads all but the last and writes the last;
   `sed` with `-i` (any form starting `-i`) writes its last path argument.
3. A word is a **path argument** only if it does not start with `-` or `~`, contains none of `* ? [ $` or a
   backtick, does not end with `/`, and either contains `/` or ends with an extension matching
   `\.[A-Za-z0-9]{1,8}$`.
4. At most 8 touches per command; further ones are dropped.

Everything else in a Bash command is ignored. `git`, `npm`, test runners and scripts change files that this
cannot see; the UI should present Bash touches as a hint, not as a complete list.

Known limit: an agent in a git worktree (`worktreePath`) touches paths under that worktree, so two agents
editing the "same" file in different worktrees have different paths. The UI may strip `worktreePath` to
compare; the core does not.

---

## 3. Event stream

### 3.1 Envelope and ordering

Every event is a JSON object with:

- `type`: the event name, camelCase.
- `seq`: integer, starts at 1, increases by exactly 1 per event for the whole life of the app process. It
  does not restart on a mode switch or a seek.
- `t`: `EpochMs`, the moment the event refers to: the transcript record's timestamp when the event comes
  from a record, otherwise the moment the change was observed (the wall clock in live mode, the fs-event's
  `t` in replay). `sessionUpserted`, `agentUpdated` and `runUpdated` produced by recomputing a batch of
  changes carry the `t` of the last change in that batch. `t` is **not** monotonic across agents; `seq` is
  the only order.

Guarantees:

1. **Reducer rule.** `apply(snapshot, event)` (section 3.3) gives the next snapshot. A snapshot with
   `seq == n` followed by events `n+1 .. m` equals the snapshot the core holds at `seq == m`. The core keeps
   its own snapshot with the same reducer, so `get_snapshot` and the stream cannot disagree.
2. An event that names a session, agent or run is always preceded by the event that created it
   (`sessionUpserted`, `agentSpawned`, `runStarted`), except that `parentAgentId` may point to an agent
   that appears later.
3. `snapshotReset` replaces everything; events before it are irrelevant.

Events reach the UI in batches (section 5.2). There is no per-event acknowledgement.

### 3.2 Event types

| type | payload | when |
|---|---|---|
| `snapshotReset` | `{snapshot}` | after a mode switch, opening a recording, or a seek |
| `sessionUpserted` | `{session: SessionInfo}` | a session file appeared or changed; liveness or title changed |
| `sessionRemoved` | `{sessionId}` | the session file was deleted |
| `agentSpawned` | `{sessionId, agent}` | first sight of an agent (including `main`) |
| `agentUpdated` | `{sessionId, agent}` | state, label, task, model, tokens or any other field changed, other than through a tool event |
| `toolCallStarted` | `{sessionId, agentId, call, tokens, outputTokens}` | a `tool_use` block was read |
| `toolCallFinished` | `{sessionId, agentId, toolUseId, isError}` | its `tool_result` was read |
| `fileTouched` | `{sessionId, agentId, path, op, source, toolUseId}` | a tool call read or wrote a path (section 2.7) |
| `runStarted` | `{sessionId, run}` | first sight of a workflow run |
| `runUpdated` | `{sessionId, run}` | name, status, phases or `agentIds` changed |
| `runFinalized` | `{sessionId, run, agents}` | the run file was read (section 4, step 9) |

Examples:

```json
{"type":"agentSpawned","seq":41,"t":1794650016000,"sessionId":"1111…","agent":{"agentId":"ab40f7a0b5c9d1404","sessionId":"1111…","kind":"workflow","parentAgentId":"main","spawnToolUseId":"toolu_01…","spawnDepth":1,"agentType":"workflow-subagent","name":null,"label":"build:tests","task":"Cover the endpoint with tests.","model":null,"state":"running","stateReason":null,"runId":"wf_0a1b2c3d-4e5","phaseIndex":2,"phaseTitle":"Build","attempt":null,"superseded":false,"queuedAt":null,"startedAt":1794650016000,"endedAt":null,"lastActivityAt":1794650016040,"tokens":0,"outputTokens":0,"toolCalls":0,"lastTool":null,"recentTools":[],"resultPreview":null,"worktreePath":null}}
{"type":"toolCallStarted","seq":42,"t":1794650018000,"sessionId":"1111…","agentId":"ab40f7a0b5c9d1404","call":{"toolUseId":"toolu_01…","name":"Read","summary":"src/stock.ts","startedAt":1794650018000,"endedAt":null,"isError":null},"tokens":13320,"outputTokens":180}
{"type":"fileTouched","seq":43,"t":1794650018000,"sessionId":"1111…","agentId":"ab40f7a0b5c9d1404","path":"/home/dev/projects/orchard/src/stock.ts","op":"read","source":"tool","toolUseId":"toolu_01…"}
{"type":"toolCallFinished","seq":44,"t":1794650019000,"sessionId":"1111…","agentId":"ab40f7a0b5c9d1404","toolUseId":"toolu_01…","isError":false}
```

### 3.3 Snapshot and reducer

`Snapshot = {seq, t, mode, sessions}` where `sessions` maps `sessionId` to a `Session` (the `SessionInfo`
fields plus `agents`, `runs` and `files` maps). `seq` is the last event applied, 0 when none.

`apply(s, e)`: first set `s.seq = e.seq` and `s.t = max(s.t, e.t)`. Then, with `S = s.sessions[e.sessionId]`
(if an event names a session, agent or run that is missing, ignore the event):

| event | effect |
|---|---|
| `snapshotReset` | `s = e.snapshot` (its `seq` equals `e.seq`) |
| `sessionUpserted` | if missing, insert `{...e.session, agents: {}, runs: {}, files: {}}`; else overwrite the `SessionInfo` fields and keep the three maps |
| `sessionRemoved` | delete the session and everything in it |
| `agentSpawned` | `S.agents[agent.agentId] = agent`; if `agent.runId` names a run in `S.runs` and its `agentIds` lacks this id, append it |
| `agentUpdated` | `S.agents[agent.agentId] = agent` (whole object replaced) |
| `toolCallStarted` | on agent `A`: append `call` to `A.recentTools` and drop from the front until 8 remain; `A.toolCalls += 1`; `A.lastTool = {name: call.name, summary: call.summary, at: call.startedAt}`; `A.tokens = e.tokens`; `A.outputTokens = e.outputTokens`; `A.lastActivityAt = max(A.lastActivityAt ?? 0, e.t)` |
| `toolCallFinished` | on agent `A`: if `A.recentTools` holds `toolUseId`, set its `endedAt = e.t` and `isError = e.isError`; `A.lastActivityAt = max(A.lastActivityAt ?? 0, e.t)` |
| `fileTouched` | `F = S.files[path]`, created as `{path, touches: {}}`; `T = F.touches[agentId]`, created as `{agentId, reads: 0, writes: 0, firstAt: e.t, lastAt: e.t, viaBash: false}`; increment `T.reads` or `T.writes` by `op`; `T.firstAt = min(T.firstAt, e.t)`; `T.lastAt = max(T.lastAt, e.t)`; if `source == "bash"` set `T.viaBash = true` |
| `runStarted`, `runUpdated` | `S.runs[run.runId] = run` (whole object replaced) |
| `runFinalized` | `S.runs[run.runId] = run`; then for each agent in `agents`: `S.agents[agent.agentId] = agent` |

The core emits `agentUpdated` whenever its own copy of an agent differs from what the tool events alone
would produce: a state change, a text-only message that changes `tokens`, a model that becomes known.

---

## 4. Reconstructing a workflow run in flight

The run file does not exist until the end, so a running workflow is assembled from four places. Every step
is incremental: the same code runs when the app starts in the middle of a run and when files change later.

1. **Discover the run.** Whichever comes first:
   - a `user` record in a transcript with `toolUseResult.taskType == "local_workflow"` and a `runId`: this
     gives `name` (`workflowName`), `summary`, `taskId`, `toolUseId` (the record's
     `tool_result.tool_use_id`), `startedAt` (the record's timestamp) and `parentAgentId` (the transcript's
     agent);
   - or any file under `subagents/workflows/<runId>/`: the run is created with `name = runId`,
     `parentAgentId = "main"`, `startedAt` = observation time, and filled in when the launch record is read.

   Emit `runStarted`, then `runUpdated` as details arrive.

2. **Declared phases.** Find the script text: the `script` input of the Workflow `tool_use` whose id is the
   run's `toolUseId`; else the file named by `toolUseResult.scriptPath`, re-rooted in the Claude dir by
   taking the part from `projects/` on (in a recording the absolute path belongs to another machine); else
   the single file matching `workflows/scripts/*-<runId>.js`. Then extract the phases without a JavaScript
   parser:
   - find `meta` followed by `=` and `{`, then inside it `phases` followed by `:` and `[`;
   - take the text up to the matching `]`, counting brackets and skipping string literals;
   - each top-level `{...}` in it is a phase; in each, read `title`, `detail`, `model` as
     `key: 'string'` or `key: "string"` with backslash escapes.

   Phases get `index` 1, 2, 3 in order. If anything fails, the run has no declared phases yet, and step 3
   creates them.

3. **Agents.** Each `agent-<id>.meta.json` or `agent-<id>.jsonl` in the run directory, and each journal
   `started` line, makes a workflow agent (whichever is seen first; the others fill in fields). `label` is
   the meta `description` or the journal `label`. Its phase is the meta `workflowPhase` or the journal
   `phase`, matched to a phase by exact title; a title that matches none is appended as a new phase with the
   next `index`. Emit `agentSpawned`.

4. **Activity.** The agent's transcript gives its task, model, tool calls, tokens and file touches exactly
   as for a plain subagent.

5. **Ends.** The journal gives the end state (section 2.3, workflow rules 2 to 4). Since journal lines carry
   no time, `endedAt` is the agent's `lastActivityAt` when the line is read, and the event's `t` is the
   observation time. Read the agent's transcript up to its current end before applying a journal line, so
   that the last tool calls are not attributed after the end.

6. **Retries.** On a `started` line whose `key` was already started by another agent that has no `result`
   and no `failed` line, the earlier agent becomes `failed` / `retried` / `superseded`.

7. **Phase states** are recomputed (section 2.6) whenever an agent of the run changes state; if any phase
   changed, emit one `runUpdated`.

8. **Run status** stays `running` until a completion notice with the run's `taskId` is read from the
   launching transcript, or the session goes `stale`, or the run file arrives.

9. **Finalisation: the run file replaces the reconstruction.** When `workflows/<runId>.json` appears (or is
   already there at startup), read it completely, then emit one `runFinalized` carrying:
   - the run rebuilt from the run file (section 2.6, right column), `final: true`; `phases` from the run
     file's `phases` and `workflow_phase` indexes, with states recomputed after the agents below;
   - **every** agent of the run, as full objects:
     - an agent whose `agentId` appears in a `workflow_agent` entry takes from it: `state` (section 2.3),
       `label`, `phaseIndex`, `phaseTitle`, `model`, `tokens`, `toolCalls`, `startedAt`, `queuedAt`,
       `attempt`, `endedAt` (`startedAt + durationMs`), `lastTool.name` and `lastTool.summary` (when
       `lastToolName` exists; `at` becomes `lastProgressAt`), `resultPreview`. It keeps from the
       reconstruction: `task`, `recentTools`, `outputTokens`, `agentType`, `spawnToolUseId`,
       `lastActivityAt`;
     - a `workflow_agent` entry without `agentId` (it never started) becomes a new agent with the synthetic
       id `<runId>#<index>`, `task` from `promptPreview` through the task rule, no tools, `tokens: 0`;
     - an agent of the run that no entry lists stays, with the state from workflow rule 1 (normally
       `failed` / `retried`, `superseded: true`).

   File touches are not affected: they only ever come from transcripts. Transcripts of a finalised run are
   still read to their end, and late records still produce tool and file events, but they no longer change
   `state`, `tokens` or `toolCalls` of a finalised agent. If the run file changes again, finalise again.

---

## 5. Tauri surface

### 5.1 Commands

All commands are `async`, return `Result<T, CommandError>`, and are invoked with camelCase argument names
(`invoke("replay_seek", { t })`).

| command | arguments | returns | effect |
|---|---|---|---|
| `get_snapshot` | none | `Snapshot` | the current state; never blocks on file reading |
| `get_status` | none | `SourceStatus` | mode, Claude dir, recording, replay clock |
| `list_recordings` | `dir?: string` | `RecordingInfo[]` | recordings found in `dir`, or in the default places below; newest first |
| `open_recording` | `path: string` | `SourceStatus` | switch to replay of that recording, **paused at the timeline start**; emits `snapshotReset` |
| `replay_play` | none | `SourceStatus` | start or resume; at the end of the timeline it restarts from the start |
| `replay_pause` | none | `SourceStatus` | |
| `replay_seek` | `t: EpochMs` | `SourceStatus` | clamp `t` to the timeline, rebuild, emit `snapshotReset`; keeps playing or paused as before |
| `replay_set_speed` | `speed: number` | `SourceStatus` | clamped to 0.1 .. 600; applies from the current position |
| `switch_to_live` | none | `SourceStatus` | close the recording, watch the Claude dir, emit `snapshotReset` |
| `pick_recording_folder` | none | `PickedFolder` | show the native folder dialog (opened from Rust, so the webview needs no dialog permission); returns the folder, or `null` when cancelled, and the recordings in it. A folder that is itself a recording returns just that one |
| `get_tool_history` | none | `ToolHistory` | every tool call the core keeps per agent of the visible sessions (the newest 2000 per agent), and in replay the time of every fs-event of the recording. The UI calls it after every snapshot it starts from (start-up, resync, `snapshotReset`) and then keeps it current from `toolCallStarted` / `toolCallFinished` events |
| `set_skip_gaps` | `on: boolean` | `SourceStatus` | quiet-gap skipping (6.2), default on. Allowed in live mode: it applies to the next recording |
| `set_claude_dir` | `dir: string \| null` | `SourceStatus` | watch another Claude dir; `null` goes back to the default (`CLAUDE_CONFIG_DIR`, else `~/.claude`). In live mode the source is rebuilt and a `snapshotReset` follows; in replay it applies on the next `switch_to_live`. Fails with `notFound` when `dir` is not a folder |
| `pick_claude_dir` | none | `PickedPath` | native folder dialog for the Claude dir; changes nothing by itself (the UI passes the path to `set_claude_dir` and remembers it in its settings) |

The four `replay_*` commands fail with `code: "notReplaying"` in live mode. `CommandError` is
`{code, message}` with `code` one of `notReplaying`, `notFound`, `invalidRecording`, `io`, `internal`.

Default recording places, in this order, duplicates removed: `$VIRGIL_RECORDINGS_DIR`; the app data
dir's `recordings/`; `./recordings` under the process's working directory (development); in debug builds
also the repository's `recordings/` (`tauri dev` runs with `src-tauri/` as its working directory). A
recording is a directory that contains `manifest.json` with `format: 1`. `list_recordings(dir)` also
accepts a recording directory itself and returns just that one.

The app starts in live mode. If the Claude dir does not exist, live mode still starts, with an empty
snapshot and `SourceStatus.error` set.

### 5.2 Event channels

- **`virgil:events`**: payload `EventBatch = {events: Event[]}`, events in `seq` order, batches in
  order. The shell ticks the core every 100 ms and sends **at most one batch per tick**,
  holding everything that tick produced, so bursts are coalesced and the UI gets at most 10 batches a second
  (a batch can be large: the first live scan or a fast replay).
- **`virgil:status`**: payload `SourceStatus`. Sent on every change of mode, recording, play state,
  speed, skip-gaps, Claude dir or problems, and about four times a second while a replay is playing, to move
  the clock. `problems` lists parse and read warnings (a bad line, a file that cannot be parsed or read, a
  file that got shorter), merged per path and reason with a count, newest first, at most 50. Commands do not emit
  anything themselves: their events and status go out on the next tick (within 100 ms), which keeps both
  channels in order.

UI start-up, and after any error: listen on both channels first, then call `get_snapshot`; drop buffered
events with `seq <= snapshot.seq`, apply the rest. If an event arrives with `seq` greater than the last
`seq + 1`, call `get_snapshot` again.

---

## 6. Replay (recording format 1)

A recording directory, written by `tools/recorder.py`:

```
manifest.json     {format: 1, claudeDir, sessionId, projectSlug, startedAt, pollMs, roots, stoppedAt?, eventCount?}
fs-events.jsonl   {seq, t, op: "create"|"modify"|"delete", path, size, mtimeMs, snapshot?}
snapshots/<seq, 7 digits>-<basename>     content of a .json file at that event
final/<path>      copy of the project roots taken when recording stopped (not `sessions/`)
```

`path` is relative to the Claude dir, with forward slashes. `stoppedAt`, `eventCount` and `final/` are
missing while a recording is still running, and after a recorder crash.

The recorder polls. That shapes the data:

- The first poll emits a `create` for every file that already existed, with `size` equal to the whole file
  and `t` a few milliseconds **after** `startedAt`. These events, defined as every event with
  `t < startedAt + pollMs`, are the **initial scan**. The state at the start of the timeline is therefore
  not empty.
- Changes are seen up to `pollMs` late, and several appends between two polls become one `modify`.
- A `size` can fall in the middle of a line.
- The recorder watches every `sessions/*.json` but only one session's project files. Other sessions appear
  with `hasTranscript: false` and only a `main` agent.

### 6.1 The file view at time `t`

The replay source gives the adapter the same interface as the live source (list, size, read bytes), computed
from the events with `event.t <= t`:

| path | exists at `t` | content at `t` |
|---|---|---|
| ends with `.jsonl` | last event is not `delete` | the first `size` bytes of `final/<path>`, `size` from the last event. The adapter's usual rule (parse up to the last `\n`) deals with a `size` in the middle of a line. If `final/<path>` is shorter than `size`, serve what there is |
| ends with `.json` | last event is not `delete` | the file named by the last event's `snapshot`. If that event has no `snapshot` (file over 8 MB, or the copy failed): `final/<path>` when this is the path's last event in the recording, else the previous snapshot |
| anything else (`.js` scripts) | last event is not `delete` | `final/<path>`, whole |

If `final/` is missing (or `stoppedAt` is), the recording is `complete: false`. It can still be opened: for
`.jsonl` and other files the source falls back to `<manifest.claudeDir>/<path>` on this machine, read-only
and cut to `size`, and skips files it cannot find. The fallback is per file, so it also covers a path that
`final/` lacks. `sessions/*.json` never depend on `final/`. While a recording is still running, the replay
source re-reads `fs-events.jsonl` about once a second and the timeline `end` grows.

The adapter and model code must be identical for live and replay. Replay differs only in: the file view
above, the clock (`now` is the replay position), and liveness (`unknown`).

### 6.2 Timeline, playing and seeking

- **Bounds.** `start = manifest.startedAt`. `end = manifest.stoppedAt`, else the last fs-event's `t`, else
  `start`. The position is always within `[start, end]`.
- **Playing.** A virtual clock advances by `speed` × wall time from the current position. Each tick, every
  fs-event with `t <= position` not yet applied is applied in `seq` order: the source reports the changed
  path to the adapter, the adapter reads the new bytes, and the resulting events are emitted with fresh
  `seq` numbers. At `end` the clock stops and `playing` becomes `false`.
- **The initial scan is always applied**, at every position including `start`: a position `p` means "all
  fs-events with `t <= max(p, startedAt + pollMs - 1)`".
- **Seeking is a rebuild.** `replay_seek(t)` throws away all state, applies the initial scan and every
  other fs-event with `event.t <= t`, in `seq` order with no pacing, and then emits a single `snapshotReset`; the events
  produced during the rebuild are not sent. The result must be identical to having played from the start
  to `t`. Seeking forward may be implemented as fast-forward from the current position only if it gives the
  same snapshot. `open_recording` is a seek to `start`.
- **Skipping quiet gaps** (`set_skip_gaps`, default on). While playing, when the stretch between the last
  applied fs-event (else `start`) and the next one (else `end`) is longer than 30 s, the clock runs at
  `max(speed, stretch / 1 s)`, so the stretch plays in about one second of wall time. Nothing is dropped:
  every fs-event is still applied in order, so the snapshot is the same as without skipping. The UI hatches
  the same stretches on the scrubber, from `ToolHistory.timelineTimes`.
- **Event times.** Events built from transcript records keep the record's timestamp as `t`, which can be up
  to `pollMs` earlier than the fs-event that revealed it, and much earlier for records that predate the
  recording. Events without a record time (journal lines, session files, the run file) use the fs-event's
  `t`. So an event's `t` is never after the position, but it can be far before it.

---

## 7. Types

### 7.1 TypeScript (copy verbatim into `src/lib/contract.ts`)

```ts
// Virgil data contract, version 1. Source of truth: docs/data-contract.md.
// Do not edit by hand without changing the document.

export type EpochMs = number;
export type SessionId = string;
/** Unique within a session. The main thread is the literal "main". */
export type AgentId = string;
export type RunId = string;
export type ToolUseId = string;

export const MAIN_AGENT_ID: AgentId = "main";
export const RECENT_TOOLS_MAX = 8;
export const EVENT_CHANNEL = "virgil:events";
export const STATUS_CHANNEL = "virgil:status";

// ---------- domain model ----------

export type Liveness = "alive" | "stale" | "unknown";
export type AgentKind = "main" | "subagent" | "workflow";
export type AgentState = "queued" | "running" | "waiting" | "done" | "failed" | "stopped";
export type RunStatus = "running" | "completed" | "failed" | "stopped";
export type PhaseState = "pending" | "running" | "done" | "failed";
export type FileOp = "read" | "write";
export type TouchSource = "tool" | "bash";
export type SourceMode = "live" | "replay";

export interface SessionInfo {
  sessionId: SessionId;
  pid: number | null;
  liveness: Liveness;
  cwd: string | null;
  projectSlug: string | null;
  /** Short derived name from the session file. */
  name: string | null;
  /** AI-written title from the transcript. */
  title: string | null;
  /** "busy", "idle", or whatever Claude Code writes. */
  status: string | null;
  kind: string | null;
  version: string | null;
  startedAt: EpochMs | null;
  updatedAt: EpochMs | null;
  statusUpdatedAt: EpochMs | null;
  hasTranscript: boolean;
}

export interface Session extends SessionInfo {
  agents: Record<AgentId, Agent>;
  runs: Record<RunId, WorkflowRun>;
  /** Keyed by normalised absolute path. */
  files: Record<string, FileEntry>;
}

export interface ToolCall {
  toolUseId: ToolUseId;
  name: string;
  /** At most 120 characters; may be empty. */
  summary: string;
  startedAt: EpochMs;
  endedAt: EpochMs | null;
  /** null until the result arrives. */
  isError: boolean | null;
}

export interface LastTool {
  name: string;
  summary: string;
  at: EpochMs;
}

export interface Agent {
  agentId: AgentId;
  sessionId: SessionId;
  kind: AgentKind;
  /** null only for the main agent. May name an agent that is not in the snapshot yet. */
  parentAgentId: AgentId | null;
  spawnToolUseId: ToolUseId | null;
  /** 0 for main. */
  spawnDepth: number;
  agentType: string | null;
  name: string | null;
  /** Short display name. */
  label: string;
  /** High-level task, at most 280 characters; empty when unknown. */
  task: string;
  model: string | null;
  state: AgentState;
  stateReason: string | null;
  runId: RunId | null;
  /** 1-based; null outside workflows. */
  phaseIndex: number | null;
  phaseTitle: string | null;
  attempt: number | null;
  /** An abandoned attempt that a later agent replaced. */
  superseded: boolean;
  queuedAt: EpochMs | null;
  startedAt: EpochMs | null;
  endedAt: EpochMs | null;
  lastActivityAt: EpochMs | null;
  /** Context size at the latest request. */
  tokens: number;
  outputTokens: number;
  toolCalls: number;
  lastTool: LastTool | null;
  /** Newest RECENT_TOOLS_MAX calls, oldest first. */
  recentTools: ToolCall[];
  resultPreview: string | null;
  worktreePath: string | null;
}

export interface Phase {
  /** 1-based. */
  index: number;
  title: string;
  detail: string | null;
  model: string | null;
  state: PhaseState;
}

export interface WorkflowRun {
  runId: RunId;
  sessionId: SessionId;
  name: string;
  summary: string | null;
  status: RunStatus;
  /** true once the run file has been read. */
  final: boolean;
  parentAgentId: AgentId;
  toolUseId: ToolUseId | null;
  taskId: string | null;
  startedAt: EpochMs | null;
  endedAt: EpochMs | null;
  phases: Phase[];
  /** First-seen order. */
  agentIds: AgentId[];
  /** null until final; sum over agentIds meanwhile. */
  totalTokens: number | null;
  totalToolCalls: number | null;
  declaredAgentCount: number | null;
  defaultModel: string | null;
  error: string | null;
}

export interface FileTouch {
  agentId: AgentId;
  reads: number;
  writes: number;
  firstAt: EpochMs;
  lastAt: EpochMs;
  /** At least one touch was guessed from a Bash command. */
  viaBash: boolean;
}

export interface FileEntry {
  path: string;
  touches: Record<AgentId, FileTouch>;
}

export interface Snapshot {
  /** seq of the last event applied; 0 when none. */
  seq: number;
  t: EpochMs;
  mode: SourceMode;
  sessions: Record<SessionId, Session>;
}

// ---------- events ----------

interface EventBase {
  seq: number;
  t: EpochMs;
}

export interface SnapshotResetEvent extends EventBase {
  type: "snapshotReset";
  snapshot: Snapshot;
}
export interface SessionUpsertedEvent extends EventBase {
  type: "sessionUpserted";
  session: SessionInfo;
}
export interface SessionRemovedEvent extends EventBase {
  type: "sessionRemoved";
  sessionId: SessionId;
}
export interface AgentSpawnedEvent extends EventBase {
  type: "agentSpawned";
  sessionId: SessionId;
  agent: Agent;
}
export interface AgentUpdatedEvent extends EventBase {
  type: "agentUpdated";
  sessionId: SessionId;
  agent: Agent;
}
export interface ToolCallStartedEvent extends EventBase {
  type: "toolCallStarted";
  sessionId: SessionId;
  agentId: AgentId;
  call: ToolCall;
  /** The agent's tokens and outputTokens after this call's message. */
  tokens: number;
  outputTokens: number;
}
export interface ToolCallFinishedEvent extends EventBase {
  type: "toolCallFinished";
  sessionId: SessionId;
  agentId: AgentId;
  toolUseId: ToolUseId;
  isError: boolean;
}
export interface FileTouchedEvent extends EventBase {
  type: "fileTouched";
  sessionId: SessionId;
  agentId: AgentId;
  path: string;
  op: FileOp;
  source: TouchSource;
  toolUseId: ToolUseId;
}
export interface RunStartedEvent extends EventBase {
  type: "runStarted";
  sessionId: SessionId;
  run: WorkflowRun;
}
export interface RunUpdatedEvent extends EventBase {
  type: "runUpdated";
  sessionId: SessionId;
  run: WorkflowRun;
}
export interface RunFinalizedEvent extends EventBase {
  type: "runFinalized";
  sessionId: SessionId;
  run: WorkflowRun;
  /** Every agent of the run, as full replacements. */
  agents: Agent[];
}

export type VirgilEvent =
  | SnapshotResetEvent
  | SessionUpsertedEvent
  | SessionRemovedEvent
  | AgentSpawnedEvent
  | AgentUpdatedEvent
  | ToolCallStartedEvent
  | ToolCallFinishedEvent
  | FileTouchedEvent
  | RunStartedEvent
  | RunUpdatedEvent
  | RunFinalizedEvent;

export type VirgilEventType = VirgilEvent["type"];

/** Payload of EVENT_CHANNEL. */
export interface EventBatch {
  events: VirgilEvent[];
}

// ---------- Tauri surface ----------

export interface RecordingInfo {
  /** Absolute path of the recording directory. */
  path: string;
  /** Directory name. */
  name: string;
  format: number;
  sessionId: SessionId | null;
  projectSlug: string | null;
  startedAt: EpochMs;
  /** Timeline end: stoppedAt, else the last event's time. */
  endedAt: EpochMs;
  eventCount: number;
  /** false while still recording, or when final/ is missing. */
  complete: boolean;
}

export interface ReplayState {
  playing: boolean;
  speed: number;
  position: EpochMs;
  start: EpochMs;
  end: EpochMs;
  /** Quiet stretches of more than 30 s play in 1 s (section 6.2). */
  skipGaps: boolean;
}

export type ProblemReason = "badLine" | "badFile" | "truncated" | "readError";

/** A parse or read warning, merged per (path, reason). */
export interface Problem {
  /** Source time of the latest occurrence. */
  t: EpochMs;
  /** Relative to the Claude dir. */
  path: string;
  reason: ProblemReason;
  count: number;
}

/** Payload of STATUS_CHANNEL and the return value of most commands. */
export interface SourceStatus {
  mode: SourceMode;
  /** The Claude dir being watched; null in replay. */
  claudeDir: string | null;
  recording: RecordingInfo | null;
  /** null in live mode. */
  replay: ReplayState | null;
  /** A problem with the source, e.g. the Claude dir is missing. */
  error: string | null;
  /** Parse and read warnings of the current source, newest first, at most 50. */
  problems: Problem[];
}

/** Every tool call the core keeps for one agent (at most 2000, the newest), oldest first. */
export interface AgentToolHistory {
  sessionId: SessionId;
  agentId: AgentId;
  calls: ToolCall[];
}

/** Result of `get_tool_history`. */
export interface ToolHistory {
  /** seq of the snapshot this history belongs to. */
  seq: number;
  agents: AgentToolHistory[];
  /** Replay: the time of every fs-event of the recording, ascending (whole timeline). Live: []. */
  timelineTimes: EpochMs[];
}

export type CommandErrorCode = "notReplaying" | "notFound" | "invalidRecording" | "io" | "internal";

export interface CommandError {
  code: CommandErrorCode;
  message: string;
}

/** Result of `pick_recording_folder`: the chosen folder, canonical (null when cancelled), and the recordings
 *  in it. When the folder is itself a recording, `recordings` is just that one and its `path` equals `path`. */
export interface PickedFolder {
  path: string | null;
  recordings: RecordingInfo[];
}

/** Result of `pick_claude_dir`: the chosen folder, canonical, or null when cancelled. */
export interface PickedPath {
  path: string | null;
}

/** Command name -> argument object and return type, for a typed invoke wrapper. */
export interface Commands {
  get_snapshot: { args: Record<string, never>; returns: Snapshot };
  get_status: { args: Record<string, never>; returns: SourceStatus };
  list_recordings: { args: { dir?: string }; returns: RecordingInfo[] };
  open_recording: { args: { path: string }; returns: SourceStatus };
  replay_play: { args: Record<string, never>; returns: SourceStatus };
  replay_pause: { args: Record<string, never>; returns: SourceStatus };
  replay_seek: { args: { t: EpochMs }; returns: SourceStatus };
  replay_set_speed: { args: { speed: number }; returns: SourceStatus };
  switch_to_live: { args: Record<string, never>; returns: SourceStatus };
  pick_recording_folder: { args: Record<string, never>; returns: PickedFolder };
  get_tool_history: { args: Record<string, never>; returns: ToolHistory };
  set_skip_gaps: { args: { on: boolean }; returns: SourceStatus };
  set_claude_dir: { args: { dir: string | null }; returns: SourceStatus };
  pick_claude_dir: { args: Record<string, never>; returns: PickedPath };
}
```

### 7.2 Rust sketch (`crates/core/src/contract.rs`)

The serialised form must match 7.1 exactly; names and module layout are a suggestion. `Option<T>` fields
serialise as `null` (no `skip_serializing_if`). Maps are `BTreeMap` so output is stable in tests.

```rust
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub type EpochMs = i64;
pub const MAIN_AGENT_ID: &str = "main";
pub const RECENT_TOOLS_MAX: usize = 8;
pub const EVENT_CHANNEL: &str = "virgil:events";
pub const STATUS_CHANNEL: &str = "virgil:status";

macro_rules! string_enum { /* derives Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq
                              with #[serde(rename_all = "camelCase")] */ }

pub enum Liveness { Alive, Stale, Unknown }
pub enum AgentKind { Main, Subagent, Workflow }
pub enum AgentState { Queued, Running, Waiting, Done, Failed, Stopped }
pub enum RunStatus { Running, Completed, Failed, Stopped }
pub enum PhaseState { Pending, Running, Done, Failed }
pub enum FileOp { Read, Write }
pub enum TouchSource { Tool, Bash }
pub enum SourceMode { Live, Replay }

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SessionInfo {
    pub session_id: String,
    pub pid: Option<u32>,
    pub liveness: Liveness,
    pub cwd: Option<String>,
    pub project_slug: Option<String>,
    pub name: Option<String>,
    pub title: Option<String>,
    pub status: Option<String>,
    pub kind: Option<String>,
    pub version: Option<String>,
    pub started_at: Option<EpochMs>,
    pub updated_at: Option<EpochMs>,
    pub status_updated_at: Option<EpochMs>,
    pub has_transcript: bool,
}

#[serde(rename_all = "camelCase")]
pub struct Session {
    #[serde(flatten)]
    pub info: SessionInfo,
    pub agents: BTreeMap<String, Agent>,
    pub runs: BTreeMap<String, WorkflowRun>,
    pub files: BTreeMap<String, FileEntry>,
}

#[serde(rename_all = "camelCase")]
pub struct ToolCall {
    pub tool_use_id: String,
    pub name: String,
    pub summary: String,
    pub started_at: EpochMs,
    pub ended_at: Option<EpochMs>,
    pub is_error: Option<bool>,
}

#[serde(rename_all = "camelCase")]
pub struct LastTool { pub name: String, pub summary: String, pub at: EpochMs }

#[serde(rename_all = "camelCase")]
pub struct Agent {
    pub agent_id: String,
    pub session_id: String,
    pub kind: AgentKind,
    pub parent_agent_id: Option<String>,
    pub spawn_tool_use_id: Option<String>,
    pub spawn_depth: u32,
    pub agent_type: Option<String>,
    pub name: Option<String>,
    pub label: String,
    pub task: String,
    pub model: Option<String>,
    pub state: AgentState,
    pub state_reason: Option<String>,
    pub run_id: Option<String>,
    pub phase_index: Option<u32>,
    pub phase_title: Option<String>,
    pub attempt: Option<u32>,
    pub superseded: bool,
    pub queued_at: Option<EpochMs>,
    pub started_at: Option<EpochMs>,
    pub ended_at: Option<EpochMs>,
    pub last_activity_at: Option<EpochMs>,
    pub tokens: u64,
    pub output_tokens: u64,
    pub tool_calls: u32,
    pub last_tool: Option<LastTool>,
    pub recent_tools: Vec<ToolCall>,
    pub result_preview: Option<String>,
    pub worktree_path: Option<String>,
}

#[serde(rename_all = "camelCase")]
pub struct Phase {
    pub index: u32,
    pub title: String,
    pub detail: Option<String>,
    pub model: Option<String>,
    pub state: PhaseState,
}

#[serde(rename_all = "camelCase")]
pub struct WorkflowRun {
    pub run_id: String,
    pub session_id: String,
    pub name: String,
    pub summary: Option<String>,
    pub status: RunStatus,
    #[serde(rename = "final")]
    pub is_final: bool,
    pub parent_agent_id: String,
    pub tool_use_id: Option<String>,
    pub task_id: Option<String>,
    pub started_at: Option<EpochMs>,
    pub ended_at: Option<EpochMs>,
    pub phases: Vec<Phase>,
    pub agent_ids: Vec<String>,
    pub total_tokens: Option<u64>,
    pub total_tool_calls: Option<u64>,
    pub declared_agent_count: Option<u32>,
    pub default_model: Option<String>,
    pub error: Option<String>,
}

#[serde(rename_all = "camelCase")]
pub struct FileTouch {
    pub agent_id: String,
    pub reads: u32,
    pub writes: u32,
    pub first_at: EpochMs,
    pub last_at: EpochMs,
    pub via_bash: bool,
}

pub struct FileEntry { pub path: String, pub touches: BTreeMap<String, FileTouch> }

pub struct Snapshot {
    pub seq: u64,
    pub t: EpochMs,
    pub mode: SourceMode,
    pub sessions: BTreeMap<String, Session>,
}

/// Serialises as {"type": "...", "seq": 1, "t": 0, ...payload}.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Event {
    pub seq: u64,
    pub t: EpochMs,
    #[serde(flatten)]
    pub kind: EventKind,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum EventKind {
    SnapshotReset { snapshot: Box<Snapshot> },
    SessionUpserted { session: SessionInfo },
    SessionRemoved { session_id: String },
    AgentSpawned { session_id: String, agent: Box<Agent> },
    AgentUpdated { session_id: String, agent: Box<Agent> },
    ToolCallStarted { session_id: String, agent_id: String, call: ToolCall, tokens: u64, output_tokens: u64 },
    ToolCallFinished { session_id: String, agent_id: String, tool_use_id: String, is_error: bool },
    FileTouched { session_id: String, agent_id: String, path: String, op: FileOp, source: TouchSource, tool_use_id: String },
    RunStarted { session_id: String, run: Box<WorkflowRun> },
    RunUpdated { session_id: String, run: Box<WorkflowRun> },
    RunFinalized { session_id: String, run: Box<WorkflowRun>, agents: Vec<Agent> },
}

pub struct EventBatch { pub events: Vec<Event> }

impl Snapshot {
    /// The reducer of section 3.3. The core and the tests use this one function.
    pub fn apply(&mut self, event: &Event) { /* ... */ }
}

#[serde(rename_all = "camelCase")]
pub struct RecordingInfo {
    pub path: String,
    pub name: String,
    pub format: u32,
    pub session_id: Option<String>,
    pub project_slug: Option<String>,
    pub started_at: EpochMs,
    pub ended_at: EpochMs,
    pub event_count: u64,
    pub complete: bool,
}

pub struct ReplayState { pub playing: bool, pub speed: f64, pub position: EpochMs, pub start: EpochMs, pub end: EpochMs, pub skip_gaps: bool }

#[serde(rename_all = "camelCase")]
pub struct SourceStatus {
    pub mode: SourceMode,
    pub claude_dir: Option<String>,
    pub recording: Option<RecordingInfo>,
    pub replay: Option<ReplayState>,
    pub error: Option<String>,
    pub problems: Vec<Problem>, // Problem { t, path, reason: ProblemReason, count }
}

// ToolHistory { seq, agents: Vec<AgentToolHistory { session_id, agent_id, calls: Vec<ToolCall> }>, timeline_times }

pub enum CommandErrorCode { NotReplaying, NotFound, InvalidRecording, Io, Internal } // camelCase
pub struct CommandError { pub code: CommandErrorCode, pub message: String }
```

Source side (not part of the UI contract; a suggested shape so that live and replay share everything else):

```rust
/// A path relative to the Claude dir, forward slashes.
pub struct FileChange { pub path: String, pub op: ChangeOp, pub t: EpochMs } // Create | Modify | Delete

/// What the engine reads through; identical for live and replay.
pub trait FileView {
    fn list_dir(&self, dir: &str) -> Vec<DirEntry>;          // DirEntry { name, is_dir }
    fn size(&self, path: &str) -> Option<u64>;
    fn mtime(&self, path: &str) -> Option<EpochMs>;
    /// Bytes [from, to) of the file as it is "now" for this source.
    fn read_range(&self, path: &str, from: u64, to: u64) -> std::io::Result<Vec<u8>>;
    fn read_all(&self, path: &str) -> std::io::Result<Vec<u8>>;
}

pub trait Source: FileView + Send {
    fn mode(&self) -> SourceMode;
    /// Changes since the last call, in order. Live: from the watcher. Replay: at most ONE fs-event per
    /// call, with the file view advanced to just after it, so play and seek read identical bytes.
    fn poll(&mut self) -> Vec<FileChange>;
    fn now(&self) -> EpochMs;
    fn liveness(&self, pid: u32) -> Liveness;
}
```

As implemented, the shell drives one `virgil_core::Hub` (it owns the source, the engine, the replay
clock and the `seq` counter): `Hub::new_default()`, `tick() -> Vec<Event>` every ~100 ms, `snapshot()`,
`status()`, `tool_history()`, `open_recording(&Path)`, `switch_to_live()`, `set_claude_dir(Option<PathBuf>)`,
`set_skip_gaps(bool)`, `replay_play/pause/seek/set_speed`, plus
`list_recordings(&[PathBuf])` and `default_recording_dirs(app_data_dir)`. In live mode a 5 s liveness
check and a 10 s rescan (the safety net for missed watcher events) run inside `tick`, and file reading is
limited to about 50 ms per tick, so the initial scan of a large history is spread over several ticks.

Tauri commands in `src-tauri` have the signatures of section 5.1, for example
`#[tauri::command] async fn replay_seek(state: State<'_, App>, t: i64) -> Result<SourceStatus, CommandError>`.

---

## 8. Things that are easy to get wrong

1. Assistant records come one per content block; group by `message.id` before counting tokens.
2. Workflow agents end in `journal.jsonl`, not in their meta. Plain subagents end in their meta
   (`finished`, `stoppedByUser`) and in the parent's completion notices.
3. The journal has no times. In live mode, lines that predate the app get the agent's last transcript
   timestamp as `endedAt`.
4. `pickedEffortModel` is not the agent's model.
5. A Workflow call has `script` or `scriptPath`, and the script is JavaScript, not JSON. Phases may be
   unavailable; the reconstruction must work from phase titles alone.
6. Phase indexes are 1-based, and `workflowPhase` in a meta is a title.
7. A retried attempt leaves an agent that the run file does not mention.
8. A run file entry can lack `agentId`.
9. The run file's `status` said `completed` for a run whose only agent errored. Show agent failures from
   the agents, not from the run status.
10. Absolute paths inside files (`scriptPath`, `transcriptDir`, `cwd`) belong to the recorded machine. Only
    `scriptPath` is ever opened, after re-rooting (section 4, step 2).
11. Main transcripts are large (41 MB observed) with multi-megabyte lines. Skip uninteresting record types
    cheaply, and read stale sessions after live ones.
12. Transcript text is private. Nothing from it is logged, persisted or copied into the repository; the
    fixtures are invented.
