# Virgil UX spec

Version 1. This document and `docs/data-contract.md` are the source of truth for the UI. If the two disagree about a field name or type, the data contract wins and this document gets corrected in the same change.

## 0. What the design has to carry

Measured on real Claude Code data on the owner's machine (shapes and counts only):

| Fact | Value seen | Design consequence |
|---|---|---|
| Sessions alive at once | about 10, most idle | Overview sorts busy first and stays readable with 20+ cards |
| Agents in one workflow run | 1, 13, 22, 45, 58 | every view has a defined 1 / 10 / 60 behaviour |
| Agents in one session | 160+ over five runs | Graph collapses finished runs; Timeline groups and virtualises rows |
| Agents running at the same moment | up to 8 | motion budget is sized for about 12 moving things, not 60 |
| Phases per run | 6, titles 3 to 6 characters, one or two phases holding 14 to 19 agents | phase columns scroll on their own; phase containers pack agents in a grid |
| Run length | under a minute to 9 h 06 min | time axis zooms from 100 ms per pixel to 1 min per pixel; replay goes to 120x and can skip quiet gaps |
| Wait in queue | up to 3 h 45 min | waiting is drawn as its own bar segment, not hidden |
| Agent label | 8 to 38 characters | node and card titles fit 2 lines of about 20 characters |
| Task prompt | a few hundred to 7,000+ characters | prompt is shown as a 2-line summary everywhere except the Inspector |
| Tool calls per agent | 0 to 85, about 80% are `Bash` | tool ticks are merged per row; "files touched" is known only for file tools (see section 7) |
| Files shared between agents in one run | 27 of 79 files, up to 7 agents on one file, 9 files edited by 2 or 3 agents | the shared-file overlay aggregates per pair and defaults to edits only |
| Tokens per agent | 40 k to 375 k; per run up to 7.9 M | numbers use short form (`41k`, `7.9M`) |
| Nesting | session, then agent, then agent (2 or 3 deep); workflow agents sit in run then phase | the graph is a left-to-right tree with at most 5 ranks |

Design principles:

1. Colour means state. Chrome is quiet; the brightest things on screen are agents that are running or failed.
2. One selection, one clock. Every view shows the same selected thing and the same moment in time.
3. Never colour alone. Every state has an icon shape and a word as well as a colour.
4. Plain words. "Waiting", "Running", "Done", "Failed", "Stopped", not internal terms.
5. Cheap by default. Layout runs only when structure changes; motion uses only `transform` and `opacity`.

## 1. Words and shared models

### 1.1 Things the UI shows

| UI word | Meaning | Comes from |
|---|---|---|
| Session | one Claude Code session; the root of everything | `sessions/<pid>.json` plus its transcript |
| Agent | a subagent started by the session or by another agent | `agent-<id>.jsonl` and `.meta.json` |
| Run | one workflow run inside a session | Workflow tool call, `workflows/<runId>.json` when finished |
| Phase | a named step of a run; holds agents | `meta.phases`, `workflowProgress` |
| Tool call | one tool use by a session or agent | `tool_use` blocks |
| File touch | an agent read or edited a file path | `Read`, `Edit`, `Write`, `NotebookEdit` tool calls |

### 1.2 States, with their fixed label, icon and colour token

Agent states:

| State | Label | Icon (16 px, 1.5 px stroke) | Token |
|---|---|---|---|
| queued | Queued | hollow circle with a clock hand | `--state-queued` |
| waiting | Waiting | hourglass | `--state-queued` |
| running | Running | ring with a rotating arc | `--state-running` |
| done | Done | circle with a check | `--state-done` |
| error | Failed | triangle with an exclamation mark | `--state-error` |
| stopped | Stopped | square inside a circle | `--state-stopped` |
| unknown | Unknown | dashed circle | `--state-idle` |

Session states: busy uses the Running icon and token with label "Busy"; waiting (Claude Code asks the person something; session file `status: "waiting"`) uses the Waiting hourglass, `--state-queued`, label "Waiting"; idle uses a pause-bars icon, `--state-idle`, label "Idle"; ended uses the Done icon in `--state-idle`, label "Ended"; stale (file exists, process gone) uses the dashed circle, `--state-idle`, label "Not running".

Run states: running, completed ("Done"), failed ("Failed"), stopped ("Stopped"), with the agent icons and tokens.

### 1.3 Agent types and models

Type is shown by a glyph in a 20 px rounded square tinted with the type token. Model is shown by a chip: a 14 px square in the model token holding one letter, followed by the short name.

| Agent type (`agentType`) | Label | Glyph | Token |
|---|---|---|---|
| (the session itself) | Session | terminal prompt `>_` | `--type-session` |
| `workflow-subagent` | Workflow agent | three stacked bars | `--type-workflow` |
| `general-purpose` | General agent | circle | `--type-general` |
| `fork` | Fork | Y-shaped branch | `--type-fork` |
| `Explore` | Explorer | magnifier | `--type-explore` |
| `Plan` | Planner | checklist | `--type-plan` |
| anything else | the raw type name, e.g. `web-engine` | diamond | `--type-custom` |

| Model id contains | Chip letter | Short name rule | Token |
|---|---|---|---|
| `opus` | O | `claude-opus-5-5` becomes "Opus 5.5" | `--model-opus` |
| `sonnet` | S | "Sonnet 5.5" | `--model-sonnet` |
| `haiku` | H | "Haiku 4.5" | `--model-haiku` |
| `fable` | F | "Fable 5.2" | `--model-fable` |
| none of these, or missing | ? | raw id, or "Unknown model" | `--model-other` |

Short name rule: drop the `claude-` prefix, capitalise the family, join the remaining number parts with a dot, drop date suffixes of 8 digits and bracket suffixes such as `[1m]`.

### 1.4 Selection, hover and the clock

Four small stores are shared by every view (owned by the foundation developer, section 6):

- `selection`: `{ kind: 'session' | 'agent' | 'run' | 'phase' | 'file', id: string } | null`. One thing at a time. Selecting anything opens the Inspector. `Esc` clears it. The selected thing is drawn with a 2 px ring in `--select-ring`, offset 2 px, in every view at once.
- `hover`: same shape, cleared on pointer leave. The hovered thing gets `--surface-hover` in the view, and the matching row in an Inspector list (and the other way round) gets a 1 px ring in `--border-strong`.
- `clock`: `{ mode: 'live' | 'replay', viewTime: number, playing: boolean, speed: number, range: [start, end] }`, times in epoch milliseconds. In live mode `viewTime` is now and advances once per second. In replay mode it is the playhead. Every view draws the world as of `viewTime`: an agent that starts after `viewTime` is not shown; one that ends after it is shown as running.
- `scope`: `{ sessionId: string | null, runId: string | null }`. Graph and Timeline need a session; Workflow needs a session and shows one run at a time.

### 1.5 Number and time formats

- Tokens and counts: under 1,000 as is; then `41k`, `375k`, `5.2M`, one decimal only below 10 (`7.9M`, `12M`).
- Durations: `42s`, `17m 20s`, `9h 06m`. Never milliseconds except in tooltips for tool calls under 1 s (`420 ms`).
- Relative time: "just now" (under 10 s), `35s ago`, `12m ago`, `3h ago`, then a date `12 Nov, 14:05`.
- Clock time: system locale, 24 h or 12 h by system setting. Replay shows both elapsed (`01:12:40`) and wall-clock time of the recording.
- Paths: shown relative to the session's `cwd` when inside it, otherwise with `~` for the home dir. Truncated in the middle (`src/…/Card.svelte`), full path in the tooltip.
- All numbers use tabular figures (`font-variant-numeric: tabular-nums`).

## 2. App frame

### 2.1 Window

- Native window decorations on all three platforms (no custom title bar in version 1). Title: `Virgil`.
- Default size 1360 x 860, minimum 960 x 600.
- The window is one CSS grid:

```
+------+---------------------------------------------------------+------------+
|      | TopBar (48)                                             |            |
| Nav  +---------------------------------------------------------+ Inspector  |
| Rail |                                                         | (360,      |
| (64) | View area                                               |  resizable |
|      |                                                         |  300-560,  |
|      |                                                         |  closable) |
|      +---------------------------------------------------------+            |
|      | ReplayBar (64, replay mode only)                        |            |
+------+---------------------------------------------------------+------------+
| StatusBar (26)                                                              |
+-----------------------------------------------------------------------------+
```

- Below 1100 px window width the Inspector overlays the view area from the right (with `--shadow-3`) instead of pushing it.
- Surfaces: window `--surface-app`; NavRail, TopBar, StatusBar, Inspector `--surface-panel`; Graph and Timeline drawing areas `--surface-canvas`. Panels are separated by a `--border-width` line in `--border-subtle`.

### 2.2 NavRail (left, 64 px)

Top to bottom: app mark (28 px), then four destinations, each a 48 x 48 button with a 20 px icon and an 11 px label under it: **Overview**, **Graph**, **Workflow**, **Timeline**. At the bottom: **Settings** (gear). The current destination has `--accent-soft` background, `--accent-text` icon and label, and a 3 px bar in `--accent` on its left edge. Graph, Workflow and Timeline are disabled (40% opacity, tooltip "Pick a session first") while `scope.sessionId` is null.

### 2.3 TopBar (48 px)

Left to right:

1. **Scope crumbs**: `All sessions › <session name> › <run name>`. Each crumb is a button opening a menu: the session crumb lists sessions (state icon, name, project, last activity; busy first); the run crumb lists that session's runs (state icon, workflow name, start time, agent count). The run crumb is shown only when the session has runs. On Overview only `All sessions` is shown.
2. Flexible space.
3. **Search field** (240 px, `Ctrl/Cmd+K` focuses it): filters the current view by text matched against agent label, task summary, type, model and file path. Non-matching agents drop to 25% opacity; they are not removed, so layouts do not jump. A count ("7 of 58") sits inside the field's right end. `Esc` clears.
4. **Mode switch**: a two-part segmented control, **Live** and **Replay**. See 2.6.
5. **Inspector toggle** (panel icon).

### 2.4 StatusBar (26 px, 12 px text, `--text-secondary`)

Left: a state dot and one sentence about the data source.

| Situation | Dot | Text |
|---|---|---|
| Live, watching | `--state-running`, slow pulse | `Watching ~/.claude` (actual path) |
| Live, dir missing | `--state-error` | `Claude folder not found: <path>` plus a **Choose folder** link |
| Live, a file could not be parsed | `--state-queued` | `Some files could not be read (3)` and a link opening the Problems list |
| Replay, loaded | `--state-idle` | `Replaying <recording name> · recorded 12 Nov, 10:27` |
| Replay, playing | `--state-running` | same, with `· 30x` |

Right: `3 sessions busy · 8 agents running`, then the time of the last received event (`Last update 2s ago`; turns `--state-queued-text` after 30 s without events while something is running), then the app version.

The **Problems list** is a popover from the StatusBar listing parse and read warnings from the core (time, file path, short reason). It is the only place raw file paths of the Claude dir appear.

### 2.5 Inspector frame

A right-hand panel that opens for whatever is selected (content in section 3.5). Header: type glyph, title, state badge, close button. A drag handle on its left edge resizes it; width persists. `Ctrl/Cmd+I` toggles it. With nothing selected and the panel open it shows "Select a session, agent, run, phase or file to see details."

### 2.6 Live and Replay

- **Live** is the default at start. The core watches the Claude dir and streams events.
- Choosing **Replay** opens the **Recording picker** dialog: a list of recent recordings (name, date, length, session name; kept in settings) and a **Open recording folder…** button that opens the native folder dialog. A valid folder contains `manifest.json` and `fs-events.jsonl`. Invalid folders show an inline error: "This folder is not an Virgil recording (manifest.json is missing)."
- After loading, the app sets `scope.sessionId` to the recording's session, opens the Graph view, shows the ReplayBar, and stays paused at the start with the hint "Press Space to play".
- While in replay the whole frame gets a 2 px top border in `--state-queued` and the Replay segment shows the recording name, so replay is never mistaken for live.
- Choosing **Live** again drops the recording and returns to the live stream and the previous live scope.
- Selection is cleared when the mode changes.

### 2.7 ReplayBar (64 px, replay mode only)

```
[|<] [<] [ Play ] [>] [>|]   01:12:40 / 09:06:17   [==== scrubber with density strip ====]   [30x v] [Skip quiet gaps] 
```

- **Play / Pause** (`Space`), **previous event / next event** (`,` and `.`), **start / end** (`Home`, `End`).
- **Scrubber**: full remaining width, 28 px tall. Its background is a density strip: one bar per pixel column, height = tool calls in that slice, in `--text-muted` at 50%. Phase starts are 1 px marks in `--border-strong` with the phase title on hover. Runs are shown as thin underlines in `--accent`. The playhead is a 2 px line with a 10 px round handle in `--accent`. Click seeks; drag scrubs (views update at most 10 times per second while dragging, and once exactly on release). `Left` / `Right` move 5 s of recording time, with `Shift` 1 min.
- **Time readout**: elapsed / total; hovering the scrubber shows a tooltip with elapsed time, wall-clock time and "N agents running".
- **Speed**: a menu with 1x, 2x, 5x, 10x, 30x, 60x, 120x. `[` and `]` step down and up. Default 10x.
- **Skip quiet gaps** (toggle, default on): any stretch of more than 30 s of recording time with no events plays in 1 s of real time. Skipped stretches are hatched on the scrubber. The core does the skipping (`set_skip_gaps`, data-contract 6.2) and reports the state in `ReplayState.skipGaps`; the hatching uses the recording's fs-event times from `get_tool_history`.
- At the end, playback pauses and the Play button becomes **Replay from start**.

### 2.8 Settings dialog

A modal dialog (560 px), opened from the NavRail or `Ctrl/Cmd+,`.

| Setting | Control | Values | Default |
|---|---|---|---|
| Theme | four preview tiles (each a tiny mock of a node on a canvas in that theme) | Graphite, Paper, Blueprint, Poster | Graphite |
| Mode | segmented | System, Light, Dark | System |
| Density | segmented | Comfortable, Compact | Comfortable |
| Motion | segmented | System, Full, Reduced | System |
| Time axis | segmented | Elapsed, Clock time | Elapsed |
| Claude folder | read-only path with **Choose folder…** and **Reset** | a dir path | auto-detected |

**Choose folder…** opens the native folder dialog (`pick_claude_dir`), then `set_claude_dir` switches live mode to that folder at once (the view returns to the Overview); the choice is kept in settings and sent to the core at every start-up. A folder that no longer exists is forgotten at start-up, with the reason shown under the path. **Reset** goes back to auto-detection. In replay the choice applies on returning to Live. The StatusBar's **Choose folder** link and the missing-folder state's button run the same flow.

Rules:

- Settings are stored in `localStorage` under the key `virgil.settings.v1` as one JSON object `{ theme, mode, density, motion, timeAxis, claudeDir, inspectorWidth, recentRecordings }`. Unknown keys are kept; missing keys take defaults.
- The theme is applied by setting `data-theme` and `data-mode` on `<html>`. With Mode = System, `data-mode` follows `prefers-color-scheme` and updates live when the system changes.
- A small inline script in `index.html` reads the stored settings and sets both attributes before the first paint, so there is no flash of the wrong theme.
- Changing theme or mode cross-fades colours over `--dur-2` (a transition on `background-color`, `color`, `border-color` applied to `html.theme-switching *` for 300 ms, then removed). Not animated under reduced motion.
- Density = Compact multiplies component paddings by 0.75 and row heights by 0.85 through `--density` (section 4.2).

### 2.9 Keyboard shortcuts

`Mod` is `Cmd` on macOS and `Ctrl` elsewhere. Shortcuts are ignored while a text field has focus, except `Esc` and `Mod` chords.

| Keys | Action |
|---|---|
| `1` `2` `3` `4` | Overview, Graph, Workflow, Timeline |
| `Mod+K` or `/` | focus search |
| `Mod+I` | toggle Inspector |
| `Mod+,` | Settings |
| `Esc` | close dialog or menu, else clear search, else clear selection |
| `L` / `R` | switch to Live / Replay |
| `Space` | play or pause (replay) |
| `,` `.` | previous / next event (replay) |
| `[` `]` | slower / faster (replay) |
| `Home` `End` | start / end of recording (replay); earliest / now (Timeline in live) |
| `Left` `Right` | seek 5 s (replay), with `Shift` 1 min |
| Arrow keys in a view | move selection to the neighbouring agent (Graph: by position; Workflow: within and across columns; Timeline: up and down rows) |
| `Enter` | open the selected session (Overview); focus the Inspector (other views) |
| `F` | fit everything in view (Graph, Timeline) |
| `+` `-` `0` | zoom in, zoom out, reset zoom (Graph, Timeline) |
| `S` | toggle the shared-files overlay (Graph) |
| `P` | select the parent of the selected agent |
| `?` | show the shortcut sheet |

Focus order is NavRail, TopBar, view, Inspector, ReplayBar. Every interactive element has a visible focus ring: 2 px `--focus-ring`, offset 2 px.

### 2.10 Empty, loading and error states

Each uses the `EmptyState` component: a 48 px line icon, a title (`--text-lg`), one sentence (`--text-secondary`), at most one button. Centered in the area it replaces.

| Where | When | Title | Sentence | Button |
|---|---|---|---|---|
| Whole app | first scan in progress | Looking for sessions… | Reading `<claude dir>` | none (skeleton cards behind) |
| Whole app | Claude dir missing | No Claude folder found | Virgil reads the files Claude Code writes in `<path>`. | Choose folder… |
| Whole app | core error | Something went wrong | the error text from the core, selectable | Try again |
| Overview | no sessions at all | No sessions yet | Start Claude Code in a terminal and it will show up here. | Open a recording… |
| Overview | search has no match | Nothing matches "x" | | Clear search |
| Graph, Timeline | session has no agents | Just the session so far | This session has not started any agents. | none (the session node or row is still drawn) |
| Workflow | session has no runs | No workflow runs | This session has not run a workflow. | Show graph |
| Workflow | run just started, no agents | Waiting for the first agent | | none (phase columns drawn empty) |
| Inspector | nothing selected | Nothing selected | Select a session, agent, run, phase or file to see details. | none |
| Replay | recording loading | Loading recording… | with a determinate progress bar if the core reports progress | Cancel |
| Replay | recording unreadable | Could not open recording | the reason | Choose another… |
| Any list section | no data | "None yet" in `--text-muted` (inline, no icon) | | |

Loading inside views uses skeleton blocks (`--surface-sunken`, shimmer 1.2 s; static under reduced motion), never spinners, and only if loading takes more than 150 ms.

## 3. Views

### 3.1 Overview

Purpose: see which sessions are alive and pick one.

Layout: a scrolling page with 24 px padding and two sections, **Running now** (session file present and process alive) and **Recent** (sessions seen in the last 7 days whose process is gone, newest first, first 24 with a **Show older** button). Each section title carries its count. Cards sit in a grid: `grid-template-columns: repeat(auto-fill, minmax(320px, 1fr))`, gap `--space-4`. A toolbar row above: project filter (menu of project names with counts) and sort (**Busy first** default, **Last activity**, **Name**).

`SessionCard` (min height 168 px):

```
+------------------------------------------------------+
| (o) Busy                                  2h 14m     |   state icon + label, time since start
| Session name on up to two lines                      |   --text-lg, display face
| pixel/virgil                                     |   project: last two parts of cwd, mono
|                                                      |
| ▁▂▂▅▇▅▂▁▁▂▃▇█▅▃▂▁▁▁▂▃▅▅▃▂                            |   activity sparkline
|                                                      |
| [o 3 running] [12 agents] [2 runs]        1.2M tok   |   count chips, token total
| Now: Build - 4 of 6 agents done                      |   only when a run is in flight
+------------------------------------------------------+
```

| Element | Encodes |
|---|---|
| State icon, label, left 3 px edge | session state (1.2) |
| Title | session name; falls back to the AI title, then to "Session " + first 8 characters of the id |
| Project | `cwd`, last two path parts; full path in tooltip |
| Time, top right | time since `startedAt` for live sessions; "ended 3h ago" for recent ones |
| Sparkline | tool calls per minute by the session and all its agents. Running sessions: last 30 minutes, 30 bars. Recent sessions: whole life in 30 equal slices. Bar colour `--state-running` for the newest 2 bars of a busy session, else `--text-muted`. Height scales to the card's own maximum. An empty series draws a flat 1 px line. |
| Chip "N running" | agents in state running; hidden when 0; carries the Running icon |
| Chip "N agents" | all agents ever started by the session |
| Chip "N runs" | workflow runs; hidden when 0 |
| Tokens | sum over the session's agents, short form; "–" when unknown |
| "Now:" line | the active run's current phase title and "x of y agents done" for that phase |

Interactions: hover raises the card (`--shadow-2`, translateY -1 px). Single click selects it (Inspector shows the session). Double click or `Enter` sets the scope to the session and opens Graph. A busy card's state icon rotates; nothing else on the card moves. A context menu offers **Open graph**, **Open workflow**, **Open timeline**, **Copy session id**.

Scale: with 1 session the card is capped at 480 px wide and left-aligned. With 10, two or three columns. With 60, the grid scrolls; cards outside the viewport use `content-visibility: auto` with `contain-intrinsic-size: 320px 168px`.

### 3.2 Graph

Purpose: who started whom, how the work is grouped, and what is alive right now.

Built with Svelte Flow; layout by elkjs in a web worker.

Layout:

```
+---------------------------------------------------------------------------+
| [Fit] [-] 80% [+]   Detail: Auto v   [x] Shared files  Edits only v       |  GraphToolbar, floats top-left
|                                                                           |
|            +- Run: build (running)  4/6 phases ----------------------+    |
|            | +- 1 Spec ---+   +- 2 Engine -+   +- 3 Screen ------+   |    |
|  [Session]-+ | [a] [a]    |-->| [a] [a]    |-->| [a] [a] [a] [a] |   |    |
|      |     | | [a]        |   | [a]        |   | [a] [a]         |   |    |
|      |     | +------------+   +------------+   +-----------------+   |    |
|      |     +---------------------------------------------------------+    |
|      +--[Agent]--[Agent]                                                  |
|      +--[Agent]                                             [minimap]     |
+---------------------------------------------------------------------------+
```

Structure:

- The session is the root node at the left. Direction is left to right (`elk.direction: RIGHT`, algorithm `layered`, `elk.hierarchyHandling: INCLUDE_CHILDREN`, node spacing 16, layer spacing 56).
- Each run is a container node (`RunGroup`). Inside it each phase is a container (`PhaseGroup`), placed left to right in phase order. Agents inside a phase are packed in a grid with `ceil(sqrt(n))` columns, at most 4 columns. This packing (and the placement of phases inside a run) is computed in `buildGraph.ts`, so each run is a fixed-size box and ELK lays out only the top level: session, plain agents and runs. (Deviation from version 1 of this spec, which had ELK `rectpacking` inside each phase: one algorithm, cheaper, and the same visual result.)
- Agents started with the Agent tool hang off whoever started them (the session or another agent) as ordinary nodes.
- Edges:
  - **Spawn edge**: parent to child, for non-workflow agents. Solid, 1.5 px, `--border-strong`, smooth-step path, small arrowhead.
  - **Run edge**: session (or the agent that started the run) to the `RunGroup`. Same style, 2 px.
  - **Sequence edge**: phase to next phase inside a run. 1 px, `--border-default`, no arrowhead fill. Workflow agents have no individual edges: being inside the phase box is the link. This keeps a 58-agent run at 6 edges, not 58.
  - **Shared-file link** (overlay, below).

`AgentNode`, three levels of detail:

| Level | Size | Shows | Used when |
|---|---|---|---|
| full | 248 x 104 | everything below | zoom at least 0.8 and its container holds 8 agents or fewer |
| compact | 168 x 44 | state icon, type glyph, label (1 line), model letter | zoom 0.45 to 0.8, or a container with 9 or more agents |
| dot | 28 x 28 | a filled circle in the state colour with the state icon in `--surface-card` | zoom below 0.45 |

The **Detail** menu in the toolbar overrides this: Auto (default), Full, Compact, Dots. ELK lays out with the size of the level in use; changing level counts as a structure change.

Full `AgentNode`:

```
+--+--------------------------------------------+
|  | [glyph] Agent label on one line     (o)    |   type glyph, label, state icon
|  | Task summary, up to two lines, second      |   --text-sm, --text-secondary
|  | line ends with an ellipsis…                |
|  | [S] Sonnet 5.5   41k tok   12 tools        |   model chip, tokens, tool calls
|  | > Bash  npm test                           |   last tool: name + summary, mono, 1 line
+--+--------------------------------------------+
 ^ 4 px state bar
```

| Element | Encodes |
|---|---|
| Left 4 px bar and state icon | agent state (1.2) |
| Border | `--border-default`; `--state-running` while running; `--state-error` when failed |
| Background | `--surface-card`; `--state-error-soft` when failed; 60% opacity overall when waiting |
| Type glyph | agent type (1.3) |
| Label | the agent's short description or workflow label |
| Task summary | first non-empty line of the prompt, cut at 120 characters (or the contract's summary field) |
| Model chip | model family and version |
| Tokens, tools | running totals as of `viewTime` |
| Last tool line | last tool name and its one-line summary as of `viewTime`; "Starting…" before the first tool call; replaced by the duration (`Took 17m 20s`) when done |
| Small badge "x2" top right | attempt number when above 1 |
| Small branch icon | the agent ran in its own git worktree |
| Stack badge "+3" on the right edge | the agent started 3 agents that are currently collapsed |

`SessionNode` (248 x 88): terminal glyph, session name, project, state, and counts "12 agents · 2 runs".

`RunGroup` header (36 px): Run state icon, workflow name, a stepper of small squares (one per phase, filled by phase state), elapsed time, agent count, collapse button. Collapsed, it becomes a 248 x 72 node showing name, state, "58 agents · 6 phases · 7.9M tok", and a segmented progress bar. Runs that finished before `viewTime` minus 10 minutes start collapsed when the session has more than one run; the newest run is always expanded.

`PhaseGroup` header (28 px): phase number, title, "5/6", and a 4 px segmented progress bar across the top edge (segments by agent state, in order done, running, waiting, failed, stopped). Border style comes from `--container-border-style`; background `--surface-panel` at 60%. A phase with no agents yet is drawn 168 x 72 with "No agents yet".

Interactions:

- Pan by dragging the background or with two-finger scroll; zoom with pinch or `Ctrl/Cmd` + wheel, range 0.15 to 2. Nodes are not draggable in version 1.
- Click a node, run or phase header to select it. Double-click a `RunGroup` to collapse or expand it. Double-click an agent to open Timeline scrolled to its row.
- Hover a node: after 400 ms a `Tooltip` shows label, state, full first line of the task, and elapsed time. Hovering also brightens its edges and dims unrelated nodes to 55%.
- Selecting an agent highlights its parent chain: the edges up to the session take `--select-ring` at 2 px.
- **Fit** (`F`) frames everything with 48 px padding. The view fits automatically on first open and whenever the structure changes, unless the user has panned or zoomed since the last fit; then a pill "New agents · Fit" appears at the top centre for 6 s instead.
- A `MiniMap` (160 x 110, bottom right) shows nodes as state-coloured rectangles; hidden under 6 nodes.

Shared-files overlay (toggle **Shared files**, `S`; off by default):

- A link joins two agents that touched the same file. One link per pair of agents, however many files they share. Width: 1 px for 1 file, 2 px for 2 to 4, 3 px for 5 or more. A small count label sits at the midpoint when zoom is at least 0.6.
- Style: dashed curve (`6 4`), `--accent-text` at 70% for pairs where both edited a shared file, `--text-muted` at 60% where at least one only read it. Drawn above spawn edges, below nodes.
- Filter menu: **Edits only** (default: both agents edited the file) or **Reads and edits**.
- With nothing selected, at most the 40 heaviest links are drawn; a note in the toolbar says "Showing 40 of 112 links".
- Selecting an agent shows all of that agent's links and hides the others. Selecting a file (from the Inspector) draws no links; instead every agent that touched the file gets a ring in `--accent` and a small "R" or "E" badge, and the rest dim to 40%.
- Clicking a link selects nothing but opens a popover listing the shared paths (each a button that selects the file).

Layout rules (cost):

- ELK runs only when the set of nodes, their parent, their detail level, or a collapse state changes. State, token, tool and time updates never trigger layout.
- Layout requests are debounced by 300 ms and run in ELK's own web worker (`elkjs/lib/elk-api` with `elk-worker`); a newer request makes the result of an older one be dropped. An automatic fit may change the automatic level of detail, at most three times per set of nodes (then it settles on compact), so a layout that does not fit at its own level cannot ping-pong. A selected node keeps its level (growing it would relayout on every click). New nodes fade in at their computed position; existing nodes glide (`--dur-3`) to new positions.
- Svelte Flow option `onlyRenderVisibleElements` is on.

Scale:

- 1 agent: session node, one edge, one full node, centered at zoom 1; never zoomed in past 1.
- 10 agents: all full nodes; fits a 1360 px window at about zoom 0.9.
- 60 agents: the two big phases switch to compact nodes in a 4-column grid; the whole run fits at about zoom 0.5; below 0.45 everything becomes dots and the graph reads as a state map. At most 12 edges animate (section 5).

### 3.3 Workflow

Purpose: progress of one run, phase by phase.

Layout:

```
+----------------------------------------------------------------------------+
| RunHeader: (o) build   Running 01:12:40     [run picker v]                 |
| [=======done=====|==run==|...waiting...]  Phase 4 of 6                     |
| 58 agents   31 done   6 running   20 waiting   1 failed   7.9M tok   1,636 tools |
+----------------------------------------------------------------------------+
| 1 Spec    6/6 | 2 Engine  6/6 | 3 Screen  4/6 | 4 Checks 0/3 | 5 … | 6 …   |
| [=========]   | [=========]   | [======--]    | [.........]  |     |       |
| +-----------+ | +-----------+ | +-----------+ |              |     |       |
| | AgentCard | | | AgentCard | | | AgentCard | |  No agents   |     |       |
| +-----------+ | +-----------+ | +-----------+ |  yet         |     |       |
| +-----------+ | +-----------+ | +-----------+ |              |     |       |
+----------------------------------------------------------------------------+
| Log  (3)                                                          [^]      |
| 10:41:07  line of text…                                                    |
+----------------------------------------------------------------------------+
```

`RunHeader`:

| Element | Encodes |
|---|---|
| State icon, name, state label | run state and workflow name |
| Timer | elapsed since start as of `viewTime`; total duration when finished |
| Run picker | other runs of the session (also in the TopBar crumbs) |
| Whole-run bar (8 px) | one segment per phase, equal widths. Each segment fills by the share of that phase's agents that are done, in `--state-done`; running share in `--state-running` (with a moving sheen, section 5); failed share in `--state-error`; the rest `--surface-sunken`. A phase with no agents yet is empty. Equal widths are used because the number of agents in future phases is not known while a run is in flight. |
| "Phase 4 of 6" | index of the latest phase with a running or waiting agent |
| Totals row | `StatTile`s: agents, done, running, waiting, failed (hidden when 0), tokens, tool calls. When the run is finished: plus duration. |
| Summary line | the run's `summary`, one line, expandable; shown under the name when present |

`PhaseColumn` (width 288 px, min 248; the board scrolls sideways when columns do not fit; each column scrolls vertically on its own):

- Header: phase number in a circle, title (display face), `done/total` count, the phase's detail text on a second line (1 line, tooltip for the rest), a model hint chip if the phase declares one, and a 4 px segmented progress bar (segments by state).
- Body: `AgentCard`s, ordered: running, waiting, failed, stopped, done; within a group by start time.
- Column border style from `--container-border-style`. The column of the phase that is currently active has a `--state-running` top edge.

`AgentCard` (workflow variant, 96 px; 56 px in compact form):

```
+--+--------------------------------------------+
|  | Agent label                     (o) 4m 12s |
|  | Task summary on up to two lines            |
|  | [O] Opus 5.5    184k tok    51 tools   x2  |
|  | > Bash  cargo test -p core                 |
+--+--------------------------------------------+
```

It encodes the same things as the full `AgentNode` (it is the same component with `variant="card"`). Compact form drops the summary and the last-tool line. A column switches to compact cards when it holds more than 8 agents. Waiting cards show "Waiting 3m" in place of the timer.

`LogPanel` (bottom drawer, 160 px, collapsible to a 32 px bar showing the count and the newest line), titled **Activity**: the run's activity derived from the snapshot (run start, each agent's start and end with its time, failures and retries, run end), newest at the bottom, mono, wrapped. It sticks to the bottom while the user has not scrolled up. The run file's own log lines and result are not in the data contract (version 1), so the panel says so; a **Log** and **Result** tab can be added when the contract carries them.

Interactions: click a card to select the agent; click a phase header to select the phase; click the RunHeader name to select the run. Arrow keys move between cards and columns. Double-click a card opens the Graph centred on that agent.

Scale:

- 1 agent: one column has one card; the other declared phases are drawn empty so the plan of the run is visible. Columns stretch to share the width, up to 420 px each.
- 10 agents: full cards, no scrolling.
- 60 agents: six columns at 288 px fit a 1920 px window, otherwise the board scrolls sideways; the 18- and 19-agent columns use compact cards (about 11 visible in 700 px) and scroll. A sticky "6 running" marker at the top of a scrolled column jumps to the first running card.

### 3.4 Timeline

Purpose: when things happened, how long they waited and ran, and where the time went. In replay it also holds the playhead.

Layout:

```
+-------------------------+--------------------------------------------------+
| [Fit] [-] [+]  Group: Phase v     | density strip + visible-window brush   |  TimelineToolbar + TimeMinimap (40)
+-------------------------+--------------------------------------------------+
|                         | 00:00    00:30    01:00    01:30    02:00        |  TimeAxis (28)
+-------------------------+--------------------------------------------------+
| (o) Session name        | ' '' '   ''' '  '      '' '                      |  session row: ticks only
| v Run: build  58        | [=============================== ]               |  run row: span bar
|   v 1 Spec  6/6         | [=======]                                        |  phase row: span bar
|     (v) spec:alpha  [O] | ..[######'#'##'###]                              |  agent rows
|     (v) spec:beta   [O] | ....[####'##'#######]                            |
|   v 2 Engine 6/6        |          [===========]                           |
|     (o) engine:alpha [S]|          ...[###'#'####>                         |
+-------------------------+--------------------------------------------------+
  label column (260 px, resizable 180-400)      ^ playhead / now line
```

Rows (height 28 px, 24 px compact; group rows 28 px):

| Row | Label column | Track |
|---|---|---|
| Session | state icon, name | tool-call ticks of the main session |
| Direct agent | indent by depth (16 px per level), state icon, label, model letter | bar + ticks |
| Run | chevron, state icon, workflow name, agent count | a 6 px span bar from run start to end in `--border-strong` |
| Phase | chevron, number, title, `done/total` | a 6 px span bar from the first queue time to the last end time among its agents, in `--accent-soft` with a `--accent` outline; plus a full-height band of `--surface-hover` at 50% behind its agent rows |
| Agent | state icon, label, model letter | bar + ticks |

Agent bar (14 px tall, vertically centred, radius `--radius-xs`):

| Part | Drawn as | Encodes |
|---|---|---|
| Waiting segment | 4 px tall line, dashed, `--state-queued` | `queuedAt` to `startedAt` |
| Running segment | solid `--state-running`; its right end follows `viewTime` | `startedAt` to `viewTime` while running |
| Finished segment | solid `--state-done` at 55% opacity, with a 3 px full-colour cap at the end; failed: `--state-error` cap and `--state-error-soft` body; stopped: `--state-stopped` cap | `startedAt` to end |
| Tool ticks | 1 px vertical lines, full bar height, `--text-primary` at 45%; ticks closer than 2 px merge into one 2 px tick | one tool call each |
| Retry notch | a small triangle above the bar | start of attempt 2 or later |
| Label on the bar | duration, `--text-xs`, shown when the bar is at least 64 px wide | |

`TimeAxis`: tick steps picked from 1 s, 5 s, 15 s, 1 min, 5 min, 15 min, 1 h so labels sit at least 80 px apart. Labels are elapsed time since the session (or recording) start, or clock time, by the Time axis setting. Faint vertical grid lines at ticks.

`TimeMinimap` (40 px): the whole time range. A histogram of tool calls (120 buckets, `--text-muted`), run spans as underlines, and a draggable, resizable brush marking the visible window.

Playhead and now line: a 1 px vertical line across all rows with a flag on the axis. Live: `--state-running`, labelled "Now", pinned 24 px from the right edge while **Follow** (toolbar toggle, default on) is active; any manual pan or zoom turns Follow off. Replay: `--accent`, draggable; dragging it seeks, same as the ReplayBar scrubber.

Interactions:

- `Ctrl/Cmd` + wheel or pinch zooms around the pointer; plain wheel scrolls rows; `Shift` + wheel or horizontal scroll pans time. Zoom range: 100 ms per pixel to 1 min per pixel. **Fit** shows the whole range.
- Click a row or bar to select the agent, run or phase. Click on empty track in replay mode seeks to that time.
- Hover a bar: tooltip with label, state, waited, ran, tokens, tool calls. Hover within 3 px of a tick: tool name, one-line summary, time.
- Drag on the axis to zoom into that range.
- **Group** menu: Phase (default: session, direct agents, then runs with phases), Start time (flat list by start), State (running first).
- Group rows collapse with their chevron; collapsed groups show their span bar only.

Rendering: the track area is SVG. Rows are virtualised (only rows in the viewport plus 6 above and below are in the DOM). All ticks of one row are one `<path>`. Label column and tracks share one scroll container so they stay aligned.

Scale:

- 1 agent: two rows (session, agent); the axis fits its span; minimum visible span is 60 s so a 5 s agent is not a full-width bar.
- 10 agents: all rows visible without scrolling.
- 60 agents: about 70 rows with group rows, about 2,000 px; 25 to 30 visible at once. Finished runs other than the scoped one start collapsed. A nine-hour run fitted to a 1,000 px track is 33 s per pixel; ticks merge into density blocks, and the minimap brush is the main way to zoom in.

### 3.5 Inspector

The panel content depends on `selection.kind`. All variants share: sticky header, sections with a small caps title and a collapse chevron (collapsed state remembered per section name), 16 px padding, one scroll area.

**Agent**

| Section | Content |
|---|---|
| Header | type glyph, label, `StateBadge`, attempt badge. Under it the `ParentChain`: `Session › Run › Phase › Agent`, or `Session › Agent › Agent`. Every part is a button that selects that thing. |
| Task | The full prompt in `--font-ui`, `--text-sm`, line breaks kept, no markdown rendering. Clamped to 12 lines with **Show all** / **Show less**; **Copy** button. If only a preview is available, a note "Preview only" follows it. |
| Details | a two-column `KeyValueList`: State, Type, Model (chip), Started (clock time and relative), Waited, Ran (ticking while running), Phase, Attempt, Worktree (path, only if present), Agent id (mono, copy on click). |
| Counts | two `StatTile`s: Tokens and Tool calls, each with a thin bar showing its share of the largest agent in the same run or session. A third line breaks tool calls down by tool name (top 4, then "other"), as small bars. |
| Files touched | a list, edits first then reads, at most 50 with **Show all**: path (middle-truncated), an `E` or `R` badge, a count if touched more than once, and a chip "+3" when 3 other agents touched the same file. Clicking a row selects the file. Empty: "No file reads or edits seen. Shell commands are not tracked." |
| Recent tool calls | the last 20 as of `viewTime`, newest first: time (elapsed), tool name (mono), one-line summary (truncated). A failed call has an error icon. |
| Result | the result preview text, clamped to 12 lines with **Show all** and **Copy**. While running: "Still running". Failed: the error text on `--state-error-soft`. |
| Started agents | only if it started others: a list of `AgentRow`s (state icon, label, model letter), each selecting that agent. |

**Session**: header (name, state); Details (project path, session id, version, started, last activity, process id and whether it is alive); Counts (agents by state as a segmented bar, runs, tokens, tool calls); Runs (list of `RunRow`: state, name, start, duration, agents); Agents (direct agents, as `AgentRow`s); Activity (the sparkline at 320 x 48 with an axis).

**Run**: header (workflow name, state); Summary; Details (started, duration, default model, agent count); Progress (stepper with one row per phase: number, title, `done/total`, segmented bar; clicking selects the phase); Totals (tokens, tool calls); Activity (last 20 derived lines, as in the LogPanel); Result (the result previews of the run's last agents, when finished). Log lines, the run result and the script are not in the data contract (version 1); the section says so instead of showing them.

**Phase**: header (number, title, state derived from its agents); Details (detail text, declared model, first start, last end); Progress (segmented bar and counts); Agents (`AgentRow`s).

**File**: header (file name, full path under it); Touched by (a list of agents with `E` / `R` badge, first and last touch time, touch count; clicking selects the agent). A note if two or more agents edited it: "Edited by 3 agents".

## 4. Design system

All styling goes through CSS custom properties. Components never contain literal colours, font names, radii, shadows or durations. Tokens come in three layers:

1. **Base tokens** (4.1 to 4.4): the same in every theme.
2. **Theme character tokens** (4.5): radii, border weight, typefaces, shadow style, canvas pattern. Set per theme, independent of mode.
3. **Colour tokens** (4.6, 4.7): set per theme and mode.

File layout: `src/lib/theme/tokens.css` (base), `src/lib/theme/themes/<name>.css` (character plus both colour blocks), `src/lib/theme/fonts.css`. Each theme block also matches a plain `[data-theme]` element so a preview can scope a theme.

### 4.1 Typefaces

Bundled with the app through npm packages, so nothing is fetched at run time. All are under the SIL Open Font License.

| Package | Family | Used for |
|---|---|---|
| `@fontsource-variable/inter` | Inter Variable | UI text in every theme |
| `@fontsource-variable/jetbrains-mono` | JetBrains Mono Variable | paths, tool names, ids, numbers in tables; display face in Blueprint |
| `@fontsource-variable/fraunces` | Fraunces Variable | display face in Paper |
| `@fontsource-variable/space-grotesk` | Space Grotesk Variable | display face in Poster |

Import only the `latin` and `latin-ext` subsets (`.../wght.css`). Fallback stacks:

```css
:root {
  --font-ui: "Inter Variable", system-ui, -apple-system, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif;
  --font-mono: "JetBrains Mono Variable", ui-monospace, "SF Mono", "Cascadia Mono", Consolas, "Liberation Mono", monospace;
  /* --font-display is set by each theme (4.5) */
}
```

### 4.2 Base tokens

```css
:root {
  /* density: 1 comfortable, 0.75 compact (set by settings as data-density on <html>) */
  --density: 1;

  /* spacing, 4 px grid */
  --space-0: 0;
  --space-1: 4px;
  --space-2: 8px;
  --space-3: 12px;
  --space-4: 16px;
  --space-5: 20px;
  --space-6: 24px;
  --space-8: 32px;
  --space-10: 40px;
  --space-12: 48px;
  --space-16: 64px;
  /* component padding that follows density */
  --pad-1: calc(var(--space-1) * var(--density));
  --pad-2: calc(var(--space-2) * var(--density));
  --pad-3: calc(var(--space-3) * var(--density));
  --pad-4: calc(var(--space-4) * var(--density));

  /* type scale: size / line height */
  --text-xs: 11px;   --leading-xs: 16px;
  --text-sm: 12px;   --leading-sm: 18px;
  --text-md: 13px;   --leading-md: 20px;   /* body default */
  --text-lg: 15px;   --leading-lg: 22px;
  --text-xl: 18px;   --leading-xl: 26px;
  --text-2xl: 24px;  --leading-2xl: 32px;
  --text-3xl: 32px;  --leading-3xl: 40px;
  --weight-regular: 400;
  --weight-medium: 500;
  --weight-semibold: 600;
  --weight-bold: 700;

  /* fixed sizes */
  --size-navrail: 64px;
  --size-topbar: 48px;
  --size-statusbar: 26px;
  --size-replaybar: 64px;
  --size-inspector: 360px;
  --size-row: calc(28px * (0.4 + 0.6 * var(--density)));  /* 28 or about 24 */
  --size-control: 28px;
  --size-icon: 16px;

  /* motion */
  --dur-1: 120ms;   /* hover, press, small fades */
  --dur-2: 200ms;   /* panels, popovers, state colour change */
  --dur-3: 320ms;   /* node moves after layout, drawer open */
  --dur-4: 600ms;   /* one-shot pulses */
  --dur-loop: 1600ms; /* one cycle of any looping animation */
  --ease-out: cubic-bezier(0.2, 0.8, 0.2, 1);
  --ease-in-out: cubic-bezier(0.45, 0, 0.25, 1);
  --ease-in: cubic-bezier(0.5, 0, 1, 0.6);
  --ease-linear: linear;

  /* layers */
  --z-canvas: 0;
  --z-panel: 10;
  --z-sticky: 20;
  --z-overlay: 30;   /* inspector when overlaying */
  --z-popover: 40;
  --z-dialog: 50;
  --z-toast: 60;
}
:root[data-density="compact"] { --density: 0.75; }
```

### 4.3 Type roles

| Role | Face | Size | Weight | Used for |
|---|---|---|---|---|
| display | `--font-display` | `--text-2xl` | `--display-weight` | view titles, empty-state titles |
| title | `--font-display` | `--text-lg` | `--display-weight` | card titles, run and phase names, Inspector header |
| body | `--font-ui` | `--text-md` | 400 | default text |
| body-strong | `--font-ui` | `--text-md` | 600 | node labels, list primaries |
| small | `--font-ui` | `--text-sm` | 400 | summaries, secondary lines |
| label | `--font-ui` (Blueprint: `--font-mono`) | `--text-xs` | 600 | section titles, chip text; uses `--label-transform` and `--label-tracking` |
| mono | `--font-mono` | `--text-sm` | 400 | paths, tool lines, ids, log |
| number | `--font-ui`, tabular figures | as context | 500 | counts, timers |

Smallest text anywhere is 11 px.

### 4.4 Elevation

Four levels, each theme defines the actual shadow values (4.5):

| Token | Used for |
|---|---|
| `--shadow-0` | flat things on the canvas; always `none` |
| `--shadow-1` | cards, nodes at rest |
| `--shadow-2` | hovered cards and nodes, sticky headers |
| `--shadow-3` | popovers, menus, dialogs, overlaying Inspector |

### 4.5 Themes: character

Four themes. They differ in shape, line, type and depth, not only in colour.

| | Graphite | Paper | Blueprint | Poster |
|---|---|---|---|---|
| Idea | a precise instrument. Monochrome chrome so that state colours are the only colour on screen. | a printed report. Warm paper, ink text, serif titles, hairlines, no shadows. | a technical drawing. Grid canvas, square corners, dashed group outlines, mono labels in capitals. | a bold poster. Thick black outlines, hard offset shadows, big round corners, one loud yellow. |
| Accent | ink (near black, near white in dark) | petrol | drafting blue (light), cyan (dark) | yellow with black text |
| Corners | medium | nearly square | square | very round |
| Lines | 1 px solid | 1 px solid | 1 px, group outlines dashed | 2 px solid, high contrast |
| Depth | soft blurred shadows | none: borders only | none: a 1 px outer line | hard offset shadow, no blur |
| Canvas | dot grid | plain | line grid with major lines | plain |
| Titles | Inter 600 | Fraunces 600 | JetBrains Mono 600, capitals for labels | Space Grotesk 700 |
| Dark mode feels like | graphite slab | ink on dark kraft | classic blueprint, navy | stage lights on black |

Character tokens (mode-independent). `--shadow-color` comes from the colour block of the active mode.

```css
:root[data-theme="graphite"] {
  --font-display: var(--font-ui);
  --display-weight: 600;
  --display-tracking: -0.01em;
  --label-transform: none;
  --label-tracking: 0.01em;
  --radius-xs: 3px;
  --radius-sm: 5px;
  --radius-md: 8px;
  --radius-lg: 12px;
  --radius-pill: 999px;
  --border-width: 1px;
  --border-width-strong: 1.5px;
  --container-border-style: solid;
  --shadow-1: 0 1px 2px color-mix(in srgb, var(--shadow-color) 8%, transparent);
  --shadow-2: 0 4px 12px color-mix(in srgb, var(--shadow-color) 12%, transparent), 0 1px 2px color-mix(in srgb, var(--shadow-color) 8%, transparent);
  --shadow-3: 0 12px 32px color-mix(in srgb, var(--shadow-color) 20%, transparent), 0 2px 6px color-mix(in srgb, var(--shadow-color) 10%, transparent);
  --canvas-pattern: radial-gradient(circle, var(--border-default) 1px, transparent 1.5px);
  --canvas-pattern-size: 20px 20px;
  --node-fill: var(--surface-card);
  --edge-width: 1.5px;
}
:root[data-theme="paper"] {
  --font-display: "Fraunces Variable", Georgia, "Times New Roman", serif;
  --display-weight: 600;
  --display-tracking: -0.005em;
  --label-transform: none;
  --label-tracking: 0.02em;
  --radius-xs: 1px;
  --radius-sm: 2px;
  --radius-md: 3px;
  --radius-lg: 4px;
  --radius-pill: 999px;
  --border-width: 1px;
  --border-width-strong: 1.5px;
  --container-border-style: solid;
  --shadow-1: none;
  --shadow-2: 0 0 0 1px var(--border-strong);
  --shadow-3: 0 0 0 1px var(--border-strong), 0 8px 24px color-mix(in srgb, var(--shadow-color) 14%, transparent);
  --canvas-pattern: none;
  --canvas-pattern-size: auto;
  --node-fill: var(--surface-card);
  --edge-width: 1px;
}
:root[data-theme="blueprint"] {
  --font-display: var(--font-mono);
  --display-weight: 600;
  --display-tracking: 0;
  --label-transform: uppercase;
  --label-tracking: 0.08em;
  --radius-xs: 0px;
  --radius-sm: 0px;
  --radius-md: 0px;
  --radius-lg: 2px;
  --radius-pill: 2px;
  --border-width: 1px;
  --border-width-strong: 1.5px;
  --container-border-style: dashed;
  --shadow-1: none;
  --shadow-2: 0 0 0 1px var(--accent);
  --shadow-3: 0 0 0 1px var(--border-strong), 0 0 0 4px var(--surface-app);
  --canvas-pattern:
    linear-gradient(var(--border-default) 1px, transparent 1px),
    linear-gradient(90deg, var(--border-default) 1px, transparent 1px),
    linear-gradient(var(--border-subtle) 1px, transparent 1px),
    linear-gradient(90deg, var(--border-subtle) 1px, transparent 1px);
  --canvas-pattern-size: 120px 120px, 120px 120px, 24px 24px, 24px 24px;
  --node-fill: var(--surface-card);
  --edge-width: 1px;
}
:root[data-theme="poster"] {
  --font-display: "Space Grotesk Variable", var(--font-ui);
  --display-weight: 700;
  --display-tracking: -0.02em;
  --label-transform: uppercase;
  --label-tracking: 0.04em;
  --radius-xs: 4px;
  --radius-sm: 8px;
  --radius-md: 14px;
  --radius-lg: 20px;
  --radius-pill: 999px;
  --border-width: 2px;
  --border-width-strong: 3px;
  --container-border-style: solid;
  --shadow-1: 3px 3px 0 var(--shadow-color);
  --shadow-2: 5px 5px 0 var(--shadow-color);
  --shadow-3: 8px 8px 0 var(--shadow-color);
  --canvas-pattern: none;
  --canvas-pattern-size: auto;
  --node-fill: var(--surface-card);
  --edge-width: 2px;
}
```

Notes for the builder:

- `color-mix()` is used only in Graphite and Paper shadows. If a target webview lacks it, the declaration is dropped and the theme falls back to no shadow, which is acceptable; add `@supports not (color: color-mix(in srgb, red, blue))` fallbacks with fixed `rgba()` values if testing shows this on Linux.
- The canvas pattern is applied as `background-image: var(--canvas-pattern); background-size: var(--canvas-pattern-size)` on the Graph background (Svelte Flow's own `Background` component is not used) and must move and scale with the viewport transform; if that costs frames at 60 nodes, keep it fixed. Version 1 keeps it fixed.
- In Poster, hover on a card moves it by `translate(-1px, -1px)` and grows the shadow from `--shadow-1` to `--shadow-2`; press moves it by `translate(2px, 2px)` with `--shadow-0`.
- In Blueprint, labels using the label role are capitals; agent labels and task text are never transformed.

### 4.6 Colour tokens: meaning

| Token | Meaning |
|---|---|
| `--surface-app` | window background |
| `--surface-canvas` | Graph and Timeline drawing area |
| `--surface-panel` | NavRail, TopBar, StatusBar, Inspector, column bodies |
| `--surface-card` | cards, nodes, list rows |
| `--surface-raised` | menus, popovers, dialogs, tooltips |
| `--surface-sunken` | inputs, code and log blocks, empty progress track |
| `--surface-hover` | hover background on any of the above |
| `--surface-selected` | selected row background |
| `--text-primary` | main text. At least 7:1 on every surface |
| `--text-secondary` | supporting text. At least 4.5:1 on every surface |
| `--text-muted` | hints, axis labels, disabled-looking but readable text. At least 4.5:1 on every surface |
| `--text-on-accent` | text on an `--accent` fill |
| `--border-subtle` | panel separators, grid lines |
| `--border-default` | card and input borders |
| `--border-strong` | edges in the graph, emphasised borders. At least 3:1 on main surfaces |
| `--accent` | primary button fill, current nav item bar, playhead |
| `--accent-hover` | hover of the above |
| `--accent-soft` | tinted background for the current nav item, phase span bars |
| `--accent-text` | links and accent-coloured text and icons. At least 4.5:1 on main surfaces and on `--accent-soft` |
| `--focus-ring` | keyboard focus ring. At least 3:1 on main surfaces |
| `--select-ring` | ring around the selected node, card or row |
| `--shadow-color` | base colour for the theme's shadows |
| `--state-<s>` | fill for state icons, bars, dots, borders. At least 3:1 on app, canvas, panel, card and raised surfaces |
| `--state-<s>-text` | state-coloured text. At least 4.5:1 on every surface and on its own soft colour |
| `--state-<s>-soft` | tinted background for badges and failed nodes. `--text-primary` on it stays above 7:1 |
| `--model-<m>` | model chip square (`opus`, `sonnet`, `haiku`, `fable`, `other`). At least 3:1 on main surfaces. The letter inside uses `--surface-card` |
| `--type-<t>` | type glyph tint (`session`, `workflow`, `general`, `fork`, `explore`, `plan`, `custom`). At least 3:1 on main surfaces |

`<s>` is one of `queued`, `running`, `done`, `error`, `stopped`, `idle`.

Rules of use:

- State colours appear only on state marks (icon, bar, dot, node border, badge, timeline bar). They are never used for buttons or links.
- Accent appears only on controls, the current nav item, the playhead and shared-file marks. It is never used inside a state mark.
- Selection always uses `--select-ring`, so a selected running node is still clearly selected.
- A `StateBadge` is `--state-<s>-soft` background, `--state-<s>-text` text, and the state icon in `--state-<s>`.
- Text is never placed on a solid `--state-<s>`, `--model-<m>` or `--type-<t>` fill, except the single model letter (bold, 10 px, decorative: the model name always follows in text).

### 4.7 Colour tokens: values

Every block below was generated and checked by script against the rules in 4.6 (WCAG 2.1 contrast). All checks pass. The comment on each block names the two state colours that are closest to each other and their distance in OKLab; every pair in every theme is at least 0.095 apart, and every state also has its own icon shape.

State hues keep their meaning across themes: waiting is amber, running is blue, done is green, failed is red, stopped is magenta-plum, idle is grey.

```css
/* Graphite, light. Closest pair of state colours: queued / error, OKLab distance 0.121 */
:root[data-theme="graphite"][data-mode="light"] {
  color-scheme: light;
  --surface-app: #F3F4F6;
  --surface-canvas: #ECEEF1;
  --surface-panel: #FAFAFB;
  --surface-card: #FFFFFF;
  --surface-raised: #FFFFFF;
  --surface-sunken: #E9EBEF;
  --surface-hover: #EEF0F3;
  --surface-selected: #E3E6EB;
  --text-primary: #14171C;
  --text-secondary: #4A5160;
  --text-muted: #5F6776;
  --border-subtle: #E2E5EA;
  --border-default: #CFD4DC;
  --border-strong: #7F8796;
  --accent: #14171C;
  --accent-hover: #2A2F38;
  --accent-soft: #E3E6EB;
  --accent-text: #14171C;
  --text-on-accent: #FFFFFF;
  --focus-ring: #2563EB;
  --select-ring: #14171C;
  --shadow-color: #14171C;
  --state-queued: #B2790C;
  --state-queued-text: #8B5D02;
  --state-queued-soft: #F4ECDD;
  --state-running: #3986E4;
  --state-running-text: #1166C0;
  --state-running-soft: #E3EEFB;
  --state-done: #1A9D4B;
  --state-done-text: #017634;
  --state-done-soft: #DFF1E6;
  --state-error: #D5584F;
  --state-error-text: #B63B35;
  --state-error-soft: #F9E8E6;
  --state-stopped: #C35AA4;
  --state-stopped-text: #A6408A;
  --state-stopped-soft: #F7E8F2;
  --state-idle: #7F8793;
  --state-idle-text: #5F6772;
  --state-idle-soft: #EDEEF0;
  --model-opus: #C66847;
  --model-sonnet: #6083D3;
  --model-haiku: #03998D;
  --model-fable: #928A0F;
  --model-other: #81868F;
  --type-session: #14171C;
  --type-workflow: #2C93B5;
  --type-general: #BC6B72;
  --type-fork: #9A8533;
  --type-explore: #3B9875;
  --type-plan: #A271AD;
  --type-custom: #B67445;
}
```

```css
/* Graphite, dark. Closest pair of state colours: running / idle, OKLab distance 0.115 */
:root[data-theme="graphite"][data-mode="dark"] {
  color-scheme: dark;
  --surface-app: #0E1013;
  --surface-canvas: #0A0B0D;
  --surface-panel: #14171B;
  --surface-card: #1A1D22;
  --surface-raised: #22262D;
  --surface-sunken: #0A0B0D;
  --surface-hover: #23272E;
  --surface-selected: #2C313A;
  --text-primary: #ECEEF2;
  --text-secondary: #A9B0BD;
  --text-muted: #969EAC;
  --border-subtle: #23272E;
  --border-default: #323843;
  --border-strong: #6A7382;
  --accent: #ECEEF2;
  --accent-hover: #FFFFFF;
  --accent-soft: #2C313A;
  --accent-text: #ECEEF2;
  --text-on-accent: #0E1013;
  --focus-ring: #6EA8FF;
  --select-ring: #ECEEF2;
  --shadow-color: #000000;
  --state-queued: #E19B1B;
  --state-queued-text: #E19B1B;
  --state-queued-soft: #423621;
  --state-running: #6EADFE;
  --state-running-text: #6EADFE;
  --state-running-soft: #2B3A4E;
  --state-done: #5AC576;
  --state-done-text: #5AC576;
  --state-done-soft: #273F33;
  --state-error: #FB8276;
  --state-error-text: #FB8276;
  --state-error-soft: #473133;
  --state-stopped: #E882C8;
  --state-stopped-text: #E882C8;
  --state-stopped-soft: #433143;
  --state-idle: #A4ABB8;
  --state-idle-text: #A4ABB8;
  --state-idle-soft: #363940;
  --model-opus: #EB9070;
  --model-sonnet: #86A9F7;
  --model-haiku: #31C3B5;
  --model-fable: #B6AF4D;
  --model-other: #A5ABB5;
  --type-session: #ECEEF2;
  --type-workflow: #5FB8D9;
  --type-general: #E19197;
  --type-fork: #BEAA60;
  --type-explore: #6BBF9B;
  --type-plan: #C697D1;
  --type-custom: #DB9A6E;
}
```

```css
/* Paper, light. Closest pair of state colours: queued / idle, OKLab distance 0.101 */
:root[data-theme="paper"][data-mode="light"] {
  color-scheme: light;
  --surface-app: #F4EFE6;
  --surface-canvas: #EFE9DD;
  --surface-panel: #F8F4EC;
  --surface-card: #FCFAF5;
  --surface-raised: #FFFDF8;
  --surface-sunken: #EAE3D5;
  --surface-hover: #F1EADD;
  --surface-selected: #E6DDCB;
  --text-primary: #221E18;
  --text-secondary: #554D41;
  --text-muted: #675E50;
  --border-subtle: #E2D9C8;
  --border-default: #CFC4AF;
  --border-strong: #8C7F68;
  --accent: #0F5E66;
  --accent-hover: #0B4A51;
  --accent-soft: #D5E5E3;
  --accent-text: #0F5E66;
  --text-on-accent: #FFFFFF;
  --focus-ring: #0F5E66;
  --select-ring: #221E18;
  --shadow-color: #3B2F1E;
  --state-queued: #A17000;
  --state-queued-text: #825B09;
  --state-queued-soft: #EFE7D3;
  --state-running: #2B80BF;
  --state-running-text: #0665A0;
  --state-running-soft: #DFE9ED;
  --state-done: #338F54;
  --state-done-text: #037138;
  --state-done-soft: #E0EBDE;
  --state-error: #B95858;
  --state-error-text: #A14445;
  --state-error-soft: #F3E3DF;
  --state-stopped: #9B60A8;
  --state-stopped-text: #854B92;
  --state-stopped-soft: #EEE4EA;
  --state-idle: #81796D;
  --state-idle-text: #686155;
  --state-idle-soft: #EBE8E2;
  --model-opus: #AC654B;
  --model-sonnet: #5D79B6;
  --model-haiku: #0D8D82;
  --model-fable: #837D2F;
  --model-other: #7F7971;
  --type-session: #221E18;
  --type-workflow: #3D849F;
  --type-general: #A5666B;
  --type-fork: #897A3F;
  --type-explore: #478A6E;
  --type-plan: #906A98;
  --type-custom: #A06D4A;
}
```

```css
/* Paper, dark. Closest pair of state colours: queued / idle, OKLab distance 0.095 */
:root[data-theme="paper"][data-mode="dark"] {
  color-scheme: dark;
  --surface-app: #1A1714;
  --surface-canvas: #151310;
  --surface-panel: #201C18;
  --surface-card: #26221D;
  --surface-raised: #2E2923;
  --surface-sunken: #151310;
  --surface-hover: #2E2923;
  --surface-selected: #3A342C;
  --text-primary: #F0E9DC;
  --text-secondary: #BDB3A2;
  --text-muted: #A89E8D;
  --border-subtle: #2E2923;
  --border-default: #40392F;
  --border-strong: #877C6A;
  --accent: #6FCBD1;
  --accent-hover: #8ED9DE;
  --accent-soft: #1F3A3C;
  --accent-text: #6FCBD1;
  --text-on-accent: #10201F;
  --focus-ring: #6FCBD1;
  --select-ring: #F0E9DC;
  --shadow-color: #000000;
  --state-queued: #D9A856;
  --state-queued-text: #D9A856;
  --state-queued-soft: #4A3D28;
  --state-running: #6FB8F5;
  --state-running-text: #6FB8F5;
  --state-running-soft: #354048;
  --state-done: #76C68D;
  --state-done-text: #76C68D;
  --state-done-soft: #364333;
  --state-error: #F19390;
  --state-error-text: #F19390;
  --state-error-soft: #4F3934;
  --state-stopped: #D299DF;
  --state-stopped-text: #D299DF;
  --state-stopped-soft: #483A44;
  --state-idle: #B8B0A3;
  --state-idle-text: #B8B0A3;
  --state-idle-soft: #433E38;
  --model-opus: #E49D85;
  --model-sonnet: #95B0EC;
  --model-haiku: #66C4B9;
  --model-fable: #B9B56E;
  --model-other: #B6B0A7;
  --type-session: #F0E9DC;
  --type-workflow: #7CBCD5;
  --type-general: #DC9EA2;
  --type-fork: #C0B17A;
  --type-explore: #83C1A5;
  --type-plan: #C7A2CF;
  --type-custom: #D7A584;
}
```

```css
/* Blueprint, light. Closest pair of state colours: queued / idle, OKLab distance 0.104 */
:root[data-theme="blueprint"][data-mode="light"] {
  color-scheme: light;
  --surface-app: #EEF3FA;
  --surface-canvas: #E6EDF7;
  --surface-panel: #F5F8FC;
  --surface-card: #FFFFFF;
  --surface-raised: #FFFFFF;
  --surface-sunken: #DFE8F4;
  --surface-hover: #E9F0F9;
  --surface-selected: #D6E3F5;
  --text-primary: #0B2447;
  --text-secondary: #3A5278;
  --text-muted: #506588;
  --border-subtle: #D3DFEF;
  --border-default: #B4C7E2;
  --border-strong: #6381AE;
  --accent: #0B4FB3;
  --accent-hover: #093F90;
  --accent-soft: #D6E3F5;
  --accent-text: #0B4FB3;
  --text-on-accent: #FFFFFF;
  --focus-ring: #0B4FB3;
  --select-ring: #0B2447;
  --shadow-color: #0B2447;
  --state-queued: #A17904;
  --state-queued-text: #7F5F04;
  --state-queued-soft: #F2ECDC;
  --state-running: #0090A8;
  --state-running-text: #066E80;
  --state-running-soft: #DBEFF3;
  --state-done: #0B9958;
  --state-done-text: #097341;
  --state-done-soft: #DDF1E8;
  --state-error: #CA5556;
  --state-error-text: #B03D41;
  --state-error-soft: #F8E7E7;
  --state-stopped: #B559A3;
  --state-stopped-text: #9C428C;
  --state-stopped-soft: #F5E8F2;
  --state-idle: #8A7E75;
  --state-idle-text: #6E6259;
  --state-idle-soft: #EFEDEC;
  --model-opus: #BB6546;
  --model-sonnet: #5D7DC8;
  --model-haiku: #0B9488;
  --model-fable: #8B8318;
  --model-other: #877E77;
  --type-session: #0B2447;
  --type-workflow: #2F8CAC;
  --type-general: #B3676D;
  --type-fork: #927F34;
  --type-explore: #3D9372;
  --type-plan: #9A6DA4;
  --type-custom: #AD7044;
}
```

```css
/* Blueprint, dark. Closest pair of state colours: error / stopped, OKLab distance 0.111 */
:root[data-theme="blueprint"][data-mode="dark"] {
  color-scheme: dark;
  --surface-app: #0B1B33;
  --surface-canvas: #081528;
  --surface-panel: #0F2340;
  --surface-card: #132A4C;
  --surface-raised: #19345C;
  --surface-sunken: #071223;
  --surface-hover: #19345C;
  --surface-selected: #214272;
  --text-primary: #E6F0FF;
  --text-secondary: #A9C0E0;
  --text-muted: #9AB3D5;
  --border-subtle: #1B3660;
  --border-default: #2B4C80;
  --border-strong: #6A8FC8;
  --accent: #4FD8F0;
  --accent-hover: #7FE4F5;
  --accent-soft: #12405A;
  --accent-text: #4FD8F0;
  --text-on-accent: #06202B;
  --focus-ring: #4FD8F0;
  --select-ring: #E6F0FF;
  --shadow-color: #000000;
  --state-queued: #EAB532;
  --state-queued-text: #EAB532;
  --state-queued-soft: #424946;
  --state-running: #1BD3F5;
  --state-running-text: #1BD3F5;
  --state-running-soft: #154F71;
  --state-done: #61DA92;
  --state-done-text: #61DA92;
  --state-done-soft: #24515B;
  --state-error: #FFA09C;
  --state-error-text: #FFA09C;
  --state-error-soft: #47445E;
  --state-stopped: #F897E4;
  --state-stopped-text: #F897E4;
  --state-stopped-soft: #45426D;
  --state-idle: #B5BFCB;
  --state-idle-text: #B5BFCB;
  --state-idle-soft: #374B68;
  --model-opus: #FEA484;
  --model-sonnet: #9EBDFF;
  --model-haiku: #4CD7C8;
  --model-fable: #C9C261;
  --model-other: #B7BEC7;
  --type-session: #E6F0FF;
  --type-workflow: #73CBED;
  --type-general: #F6A4A9;
  --type-fork: #D1BD73;
  --type-explore: #7ED2AE;
  --type-plan: #DAAAE5;
  --type-custom: #EFAD80;
}
```

```css
/* Poster, light. Closest pair of state colours: queued / idle, OKLab distance 0.104 */
:root[data-theme="poster"][data-mode="light"] {
  color-scheme: light;
  --surface-app: #FFFBF0;
  --surface-canvas: #FFF6DD;
  --surface-panel: #FFFFFF;
  --surface-card: #FFFFFF;
  --surface-raised: #FFFFFF;
  --surface-sunken: #F3ECD8;
  --surface-hover: #FFF1C2;
  --surface-selected: #FFE680;
  --text-primary: #111111;
  --text-secondary: #3D3D3D;
  --text-muted: #595959;
  --border-subtle: #D9D2BE;
  --border-default: #111111;
  --border-strong: #111111;
  --accent: #FFD60A;
  --accent-hover: #FFC400;
  --accent-soft: #FFF1B8;
  --accent-text: #111111;
  --text-on-accent: #111111;
  --focus-ring: #1B4DFF;
  --select-ring: #111111;
  --shadow-color: #111111;
  --state-queued: #A77601;
  --state-queued-text: #855E05;
  --state-queued-soft: #EDE4CC;
  --state-running: #2377FE;
  --state-running-text: #035DDC;
  --state-running-soft: #D3E4FF;
  --state-done: #069A3E;
  --state-done-text: #09762E;
  --state-done-soft: #CDEBD8;
  --state-error: #EE1118;
  --state-error-text: #C1010D;
  --state-error-soft: #FCCFD1;
  --state-stopped: #CF2BB7;
  --state-stopped-text: #B500A0;
  --state-stopped-soft: #F5D5F1;
  --state-idle: #858073;
  --state-idle-text: #6C675B;
  --state-idle-soft: #E7E6E3;
  --model-opus: #D74B01;
  --model-sonnet: #4676F1;
  --model-haiku: #0B9488;
  --model-fable: #8C830D;
  --model-other: #848076;
  --type-session: #111111;
  --type-workflow: #098DB2;
  --type-general: #CC5162;
  --type-fork: #977E0D;
  --type-explore: #0B976D;
  --type-plan: #A85DB9;
  --type-custom: #C16302;
}
```

```css
/* Poster, dark. Closest pair of state colours: error / idle, OKLab distance 0.104 */
:root[data-theme="poster"][data-mode="dark"] {
  color-scheme: dark;
  --surface-app: #121212;
  --surface-canvas: #0A0A0A;
  --surface-panel: #1A1A1A;
  --surface-card: #1F1F1F;
  --surface-raised: #262626;
  --surface-sunken: #0A0A0A;
  --surface-hover: #2B2B2B;
  --surface-selected: #3A3200;
  --text-primary: #FAFAFA;
  --text-secondary: #C9C9C9;
  --text-muted: #A6A6A6;
  --border-subtle: #333333;
  --border-default: #FAFAFA;
  --border-strong: #FAFAFA;
  --accent: #FFD60A;
  --accent-hover: #FFE04D;
  --accent-soft: #3A3200;
  --accent-text: #FFD60A;
  --text-on-accent: #111111;
  --focus-ring: #7DA2FF;
  --select-ring: #FAFAFA;
  --shadow-color: #000000;
  --state-queued: #F4B00C;
  --state-queued-text: #F4B00C;
  --state-queued-soft: #52421A;
  --state-running: #98BFFE;
  --state-running-text: #98BFFE;
  --state-running-soft: #3C4555;
  --state-done: #2BE263;
  --state-done-text: #2BE263;
  --state-done-soft: #224E2F;
  --state-error: #FEA195;
  --state-error-text: #FEA195;
  --state-error-soft: #553E3B;
  --state-stopped: #FE91E8;
  --state-stopped-text: #FE91E8;
  --state-stopped-soft: #553A4F;
  --state-idle: #C3BDB0;
  --state-idle-text: #C3BDB0;
  --state-idle-soft: #464542;
  --model-opus: #FEA484;
  --model-sonnet: #9EBDFF;
  --model-haiku: #0BDAC9;
  --model-fable: #CFC20C;
  --model-other: #C1BDB3;
  --type-session: #FAFAFA;
  --type-workflow: #40CFFF;
  --type-general: #FF9EA6;
  --type-fork: #DBBC3F;
  --type-explore: #4CDAA7;
  --type-plan: #E69EF7;
  --type-custom: #FEA667;
}
```

If a colour value is changed later, re-check it against 4.6 before merging. A unit test in the UI package should parse the theme CSS files and assert the contrast rules, so themes cannot drift below AA.

## 5. Motion

### 5.1 What moves

| What | How | Duration, easing | Loop |
|---|---|---|---|
| Running state icon | the arc rotates (`transform: rotate`) | `--dur-loop`, linear | yes |
| Running node (Graph) | a 2 px outline in `--state-running` breathes between 35% and 100% opacity (a pseudo-element; `opacity` only) | `--dur-loop`, `--ease-in-out`, alternate | yes |
| Edge into a running agent, run edge of a running run | dashes flow toward the child (`stroke-dashoffset` on a `6 6` dash) | `--dur-loop`, linear | yes |
| Tool call arrives | a ring expands from the node or card's state icon, scale 1 to 2.2, opacity 0.6 to 0 | `--dur-4`, `--ease-out` | no |
| State change | bar, border and icon colour cross-fade; on done, the check draws in (`stroke-dashoffset`) | `--dur-2`, `--ease-out` | no |
| Agent appears | fade in and scale from 0.92 | `--dur-2`, `--ease-out` | no |
| Layout change (Graph) | nodes glide to their new position (`transform`) | `--dur-3`, `--ease-in-out` | no |
| Running share of a progress bar | a light sheen sweeps across (a translated gradient overlay) | `--dur-loop` x 1.5, linear | yes |
| Running bar in Timeline | its right end grows with `viewTime`; updated once per second in live, per frame while replay is playing | none (direct) | |
| Count and token numbers | change in place, no rolling digits | none | |
| Hover, press | background and shadow change; in Poster the offset shift | `--dur-1`, `--ease-out` | no |
| Inspector, log drawer, popovers, dialogs | slide 8 px and fade | `--dur-2`, `--ease-out` (close: `--dur-1`, `--ease-in`) | no |
| Card order change in a column | cards move to their new slot (FLIP, `transform`) | `--dur-3`, `--ease-in-out` | no |
| StatusBar live dot | opacity 1 to 0.4 | `--dur-loop` x 2, alternate | yes |
| Theme or mode change | colour cross-fade | `--dur-2` | no |

Nothing else moves. In particular: no parallax, no bouncing, no animated gradients on backgrounds, no motion on idle, waiting or finished things.

### 5.2 Reduced motion

Reduced motion is on when the Motion setting is Reduced, or it is System and `prefers-reduced-motion: reduce` matches. The app then sets `data-motion="reduced"` on `<html>`, and:

- All loops stop. The running icon is drawn as a static three-quarter ring; the breathing outline stays at full opacity; edges into running agents are drawn dashed but still; no sheen.
- One-shot pulses and the check draw-in are removed. A tool call is still visible through the changing last-tool text and counts.
- Appear, layout glide and card reordering happen without transition (instant).
- Panels and popovers fade only (`--dur-1`), no slide.
- Replay still advances; that is content, not decoration.

Implementation: every animation and transition is declared with the duration tokens, and one block switches them off:

```css
:root[data-motion="reduced"] {
  --dur-1: 0ms; --dur-2: 0ms; --dur-3: 0ms; --dur-4: 0ms;
}
:root[data-motion="reduced"] .loops { animation: none !important; }
```

Every looping element carries the class `loops`. Fades of panels use a separate fixed 120 ms opacity transition that is kept.

### 5.3 Staying cheap at 60 live nodes

1. Animate only `transform` and `opacity` (and `stroke-dashoffset` on at most 12 edges). Never animate `box-shadow`, `filter`, `width`, `height`, `top` or `left`.
2. Only running things loop. Real runs peak at about 8 running agents. A hard cap backs this up: at most 12 nodes and 12 edges loop at once; beyond that the most recently active keep moving and the rest show the static running style.
3. No looping animation runs outside the viewport: an `IntersectionObserver` per view root toggles the `loops` class on entering and leaving elements. At dot level of detail, loops are off; the dot only cross-fades colour.
4. Loops pause when the window is hidden or not focused for more than 30 s (`document.visibilityState`), by setting `animation-play-state: paused` on the view root.
5. Tool-call pulses are rate limited: at most one pulse per node per 800 ms, and none while replay speed is above 10x or while the scrubber is being dragged.
6. Events from the core are batched: the UI applies incoming events to its stores once per animation frame, and at most 10 times per second while replay speed is above 10x.
7. Graph layout only on structure changes, debounced 300 ms, in a worker (3.2). Timeline rows are virtualised and ticks are one path per row (3.4).
8. No `backdrop-filter` anywhere. `will-change: transform` only on nodes while they glide.
9. Budget: with 60 agents, 8 running, the Graph should hold 60 frames per second on a 2020 laptop, and a single event batch should take under 4 ms of script time. The views developer checks this with the 60-agent fixture.

## 6. Components

Paths are under `src/lib/`. Props are TypeScript; `Agent`, `Session`, `Run`, `Phase`, `ToolCall`, `FileTouch` are the snapshot types from the data contract. Types named `*State`, `ModelFamily`, `AgentKind` are the small unions from section 1.

The work splits in two. **Foundation** must land the stores and the shared components' props first (the files can render rough versions), so **Views** is not blocked.

### 6.1 Foundation (developer A)

Design files:

| File | Content |
|---|---|
| `theme/tokens.css` | 4.2 |
| `theme/fonts.css` | 4.1 imports |
| `theme/themes/graphite.css`, `paper.css`, `blueprint.css`, `poster.css` | 4.5 and 4.7 |
| `theme/base.css` | reset, body type, focus ring, scrollbars, `.loops`, reduced-motion block |
| `theme/contrast.test.ts` | parses theme files and asserts the rules of 4.6 |

Stores (`stores/`):

| Store | Shape | Notes |
|---|---|---|
| `settings` | `{ theme, mode, density, motion, timeAxis, claudeDir, inspectorWidth, recentRecordings }` | persists to `localStorage`; applies `data-*` attributes |
| `selection` | `Selection \| null` with `select(sel)`, `clear()` | |
| `hover` | `Selection \| null` | |
| `scope` | `{ sessionId, runId }` with `openSession(id)`, `openRun(id)` | |
| `clock` | `{ mode, viewTime, playing, speed, range, skipGaps }` with `play()`, `pause()`, `seek(t)`, `setSpeed(x)`, `step(±1)` | forwards to the core's replay commands |
| `route` | `'overview' \| 'graph' \| 'workflow' \| 'timeline'` | |
| `search` | `{ text, matches: Set<string> }` | match set computed once, read by all views |
| `world` | sessions, agents, runs, phases, tool calls, file touches as of `viewTime`, plus derived maps: `childrenOf`, `agentsByPhase`, `fileTouchers`, `sharedPairs` | fed by the event stream; batched per frame |

Frame (`frame/`):

| Component | Props | In |
|---|---|---|
| `AppFrame` | none (reads stores); slots for the view and inspector | whole app |
| `NavRail` | `current: Route`, `disabled: Route[]`, `onnavigate` | frame |
| `TopBar` | none | frame |
| `ScopeCrumbs` | `sessions: Session[]`, `runs: Run[]`, `scope` | TopBar |
| `SearchField` | `value`, `count?: {shown, total}`, `placeholder` | TopBar |
| `ModeSwitch` | `mode: 'live' \| 'replay'`, `recordingName?: string` | TopBar |
| `StatusBar` | `source`, `busySessions`, `runningAgents`, `lastEventAt`, `problems: Problem[]` | frame |
| `ProblemsPopover` | `problems: Problem[]` | StatusBar |
| `InspectorFrame` | `open`, `width`, `title`, `onclose`, `onresize`; default slot | frame |
| `ReplayBar` | `clock`, `density: number[]`, `markers: {t, label, kind}[]`, `gaps: [number, number][]` | frame (replay) |
| `Scrubber` | `range`, `value`, `density`, `markers`, `gaps`, `onseek` | ReplayBar |
| `SettingsDialog` | `open`, `onclose` | frame |
| `ThemeTile` | `theme`, `mode`, `selected` | SettingsDialog |
| `RecordingPicker` | `open`, `recent: RecordingRef[]`, `onopen(path)`, `error?` | frame |
| `ShortcutSheet` | `open` | frame |

Shared components (`components/`), used by both developers:

| Component | Props | Notes |
|---|---|---|
| `Button` | `variant: 'primary' \| 'secondary' \| 'ghost'`, `size: 'sm' \| 'md'`, `icon?`, `disabled?` | |
| `IconButton` | `icon`, `label` (for the tooltip and screen readers), `pressed?` | |
| `SegmentedControl` | `options: {value, label}[]`, `value` | |
| `Menu`, `MenuItem` | `anchor`, `open`, items | |
| `Popover` | `anchor`, `open`, `placement` | |
| `Dialog` | `open`, `title`, `width?` | traps focus |
| `Tooltip` | `text` or slot, `delay = 400` | |
| `Icon` | `name`, `size = 16` | one inline SVG sprite, bundled; no icon font |
| `StateIcon` | `state`, `size = 16`, `animated = true` | 1.2 |
| `StateBadge` | `state`, `label?` | icon + word on soft colour |
| `TypeGlyph` | `kind: AgentKind`, `size = 20` | 1.3 |
| `ModelChip` | `model: string \| null`, `compact = false` | compact shows the letter only |
| `Chip` | `icon?`, `label`, `tone?: 'neutral' \| State` | count chips |
| `StatTile` | `label`, `value`, `sub?`, `share?: number` | share draws the thin bar |
| `SegmentedProgress` | `counts: Record<State, number>`, `height = 4`, `animated = true` | per-state segments |
| `PhaseStepper` | `phases: {title, state, done, total}[]`, `current?` | run bar and run node |
| `Sparkline` | `values: number[]`, `width`, `height`, `highlightLast = 0` | bars, SVG |
| `KeyValueList` | `items: {key, value, mono?, copy?}[]` | |
| `ClampText` | `text`, `lines = 12`, `mono = false`, `copy = true` | Show all / Show less |
| `PathText` | `path`, `cwd?` | middle truncation, tooltip |
| `Duration` | `ms` or `from`, `to?` | ticks with the clock |
| `RelativeTime` | `t` | |
| `EmptyState` | `icon`, `title`, `text?`, `action?: {label, onclick}` | 2.10 |
| `Skeleton` | `width`, `height`, `radius?` | |
| `AgentRow` | `agent`, `selected`, `onselect` | one-line agent, used in lists |
| `AgentCard` | `agent`, `variant: 'node' \| 'card'`, `detail: 'full' \| 'compact' \| 'dot'`, `selected`, `dimmed`, `badges?: {attempt?, children?, file?: 'R' \| 'E'}` | the one visual for an agent; Graph and Workflow both use it |

Overview (`overview/`; `views/` holds only the views developer's work):

| Component | Props |
|---|---|
| `OverviewView` | none (reads stores) |
| `OverviewToolbar` | `projects: {name, count}[]`, `project`, `sort` |
| `SessionSection` | `title`, `sessions: Session[]`, `limit?` |
| `SessionCard` | `session`, `stats: {running, agents, runs, tokens}`, `activity: number[]`, `now?: {phaseTitle, done, total}`, `selected` |

Foundation also owns: `App.svelte`, routing between views, global shortcuts (2.9), the theme bootstrap script in `index.html`, and the Tauri event subscription that feeds `world`.

### 6.2 Views (developer B)

Graph (`views/graph/`):

| Component | Props |
|---|---|
| `GraphView` | none (reads stores) |
| `GraphToolbar` | `zoom`, `detail: 'auto' \| 'full' \| 'compact' \| 'dot'`, `sharedFiles: boolean`, `sharedMode: 'edits' \| 'all'`, `linkNote?: string` |
| `SessionNode` | Svelte Flow node; `data: {session, stats}` |
| `AgentNode` | Svelte Flow node wrapping `AgentCard`; `data: {agent, detail, badges}` |
| `RunGroup` | Svelte Flow node; `data: {run, phases, collapsed, stats}` |
| `PhaseGroup` | Svelte Flow node; `data: {phase, counts}` |
| `SpawnEdge` | Svelte Flow edge; `data: {kind: 'spawn' \| 'run' \| 'sequence', active: boolean, highlighted: boolean}` |
| `SharedFileEdge` | Svelte Flow edge; `data: {files: string[], bothEdited: boolean}` |
| `SharedFilesPopover` | `files: string[]`, `onselect(path)` |
| `GraphMiniMap` | wraps Svelte Flow `MiniMap` with state colours |
| `layout.ts` | `toElk` / `fromElk` (pure) and `LayoutEngine`: input `{nodes: {id, width, height}[], edges}` (top level only), output positions; ELK runs in its own worker (`elk-worker`), so no `layout.worker.ts` of ours. Unit tested with 1, 10 and 60 agents |
| `buildGraph.ts` | `world` and view options to nodes and edges. Pure, unit tested |

Workflow (`views/workflow/`):

| Component | Props |
|---|---|
| `WorkflowView` | none |
| `RunHeader` | `run`, `phases`, `totals: {agents, done, running, waiting, failed, tokens, toolCalls, durationMs?}`, `runs: Run[]` |
| `PhaseBoard` | `phases`, `agentsByPhase` |
| `PhaseColumn` | `phase`, `agents: Agent[]`, `active: boolean`, `compact: boolean` |
| `LogPanel` | `lines: {text, t?}[]`, `result?: string`, `open` |

Timeline (`views/timeline/`):

| Component | Props |
|---|---|
| `TimelineView` | none |
| `TimelineToolbar` | `group: 'phase' \| 'start' \| 'state'`, `follow: boolean` |
| `TimeMinimap` | `range`, `window: [number, number]`, `density: number[]`, `runs: {start, end}[]`, `onwindow` |
| `TimeAxis` | `window`, `width`, `mode: 'elapsed' \| 'clock'`, `origin` |
| `TimelineRows` | `rows: TimelineRow[]`, `window`, `viewTime`; virtualised |
| `TimelineLabel` | `row: TimelineRow`, `selected` |
| `AgentBar` | `agent`, `ticks: number[]`, `scale`, `viewTime` |
| `SpanBar` | `start`, `end`, `kind: 'run' \| 'phase'`, `scale` |
| `Playhead` | `t`, `scale`, `mode`, `draggable`, `onseek` |
| `buildRows.ts` | `world`, grouping and collapse state to `TimelineRow[]`. Pure, unit tested |
| `timeScale.ts` | time to pixel mapping, tick step choice, tick merging. Pure, unit tested |

Inspector (`views/inspector/`):

| Component | Props |
|---|---|
| `Inspector` | none (reads `selection`); picks one of the five below |
| `AgentInspector` | `agent`, `chain: Selection[]`, `files: FileTouch[]`, `tools: ToolCall[]`, `children: Agent[]`, `peerMax: {tokens, toolCalls}` |
| `SessionInspector` | `session`, `stats`, `runs`, `agents`, `activity` |
| `RunInspector` | `run`, `phases`, `totals`, `log` |
| `PhaseInspector` | `phase`, `agents`, `counts` |
| `FileInspector` | `path`, `touchers: {agent, mode: 'R' \| 'E', first, last, count}[]` |
| `InspectorSection` | `title`, `count?`, `collapsed` |
| `ParentChain` | `chain: {kind, id, label}[]` |
| `FileList` | `files: {path, mode, count, others}[]`, `limit = 50` |
| `ToolCallList` | `calls: ToolCall[]`, `limit = 20` |
| `ToolBreakdown` | `counts: {name, n}[]` |

Shared view helpers (`views/shared/`): `toolHistory.ts` and `history.svelte.ts` keep every tool call the UI has seen per agent (the snapshot keeps only 8), for Timeline ticks and the Inspector's recent calls; `motion.ts` holds the 12-loop budget; `links.svelte.ts` carries "open Graph / Timeline at this agent" requests between views; `fixtures.ts` builds the 1, 10 and 60 agent sessions for tests.

Views also owns: arrow-key navigation inside each view, view-specific shortcuts (`F`, `+`, `-`, `0`, `S`), and the 60-agent performance check.

### 6.3 Shared ground rules

- A view never reads Tauri events directly; it reads `world`, `clock`, `selection`, `scope`, `search`, `settings`.
- A view never computes "now"; it uses `clock.viewTime`.
- `AgentCard` is the only place that decides how an agent looks. If Graph or Workflow needs something new on it, it is added there.
- Every component renders correctly with missing optional data (no model, no tokens, no task text): it shows "–" or leaves the line out, never "undefined" or "NaN".
- Every list longer than 50 items is clamped or virtualised.
- Each view component has a story-like test page or component test with the synthetic fixtures at 1, 10 and 60 agents.

## 7. Data the design needs

What the UI expects from the snapshot and event stream. Items marked **gap** may not be available from the files; the stated fallback applies, and the UI must not break without them.

| Need | Used by | Status |
|---|---|---|
| Per agent: id, label, type, model, state, parent agent or session, run and phase, queued / started / ended times, attempt, tokens, tool-call count, last tool name and summary, worktree path | everywhere | available (meta file, transcript, run file) |
| Agent state before a run finishes | Graph, Workflow, Timeline | **partly a gap**. The run file with `state` is written only at the end. In flight, state has to be inferred: transcript exists and grows = running; a final result record or `finished` = done. "Waiting" (queued but no file yet) is not visible in flight, so waiting agents appear only once they start; after the run finishes, queue times are known and replay can show the waiting segment. Failed vs stopped in flight may be unknown: show Unknown. |
| Tokens per agent while running | nodes, cards, totals | **partly a gap**: must be summed from `usage` in assistant records; may lag or differ from the final number. Show "–" until the first value. |
| Full task prompt | Inspector | available from the agent's first user record; the run file has only a 400-character preview. If only the preview exists, show "Preview only". |
| One-line task summary | nodes, cards | **derived**: first non-empty line of the prompt cut at 120 characters. A better summary would need a model and is out of scope. |
| Total agents a phase will have | phase `done/total`, whole-run bar | **gap** in flight: the script decides as it goes. `total` means "agents seen so far"; the whole-run bar therefore uses equal phase segments. |
| Which phase is active | RunHeader, session card "Now:" line | **derived**: latest phase with a running agent |
| Log lines while a run is in flight | LogPanel | **gap**: logs are in the run file only, written at the end, and are plain strings without times. In flight the panel shows "Log lines appear when the run finishes". |
| Run result, summary, totals, duration | RunHeader, RunInspector | available at the end only; totals in flight are sums over agents |
| Tool calls: time, tool name, one-line summary, whether it failed | Timeline ticks, Inspector, sparkline, replay density strip | available from transcripts; the core should make the one-line summary (command head, file path, pattern) and cap it at 120 characters |
| Files touched: path, read or edit, time | Inspector, shared-files overlay | **partial**: only for `Read`, `Edit`, `Write`, `NotebookEdit`. About 80% of real tool calls are `Bash`, whose file effects are not known. The UI says so in the empty text. Paths should be made absolute and normalised by the core so that the same file matches across agents and worktrees (a file in a worktree is a different path; matching it to the main checkout is a possible later step). |
| Shared pairs (agent, agent, files, both edited) | Graph overlay | derived in the UI from file touches |
| Session: name, cwd, state, started, updated, pid alive, version | Overview, SessionInspector | available for running sessions |
| Recent (ended) sessions | Overview "Recent" | **gap**: no session file once the process ends. The core has to list recent transcripts under `projects/` and read name, cwd and times from them. If it does not, the Recent section is left out. |
| Activity series per session (tool calls per minute) | Overview sparkline, minimap, scrubber | **derived** in the UI from every tool call: `get_tool_history` after each snapshot (up to 2000 calls per agent), then `toolCallStarted` events |
| Parent of an agent | Graph edges, ParentChain | `parentAgentId` when present; otherwise the session. For workflow agents the parent is the run and phase. Which agent or session started a run is known from where the Workflow tool call sits. |
| Replay: recording start and end, seek to any time (including backwards) with a fresh snapshot, play / pause / speed, event times for previous / next event, list of quiet gaps | ReplayBar, clock | available from the core's Replay source (event times: `ToolHistory.timelineTimes`; gaps: `set_skip_gaps`). Speeds 1x to 120x. If backward seek is slow, the UI shows a thin progress line on the scrubber while waiting. |
| Parse and read warnings | Problems popover | available: `SourceStatus.problems` (time, path, reason, count), newest first, at most 50 |

## 8. Done means

A build meets this spec when:

1. All four themes work in light and dark, switch without reload, follow the system mode by default, and persist across restarts; the contrast test passes.
2. Overview, Graph, Workflow and Timeline render the synthetic fixtures at 1, 10 and 60 agents without overlap, clipping or layout jumps on state updates.
3. Selecting an agent in any view shows it selected in every view and fills the Inspector with all sections of 3.5.
4. Loading the recording at `recordings/first-build` and pressing Space plays it: agents appear in the Graph, cards move through the Workflow columns, bars grow in the Timeline, and seeking backwards restores the earlier picture.
5. With reduced motion on, nothing loops.
6. Every state in 2.10 can be reached and reads as written.
