#!/usr/bin/env python3
"""Generate the Virgil test fixtures. Deterministic: rerunning rewrites identical files.

Everything here is invented. The shapes copy what Claude Code 2.1.32x writes
(see docs/data-contract.md, section 1), the content does not come from any real
transcript.

One timeline of writes is built in memory, then projected three ways:
  claude-small/      the Claude dir at the end of the timeline (T0 + 60 s)
  claude-inflight/   the Claude dir at T0 (workflow mid-run, no run file)
  recording-small/   a format-1 recording of T0 .. T0 + 60 s, polled every 500 ms
and expected.json holds what a correct reader concludes from each.

Usage: python3 fixtures/generate.py
"""
import hashlib
import json
import os
import shutil
from datetime import datetime, timezone

HERE = os.path.dirname(os.path.abspath(__file__))
T0 = 1794650000000  # 2026-11-14T09:53:20.000Z, the "in flight" moment
END = T0 + 60_000
POLL = 500
VERSION = "2.1.323"

S1 = "11111111-1111-4111-8111-111111111111"
S2 = "22222222-2222-4222-8222-222222222222"
CWD1 = "/home/dev/projects/orchard"
CWD2 = "/home/dev/projects/kiln"
SLUG1 = "-home-dev-projects-orchard"
SLUG2 = "-home-dev-projects-kiln"
CLAUDE_DIR = "/home/dev/.claude"
PID_ALIVE = 4242
PID_STALE = 99999999  # above every platform's pid range, never alive
RUN = "wf_0a1b2c3d-4e5"
TASK_ID = "w7k2m9x4q"
WF_NAME = "orchard-stock"

A1, A2, A3 = "a3f1c09d2b7e4a611", "a7c2e15f8d0b3a922", "a9d4b27a6c1e5f033"
W_PLAN, W_API, W_UI, W_TESTS = "ab10c4d7e2f6a8101", "ab20d5e8f3a7b9202", "ab30e6f9a4b8c0303", "ab40f7a0b5c9d1404"

B58 = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"
_counter = {"n": 0}


def at(seconds):
    return T0 + int(round(seconds * 1000))


def iso(ms):
    return datetime.fromtimestamp(ms / 1000, tz=timezone.utc).strftime("%Y-%m-%dT%H:%M:%S.") + "%03dZ" % (ms % 1000)


def token(prefix, length=22):
    _counter["n"] += 1
    digest = hashlib.sha256(("%s-%d" % (prefix, _counter["n"])).encode()).digest()
    return prefix + "".join(B58[b % 58] for b in digest[:length])


def new_uuid():
    _counter["n"] += 1
    h = hashlib.sha256(("uuid-%d" % _counter["n"]).encode()).hexdigest()
    return "%s-%s-4%s-8%s-%s" % (h[:8], h[8:12], h[12:15], h[15:18], h[18:30])


class Files:
    """path -> list of (time_ms, bytes); .jsonl entries append, others replace."""

    def __init__(self):
        self.writes = {}

    def append(self, path, t, obj):
        line = (json.dumps(obj, separators=(",", ":"), ensure_ascii=False) + "\n").encode()
        self.writes.setdefault(path, []).append((t, line))

    def put(self, path, t, content):
        if not isinstance(content, (bytes, str)):
            content = json.dumps(content, separators=(",", ":"), ensure_ascii=False)
        if isinstance(content, str):
            content = content.encode()
        self.writes.setdefault(path, []).append((t, content))

    def content_at(self, path, t):
        """Bytes of the file at time t, or None when it does not exist yet."""
        ws = [w for w in self.writes[path] if w[0] <= t]
        if not ws:
            return None
        if path.endswith(".jsonl"):
            return b"".join(w[1] for w in ws)
        return ws[-1][1]

    def mtime_at(self, path, t):
        return max(w[0] for w in self.writes[path] if w[0] <= t)


FILES = Files()
EXPECT = {}  # agent key -> bookkeeping used for expected.json


class Transcript:
    def __init__(self, session, cwd, slug, agent_id=None, run_id=None, model="claude-opus-5-5"):
        self.session, self.cwd, self.agent_id, self.model = session, cwd, agent_id, model
        base = "projects/%s/%s" % (slug, session)
        if agent_id is None:
            self.path = base + ".jsonl"
        elif run_id:
            self.path = "%s/subagents/workflows/%s/agent-%s.jsonl" % (base, run_id, agent_id)
        else:
            self.path = "%s/subagents/agent-%s.jsonl" % (base, agent_id)
        self.parent = None
        self.context = 11800 + sum((agent_id or session).encode()) % 700
        self.prompt_id = new_uuid()
        self.key = "%s/%s" % (session, agent_id or "main")
        self.stats = EXPECT.setdefault(self.key, {"calls": [], "results": {}, "usage": [], "first": None, "last": None})

    def _common(self, t, kind):
        rec = {"parentUuid": self.parent, "isSidechain": self.agent_id is not None}
        if self.agent_id:
            rec["agentId"] = self.agent_id
        self._tail = {
            "type": kind, "uuid": new_uuid(), "timestamp": iso(t), "userType": "external",
            "entrypoint": "cli", "cwd": self.cwd, "sessionId": self.session, "version": VERSION,
            "gitBranch": "main",
        }
        self.stats["first"] = self.stats["first"] or t
        self.stats["last"] = t
        return rec

    def _emit(self, t, rec):
        rec.update(self._tail)
        self.parent = rec["uuid"]
        FILES.append(self.path, t, rec)
        return rec

    def raw(self, t, obj):
        """A record without the uuid chain (ai-title, queue-operation, ...)."""
        FILES.append(self.path, t, obj)

    def user(self, s, text, **extra):
        t = at(s)
        rec = self._common(t, "user")
        rec["promptId"] = self.prompt_id
        rec["message"] = {"role": "user", "content": text}
        rec.update(extra)
        return self._emit(t, rec)

    def attachment(self, s, attachment):
        t = at(s)
        rec = self._common(t, "attachment")
        rec["attachment"] = attachment
        return self._emit(t, rec)

    def assistant(self, s, blocks, stop="tool_use", output=180):
        """One API message. Written as one record per content block, all sharing message.id and usage."""
        t = at(s)
        msg_id, req_id = token("msg_01"), token("req_01")
        created = 900 + 40 * len(self.stats["usage"])
        usage = {
            "input_tokens": 3, "cache_creation_input_tokens": created,
            "cache_read_input_tokens": self.context, "output_tokens": output,
            "service_tier": "standard",
        }
        self.context += created + output + 3
        self.stats["usage"].append(usage)
        ids = []
        for i, block in enumerate(blocks):
            block = dict(block)
            if block["type"] == "tool_use":
                block["id"] = token("toolu_01")
                block["caller"] = {"type": "direct"}
                ids.append(block["id"])
                self.stats["calls"].append({"id": block["id"], "name": block["name"], "input": block["input"], "t": t + i})
            rec = self._common(t + i, "assistant")
            rec["message"] = {
                "model": self.model, "id": msg_id, "type": "message", "role": "assistant",
                "content": [block], "stop_reason": stop, "stop_sequence": None, "usage": usage,
            }
            rec["requestId"] = req_id
            rec["apiBlockIndex"] = i
            self._emit(t + i, rec)
        return ids[0] if len(ids) == 1 else ids

    def result(self, s, tool_use_id, content, is_error=None, tool_use_result=None):
        t = at(s)
        rec = self._common(t, "user")
        rec["promptId"] = self.prompt_id
        block = {"tool_use_id": tool_use_id, "type": "tool_result", "content": content}
        if is_error is not None:
            block["is_error"] = is_error
        rec["message"] = {"role": "user", "content": [block]}
        if tool_use_result is not None:
            rec["toolUseResult"] = tool_use_result
        rec["sourceToolAssistantUUID"] = self.parent
        self.stats["results"][tool_use_id] = {"t": t, "isError": bool(is_error)}
        return self._emit(t, rec)

    def tool(self, s, name, tool_input, result="ok", took=1.0, is_error=None, thinking=False):
        blocks = ([{"type": "thinking", "thinking": "", "signature": "c2lnbmF0dXJl"}] if thinking else [])
        blocks.append({"type": "tool_use", "name": name, "input": tool_input})
        tid = self.assistant(s, blocks)
        if result is not None:
            self.result(s + took, tid, result, is_error)
        return tid

    def text(self, s, text, stop="end_turn"):
        self.assistant(s, [{"type": "text", "text": text}], stop=stop, output=90)


def read(path):
    return "Read", {"file_path": path}


def edit(path, old, new):
    return "Edit", {"replace_all": False, "file_path": path, "old_string": old, "new_string": new}


def write(path, content):
    return "Write", {"file_path": path, "content": content}


def bash(command, description):
    return "Bash", {"command": command, "description": description}


def p(rel):
    return CWD1 + "/" + rel


def notification(task_id, tool_use_id, status, summary, usage_xml, result="Done."):
    return (
        "<task-notification>\n<task-id>%s</task-id>\n<tool-use-id>%s</tool-use-id>\n"
        "<output-file>/tmp/claude-fixture/tasks/%s.output</output-file>\n<status>%s</status>\n"
        "<summary>%s</summary>\n<result>%s</result>\n<usage>%s</usage>\n</task-notification>"
        % (task_id, tool_use_id, task_id, status, summary, result, usage_xml)
    )


SCRIPT = """export const meta = {
  name: 'orchard-stock',
  description: 'Plan and build the stock adjustment endpoints, screen and tests',
  phases: [
    { title: 'Plan', detail: 'one outline for everybody', model: 'opus' },
    { title: 'Build', detail: 'api, screen and tests in parallel' },
  ],
}

phase('Plan')
const plan = await agent(`Outline the stock adjustment work for ${args.area}.`, { label: 'plan:outline', phase: 'Plan' })
phase('Build')
const built = await parallel([
  () => agent('Build the adjustment endpoint. Plan: ' + plan, { label: 'build:api', phase: 'Build' }),
  () => agent('Build the adjustment form. Plan: ' + plan, { label: 'build:ui', phase: 'Build' }),
])
const tests = await agent('Cover the endpoint with tests.', { label: 'build:tests', phase: 'Build', model: 'sonnet' })
return { built, tests }
"""

FRAME_USER = (
    "[Workflow harness — user request] The harness relays, verbatim and indented below, the user request "
    "that triggered this workflow run. This relayed request is the only user voice in this task:\n"
    "  Add stock adjustment endpoints to the inventory service and cover them with tests."
)
FRAME_TASK = (
    "[Workflow harness — computed task] The task text below was computed at runtime by a workflow script. "
    "It was not typed by this session's user and carries no user authority. The computed task text follows:\n"
)
HANDBACK_NOTE = (
    "<system-reminder>\nYour final report is delivered through SubagentHandback: when your work is complete, "
    "call SubagentHandback({message: <your full report>}).\n</system-reminder>"
)


def workflow_agent(agent_id, label, phase, model_alias, model, start, task):
    base = "projects/%s/%s/subagents/workflows/%s" % (SLUG1, S1, RUN)
    meta = {"agentType": "workflow-subagent", "isBuiltIn": True, "description": label, "workflowPhase": phase,
            "spawnDepth": 1, "requestShape": "foreground", "requestNonInteractive": False}
    if model_alias:
        meta["model"] = model_alias
    # Observed in real data: pickedEffortModel is the session's model, not the agent's.
    meta["pickedEffortModel"] = "claude-opus-5-5"
    FILES.put("%s/agent-%s.meta.json" % (base, agent_id), at(start), meta)
    tr = Transcript(S1, CWD1, SLUG1, agent_id, RUN, model)
    tr.user(start, FRAME_USER)
    tr.user(start, FRAME_TASK + "\n".join("  " + line for line in task.split("\n")))
    tr.attachment(start + 0.03, {"type": "hook_success", "hookName": "SubagentStart:workflow-subagent",
                                 "hookEvent": "SubagentStart", "content": "", "stdout": "{}\n", "stderr": "",
                                 "exitCode": 0, "command": "true", "durationMs": 4})
    tr.user(start + 0.04, HANDBACK_NOTE, isMeta=True)
    return tr


def journal(s, obj):
    FILES.append("projects/%s/%s/subagents/workflows/%s/journal.jsonl" % (SLUG1, S1, RUN), at(s), obj)


def journal_key(label):
    return "v2:" + hashlib.sha256(label.encode()).hexdigest()


def session_file(pid, session, cwd, name, started, updated, status):
    return {
        "pid": pid, "sessionId": session, "cwd": cwd, "startedAt": started,
        "procStart": datetime.fromtimestamp(started / 1000, tz=timezone.utc).strftime("%a %b %e %H:%M:%S %Y"),
        "version": VERSION, "peerProtocol": 1, "peerFeatures": ["notify_idle"], "kind": "interactive",
        "entrypoint": "cli", "pidDomain": "linux", "messagingSocketPath": "/tmp/cc-socks/%d.sock" % pid,
        "name": name, "nameSource": "derived", "nameSince": started, "updatedAt": updated,
        "status": status, "statusUpdatedAt": updated,
    }


def build():
    # ---- session 2: stale pid, a transcript with no agents ------------------------------------
    FILES.put("sessions/%d.json" % PID_STALE, at(-86000),
              session_file(PID_STALE, S2, CWD2, "kiln-firing-log", at(-90000), at(-86000), "busy"))
    m2 = Transcript(S2, CWD2, SLUG2, model="claude-opus-5-5")
    m2.user(-90000, "Why does the firing log skip the cooldown rows?", permissionMode="default", origin={"kind": "human"})
    m2.raw(at(-89999), {"type": "ai-title", "aiTitle": "Firing log cooldown rows", "sessionId": S2})
    m2.tool(-89990, *read(CWD2 + "/src/log.py"), result="def rows(): ...")
    m2.text(-89980, "The reader stops at the first blank line; the cooldown rows come after one.")

    # ---- session 1: main thread ------------------------------------------------------------------
    FILES.put("sessions/%d.json" % PID_ALIVE, at(-600),
              session_file(PID_ALIVE, S1, CWD1, "orchard-inventory", at(-605), at(-600), "busy"))
    FILES.put("sessions/%d.json" % PID_ALIVE, at(-119),
              session_file(PID_ALIVE, S1, CWD1, "orchard-inventory", at(-605), at(-119), "busy"))
    FILES.put("sessions/%d.json" % PID_ALIVE, at(50),
              session_file(PID_ALIVE, S1, CWD1, "orchard-inventory", at(-605), at(50), "idle"))

    main = Transcript(S1, CWD1, SLUG1, model="claude-opus-5-5")
    main.raw(at(-600), {"type": "permission-mode", "permissionMode": "default", "sessionId": S1})
    main.user(-600, "Add stock adjustment endpoints to the inventory service and cover them with tests.",
              permissionMode="default", origin={"kind": "human"}, promptSource="typed")
    main.raw(at(-598), {"type": "ai-title", "aiTitle": "Stock adjustment endpoints", "sessionId": S1})
    main.raw(at(-597), {"type": "future-record-type", "sessionId": S1, "payload": {"anything": [1, 2, 3]}})
    main.tool(-590, *read(p("src/stock.ts")), result="export function level(sku: string) { return 0 }", thinking=True)

    a1_prompt = "Survey the inventory module.\n\nList every place stock levels change and report the call sites."
    tu_a1 = main.tool(-580, "Agent", {"description": "Survey inventory module", "subagent_type": "general-purpose",
                                      "prompt": a1_prompt}, result=None)
    main.result(-579.8, tu_a1, [{"type": "text", "text": "Async agent launched. agentId: %s" % A1}], tool_use_result={
        "isAsync": True, "status": "async_launched", "agentId": A1, "description": "Survey inventory module",
        "resolvedModel": "claude-opus-5-5", "prompt": a1_prompt,
        "outputFile": "/tmp/claude-fixture/tasks/%s.output" % A1, "canReadOutputFile": True, "canContinueAgent": True})
    a3_prompt = "Draft a changelog entry for the stock adjustment endpoints."
    tu_a3 = main.tool(-575, "Agent", {"description": "Draft changelog entry", "subagent_type": "general-purpose",
                                      "model": "sonnet", "prompt": a3_prompt}, result=None)
    main.result(-574.8, tu_a3, [{"type": "text", "text": "Async agent launched. agentId: %s" % A3}], tool_use_result={
        "isAsync": True, "status": "async_launched", "agentId": A3, "description": "Draft changelog entry",
        "resolvedModel": "claude-sonnet-5-5", "prompt": a3_prompt,
        "outputFile": "/tmp/claude-fixture/tasks/%s.output" % A3, "canReadOutputFile": True, "canContinueAgent": True})
    main.text(-574, "Two agents are working; I will wait for the survey.")
    main.raw(at(-573.9), {"parentUuid": main.parent, "isSidechain": False, "type": "system", "subtype": "turn_duration",
                          "durationMs": 26100, "messageCount": 9, "timestamp": iso(at(-573.9)), "uuid": new_uuid(),
                          "isMeta": False, "userType": "external", "entrypoint": "cli", "cwd": CWD1,
                          "sessionId": S1, "version": VERSION, "gitBranch": "main"})

    # ---- plain subagent A1 with nested A2 ---------------------------------------------------------
    sub = "projects/%s/%s/subagents" % (SLUG1, S1)
    a1_meta = {"agentType": "general-purpose", "isBuiltIn": True, "launchedWithDefaults": True,
               "description": "Survey inventory module", "toolUseId": tu_a1, "spawnDepth": 1,
               "requestShape": "background", "requestNonInteractive": True, "pickedEffortModel": "claude-opus-5-5"}
    FILES.put("%s/agent-%s.meta.json" % (sub, A1), at(-579.9), a1_meta)
    a1 = Transcript(S1, CWD1, SLUG1, A1, model="claude-opus-5-5")
    a1.user(-579.9, a1_prompt)
    a1.tool(-577, *read(p("src/stock.ts")), result="export function level(sku: string) { return 0 }")
    a1.tool(-572, *bash('grep -rn "adjust" src/ > /tmp/orchard-adjust.txt', "Collect adjust call sites"), result="")
    a2_prompt = "Trace stock adjustments through the ledger and say which functions write."
    tu_a2 = a1.tool(-560, "Agent", {"description": "Trace stock adjustments", "subagent_type": "general-purpose",
                                    "prompt": a2_prompt}, result=None)
    a1.result(-559.8, tu_a2, [{"type": "text", "text": "Async agent launched. agentId: %s" % A2}], tool_use_result={
        "isAsync": True, "status": "async_launched", "agentId": A2, "description": "Trace stock adjustments",
        "resolvedModel": "claude-opus-5-5", "prompt": a2_prompt})
    a1.text(-559, "Waiting for the ledger trace.")

    a2_meta = {"agentType": "general-purpose", "isBuiltIn": True, "launchedWithDefaults": True,
               "description": "Trace stock adjustments", "toolUseId": tu_a2, "parentAgentId": A1, "spawnDepth": 2,
               "requestShape": "background", "requestNonInteractive": True, "pickedEffortModel": "claude-opus-5-5"}
    FILES.put("%s/agent-%s.meta.json" % (sub, A2), at(-559.9), a2_meta)
    a2 = Transcript(S1, CWD1, SLUG1, A2, model="claude-opus-5-5")
    a2.user(-559.9, a2_prompt)
    a2.tool(-556, *read(p("src/ledger.ts")), result="export function post(entry) {}")
    a2.tool(-548, *read(p("src/stock.ts")), result="export function level(sku: string) { return 0 }")
    a2.text(-521, "Only ledger.post writes; stock.level reads.")
    FILES.put("%s/agent-%s.meta.json" % (sub, A2), at(-520), dict(a2_meta, finished=True))

    a1.user(-519.5, notification(A2, tu_a2, "completed", 'Agent "Trace stock adjustments" finished',
                                 "<subagent_tokens>17442</subagent_tokens><tool_uses>2</tool_uses><duration_ms>39900</duration_ms>"),
            origin={"kind": "task-notification"})
    a1.text(-505, "Stock levels change in ledger.post only; three call sites, all in src/routes.")
    FILES.put("%s/agent-%s.meta.json" % (sub, A1), at(-500), dict(a1_meta, finished=True))

    # ---- plain subagent A3, stopped by the user with a tool call still open -------------------------
    a3_meta = {"agentType": "general-purpose", "isBuiltIn": True, "description": "Draft changelog entry",
               "toolUseId": tu_a3, "spawnDepth": 1, "requestShape": "background", "requestNonInteractive": True,
               "model": "sonnet", "pickedEffortModel": "claude-opus-5-5"}
    FILES.put("%s/agent-%s.meta.json" % (sub, A3), at(-574.9), a3_meta)
    a3 = Transcript(S1, CWD1, SLUG1, A3, model="claude-sonnet-5-5")
    a3.user(-574.9, a3_prompt)
    a3.tool(-570, *read(p("CHANGELOG.md")), result="# Changelog")
    a3.tool(-545, *edit(p("CHANGELOG.md"), "# Changelog", "# Changelog\n\n- Stock adjustments"), result=None)
    FILES.put("%s/agent-%s.meta.json" % (sub, A3), at(-540), dict(a3_meta, stoppedByUser=True))

    # ---- notifications in the main transcript -------------------------------------------------------
    n3 = notification(A3, tu_a3, "killed", 'Agent "Draft changelog entry" was stopped', "", result="")
    main.raw(at(-539.8), {"type": "queue-operation", "operation": "enqueue", "timestamp": iso(at(-539.8)), "sessionId": S1, "content": n3})
    n1 = notification(A1, tu_a1, "completed", 'Agent "Survey inventory module" finished',
                      "<subagent_tokens>21077</subagent_tokens><tool_uses>3</tool_uses><duration_ms>79900</duration_ms>")
    main.raw(at(-499.8), {"type": "queue-operation", "operation": "enqueue", "timestamp": iso(at(-499.8)), "sessionId": S1, "content": n1})
    main.raw(at(-499.5), {"type": "queue-operation", "operation": "remove", "timestamp": iso(at(-499.5)), "sessionId": S1,
                          "content": n1, "reason": "absorbed_mid_turn", "commandUuid": new_uuid(), "deliveryId": new_uuid()})
    main.text(-495, "The survey is in. I will run the build as a workflow.")

    # ---- workflow launch ------------------------------------------------------------------------------
    script_path = "projects/%s/%s/workflows/scripts/%s-%s.js" % (SLUG1, S1, WF_NAME, RUN)
    tu_wf = main.tool(-120.2, "Workflow", {"script": SCRIPT, "args": {"area": "stock"}}, result=None)
    FILES.put(script_path, at(-120.1), SCRIPT)
    journal(-120, {"type": "launched"})
    summary = "Plan and build the stock adjustment endpoints, screen and tests"
    main.result(-120, tu_wf, "Workflow launched in the background. runId: %s" % RUN, is_error=False, tool_use_result={
        "status": "async_launched", "taskId": TASK_ID, "taskType": "local_workflow", "workflowName": WF_NAME,
        "runId": RUN, "summary": summary,
        "transcriptDir": "%s/projects/%s/%s/subagents/workflows/%s" % (CLAUDE_DIR, SLUG1, S1, RUN),
        "scriptPath": "%s/%s" % (CLAUDE_DIR, script_path)})
    main.text(-119.5, "The workflow is running: a plan first, then the endpoint, the form and the tests.")

    # ---- workflow agents --------------------------------------------------------------------------------
    plan = workflow_agent(W_PLAN, "plan:outline", "Plan", "opus", "claude-opus-5-5", -119.9,
                          "Outline the stock adjustment work for stock.\n\nKeep it to one page.")
    journal(-119.9, {"type": "started", "key": journal_key("plan:outline"), "agentId": W_PLAN, "label": "plan:outline", "phase": "Plan"})
    plan.tool(-115, *read(p("src/stock.ts")), result="export function level(sku: string) { return 0 }")
    plan.tool(-95, *write(p("docs/stock-plan.md"), "# Stock adjustments\n\n1. endpoint\n2. form\n3. tests\n"), result="File created")
    plan.tool(-72, "SubagentHandback", {"message": "Plan written to docs/stock-plan.md."}, result="Delivered.", took=0.4)
    journal(-70, {"type": "result", "key": journal_key("plan:outline"), "agentId": W_PLAN,
                  "result": "Plan written to docs/stock-plan.md.", "remoteControlDeviceReport": False,
                  "toolsUsed": ["Read", "Write", "SubagentHandback"]})

    api = workflow_agent(W_API, "build:api", "Build", None, "claude-fable-5-2", -68,
                         "Build the adjustment endpoint. Plan: see docs/stock-plan.md")
    journal(-68, {"type": "started", "key": journal_key("build:api"), "agentId": W_API, "label": "build:api", "phase": "Build"})
    ui = workflow_agent(W_UI, "build:ui", "Build", None, "claude-fable-5-2", -67.9,
                        "Build the adjustment form. Plan: see docs/stock-plan.md")
    journal(-67.9, {"type": "started", "key": journal_key("build:ui"), "agentId": W_UI, "label": "build:ui", "phase": "Build"})

    api.tool(-64, *read(p("docs/stock-plan.md")), result="# Stock adjustments")
    api.tool(-55, *edit(p("src/stock.ts"), "return 0", "return ledger.sum(sku)"), result="Edited")
    api.tool(-44, *write(p("src/routes/adjust.ts"), "export const adjust = () => {}\n"), result="File created")
    api.tool(-36, *bash("npm test -- stock", "Run the stock tests"), result="1 failing", took=6, is_error=True)
    api.tool(-26, *edit(p("src/stock.ts"), "ledger.sum(sku)", "ledger.sum(sku) ?? 0"), result="Edited")
    api.tool(-17, "SubagentHandback", {"message": "Endpoint added in src/routes/adjust.ts."}, result="Delivered.", took=0.4)
    journal(-15, {"type": "result", "key": journal_key("build:api"), "agentId": W_API,
                  "result": "Endpoint added in src/routes/adjust.ts.", "remoteControlDeviceReport": False,
                  "toolsUsed": ["Read", "Edit", "Write", "Bash", "SubagentHandback"]})

    ui.tool(-63, *read(p("docs/stock-plan.md")), result="# Stock adjustments")
    ui.tool(-30, *write(p("src/ui/AdjustForm.tsx"), "export function AdjustForm() { return null }\n"), result="File created")
    # T0 falls here: the Bash call below is open in claude-inflight.
    ui.tool(-4, *bash("npm run build", "Build the app"), result="built in 3.1s", took=9)
    ui.tool(6, *edit(p("src/ui/AdjustForm.tsx"), "return null", "return <form />"), result="Edited")
    ui.tool(10, *read(p("src/routes/adjust.ts")), result="export const adjust = () => {}")
    ui.tool(13.5, "SubagentHandback", {"message": "Form added in src/ui/AdjustForm.tsx."}, result="Delivered.", took=0.4)
    journal(15, {"type": "result", "key": journal_key("build:ui"), "agentId": W_UI,
                 "result": "Form added in src/ui/AdjustForm.tsx.", "remoteControlDeviceReport": False,
                 "toolsUsed": ["Read", "Write", "Bash", "Edit", "SubagentHandback"]})

    tests = workflow_agent(W_TESTS, "build:tests", "Build", "sonnet", "claude-sonnet-5-5", 16,
                           "Cover the endpoint with tests.")
    journal(16, {"type": "started", "key": journal_key("build:tests"), "agentId": W_TESTS, "label": "build:tests", "phase": "Build"})
    tests.tool(18, *read(p("src/stock.ts")), result="export function level(sku: string) { return ledger.sum(sku) ?? 0 }")
    tests.tool(22, *read(p("src/routes/adjust.ts")), result="export const adjust = () => {}")
    tests.tool(30.01, *write(p("test/stock.test.ts"), "test('adjust', () => {})\n"), result="File created")
    tests.tool(36, *bash("cat package.json && npm test -- stock", "Check scripts, run the stock tests"), result="3 passing", took=6)
    tests.tool(43.8, "SubagentHandback", {"message": "Three tests in test/stock.test.ts, all passing."}, result="Delivered.", took=0.4)
    journal(45, {"type": "result", "key": journal_key("build:tests"), "agentId": W_TESTS,
                 "result": "Three tests in test/stock.test.ts, all passing.", "remoteControlDeviceReport": False,
                 "toolsUsed": ["Read", "Write", "Bash", "SubagentHandback"]})

    # ---- run file, written once when the run ends -----------------------------------------------------
    start, end = at(-120), at(46)
    progress = [{"type": "workflow_phase", "index": 1, "title": "Plan"}, {"type": "workflow_phase", "index": 2, "title": "Build"}]
    run_agents = [(W_PLAN, "plan:outline", 1, "Plan", plan, -70), (W_API, "build:api", 2, "Build", api, -15),
                  (W_UI, "build:ui", 2, "Build", ui, 15), (W_TESTS, "build:tests", 2, "Build", tests, 45)]
    total_tokens = total_calls = 0
    for i, (aid, label, pi, pt, tr, done) in enumerate(run_agents, 1):
        last = tr.stats["usage"][-1]
        # Real run files report roughly the last request's input side; a little under the transcript-derived value.
        tok = last["input_tokens"] + last["cache_creation_input_tokens"] + last["cache_read_input_tokens"] + 2
        calls = len(tr.stats["calls"])
        tr.stats["runTokens"] = tok
        total_tokens += tok
        total_calls += calls
        progress.append({
            "type": "workflow_agent", "index": i, "label": label, "phaseIndex": pi, "phaseTitle": pt,
            "agentId": aid, "model": tr.model, "state": "done", "startedAt": tr.stats["first"],
            "queuedAt": tr.stats["first"] - 4, "attempt": 1, "lastToolName": "SubagentHandback",
            "lastToolSummary": tr.stats["calls"][-1]["input"]["message"],
            "promptPreview": "[Workflow harness — user request] The harness relays, verbatim and indented below…",
            "promptFramed": True, "lastProgressAt": tr.stats["last"], "tokens": tok, "toolCalls": calls,
            "durationMs": tr.stats["last"] - tr.stats["first"],
            "resultPreview": tr.stats["calls"][-1]["input"]["message"]})
    FILES.put("projects/%s/%s/workflows/%s.json" % (SLUG1, S1, RUN), end, json.dumps({
        "runId": RUN, "timestamp": iso(end), "taskId": TASK_ID, "script": SCRIPT,
        "scriptPath": "%s/%s" % (CLAUDE_DIR, script_path), "args": {"area": "stock"},
        "result": {"built": ["Endpoint added in src/routes/adjust.ts.", "Form added in src/ui/AdjustForm.tsx."],
                   "tests": "Three tests in test/stock.test.ts, all passing."},
        "agentCount": 4, "logs": [], "durationMs": end - start, "summary": summary, "workflowName": WF_NAME,
        "status": "completed", "startTime": start,
        "phases": [{"title": "Plan", "detail": "one outline for everybody", "model": "opus"},
                   {"title": "Build", "detail": "api, screen and tests in parallel"}],
        "defaultModel": "claude-fable-5-2", "workflowProgress": progress, "totalTokens": total_tokens,
        "totalToolCalls": total_calls}, indent=1))
    EXPECT["run"] = {"totalTokens": total_tokens, "totalToolCalls": total_calls, "startTime": start, "durationMs": end - start}

    nw = notification(TASK_ID, tu_wf, "completed", 'Workflow "orchard-stock" completed',
                      "<agent_count>4</agent_count><agents_done>4</agents_done><agents_error>0</agents_error>"
                      "<subagent_tokens>%d</subagent_tokens><tool_uses>%d</tool_uses><duration_ms>%d</duration_ms>"
                      % (total_tokens, total_calls, end - start), result="{}")
    main.raw(at(46.2), {"type": "queue-operation", "operation": "enqueue", "timestamp": iso(at(46.2)), "sessionId": S1, "content": nw})
    main.raw(at(47), {"type": "queue-operation", "operation": "remove", "timestamp": iso(at(47)), "sessionId": S1,
                      "content": nw, "reason": "absorbed_mid_turn", "commandUuid": new_uuid(), "deliveryId": new_uuid()})
    main.text(48.5, "The workflow finished: endpoint, form and three passing tests.")


def write_dir(root, t, only_prefix=None):
    for path in sorted(FILES.writes):
        if only_prefix and not path.startswith(only_prefix):
            continue
        content = FILES.content_at(path, t)
        if content is None:
            continue
        full = os.path.join(root, path)
        os.makedirs(os.path.dirname(full), exist_ok=True)
        with open(full, "wb") as f:
            f.write(content)


# One poll catches build:tests in the middle of writing a line (see README).
PARTIAL = {"t": T0 + 30_000, "path_suffix": "agent-%s.jsonl" % W_TESTS, "extra": 37}


def write_recording(root):
    snaps = os.path.join(root, "snapshots")
    os.makedirs(snaps, exist_ok=True)
    project = "projects/" + SLUG1
    roots = [{"path": "sessions", "suffix": ".json"}, {"path": "%s/%s.jsonl" % (project, S1)}, {"path": "%s/%s" % (project, S1)}]

    def watched(path):
        return path.startswith("sessions/") or path.startswith("%s/%s" % (project, S1))

    order = sorted(p_ for p_ in FILES.writes if watched(p_))
    order.sort(key=lambda p_: 0 if p_.startswith("sessions/") else 1 if p_.endswith(S1 + ".jsonl") else 2)
    known, events, partial_at = {}, [], None
    for poll in range(T0, END, POLL):
        tick = 0
        for path in order:
            content = FILES.content_at(path, poll)
            if content is None:
                continue
            size, mtime = len(content), FILES.mtime_at(path, poll)
            if poll == PARTIAL["t"] and path.endswith(PARTIAL["path_suffix"]):
                later = FILES.content_at(path, poll + POLL)
                assert len(later) > size + PARTIAL["extra"], "partial-line poll needs a line landing in the next poll"
                size, mtime, partial_at = size + PARTIAL["extra"], poll, len(events) + 1
            if known.get(path) == (size, mtime):
                continue
            seq = len(events) + 1
            tick += 1
            ev = {"seq": seq, "t": poll + tick, "op": "modify" if path in known else "create",
                  "path": path, "size": size, "mtimeMs": mtime}
            if path.endswith(".json"):
                name = "%07d-%s" % (seq, os.path.basename(path))
                with open(os.path.join(snaps, name), "wb") as f:
                    f.write(content)
                ev["snapshot"] = "snapshots/" + name
            known[path] = (size, mtime)
            events.append(ev)
    with open(os.path.join(root, "fs-events.jsonl"), "w") as f:
        for ev in events:
            f.write(json.dumps(ev) + "\n")
    write_dir(os.path.join(root, "final"), END, only_prefix=project + "/" + S1)
    with open(os.path.join(root, "manifest.json"), "w") as f:
        json.dump({"format": 1, "claudeDir": CLAUDE_DIR, "sessionId": S1, "projectSlug": SLUG1, "startedAt": T0,
                   "pollMs": POLL, "roots": roots, "stoppedAt": END, "eventCount": len(events)}, f, indent=1)
    return events, partial_at


def tokens_at(stats, t):
    """Contract rule: context of the latest assistant message = input + cache_creation + cache_read + output."""
    # usage entries are in order; count the messages whose records exist at t via the calls/text timeline
    n = stats.get("_msg_times")
    usable = [u for (mt, u) in n if mt <= t]
    if not usable:
        return 0
    u = usable[-1]
    return u["input_tokens"] + u["cache_creation_input_tokens"] + u["cache_read_input_tokens"] + u["output_tokens"]


FILE_TOOLS = {"Read": "read", "Edit": "write", "Write": "write"}
BASH_TOUCHES = {  # what section 2.7 of the contract detects in the fixture's Bash commands
    'grep -rn "adjust" src/ > /tmp/orchard-adjust.txt': [("/tmp/orchard-adjust.txt", "write")],
    "cat package.json && npm test -- stock": [(CWD1 + "/package.json", "read")],
}


def expected_at(t, final):
    """What a correct reader concludes from the Claude dir as it is at time t."""
    def agent(key, **fixed):
        st = EXPECT[key]
        calls = [c for c in st["calls"] if c["t"] <= t]
        open_calls = [c["id"] for c in calls if not (c["id"] in st["results"] and st["results"][c["id"]]["t"] <= t)]
        files = {}
        for c in calls:
            touched = []
            if c["name"] in FILE_TOOLS:
                touched = [(c["input"]["file_path"], FILE_TOOLS[c["name"]])]
            elif c["name"] == "Bash":
                touched = BASH_TOUCHES.get(c["input"]["command"], [])
            for path, op in touched:
                e = files.setdefault(path, {"reads": 0, "writes": 0})
                e["reads" if op == "read" else "writes"] += 1
        out = {"toolCalls": len(calls), "openToolCalls": len(open_calls),
               "lastToolName": calls[-1]["name"] if calls else None,
               "transcriptTokens": tokens_at(st, t), "files": files}
        out.update(fixed)
        return out

    inflight = not final
    agents = {
        "main": agent(S1 + "/main", kind="main", parentAgentId=None, state="waiting" if final else "running", runId=None, spawnDepth=0,
                      label="Stock adjustment endpoints", task="Add stock adjustment endpoints to the inventory service and cover them with tests."),
        A1: agent(S1 + "/" + A1, kind="subagent", parentAgentId="main", state="done", runId=None, spawnDepth=1, label="Survey inventory module", task="Survey the inventory module.", model="claude-opus-5-5"),
        A2: agent(S1 + "/" + A2, kind="subagent", parentAgentId=A1, state="done", runId=None, spawnDepth=2, label="Trace stock adjustments", task="Trace stock adjustments through the ledger and say which functions write.", model="claude-opus-5-5"),
        A3: agent(S1 + "/" + A3, kind="subagent", parentAgentId="main", state="stopped", runId=None, spawnDepth=1, label="Draft changelog entry", task="Draft a changelog entry for the stock adjustment endpoints.", model="claude-sonnet-5-5", stateReason="stoppedByUser"),
        W_PLAN: agent(S1 + "/" + W_PLAN, kind="workflow", parentAgentId="main", state="done", runId=RUN, phaseIndex=1, spawnDepth=1, label="plan:outline", model="claude-opus-5-5", task="Outline the stock adjustment work for stock."),
        W_API: agent(S1 + "/" + W_API, kind="workflow", parentAgentId="main", state="done", runId=RUN, phaseIndex=2, spawnDepth=1, label="build:api", model="claude-fable-5-2", task="Build the adjustment endpoint. Plan: see docs/stock-plan.md"),
        W_UI: agent(S1 + "/" + W_UI, kind="workflow", parentAgentId="main", state="running" if inflight else "done", runId=RUN, phaseIndex=2, spawnDepth=1, label="build:ui", model="claude-fable-5-2", task="Build the adjustment form. Plan: see docs/stock-plan.md"),
    }
    if final:
        agents[W_TESTS] = agent(S1 + "/" + W_TESTS, kind="workflow", parentAgentId="main", state="done", runId=RUN, phaseIndex=2, spawnDepth=1, label="build:tests", model="claude-sonnet-5-5", task="Cover the endpoint with tests.")
        for wid in (W_PLAN, W_API, W_UI, W_TESTS):
            agents[wid]["tokens"] = EXPECT[S1 + "/" + wid]["runTokens"]  # the run file wins
    shared = {}
    for aid, a in agents.items():
        for path in a["files"]:
            shared.setdefault(path, []).append(aid)
    run = {"runId": RUN, "name": WF_NAME, "parentAgentId": "main", "final": final,
           "status": "completed" if final else "running",
           "phases": [{"index": 1, "title": "Plan", "state": "done"}, {"index": 2, "title": "Build", "state": "done" if final else "running"}],
           "agentIds": [W_PLAN, W_API, W_UI] + ([W_TESTS] if final else []),
           "startedAt": at(-120), "endedAt": at(46) if final else None}
    if final:
        run.update(totalTokens=EXPECT["run"]["totalTokens"], totalToolCalls=EXPECT["run"]["totalToolCalls"])
    return {
        "t": t,
        "sessions": {
            S1: {"pid": PID_ALIVE, "liveness": "alive (inject the pid probe; unknown in replay)", "status": "idle" if final else "busy",
                 "title": "Stock adjustment endpoints", "name": "orchard-inventory", "cwd": CWD1, "agentCount": len(agents),
                 "agents": agents, "runs": {RUN: run},
                 "filesTouchedByMoreThanOneAgent": {k: v for k, v in sorted(shared.items()) if len(v) > 1}},
            S2: {"pid": PID_STALE, "liveness": "stale", "status": "busy", "title": "Firing log cooldown rows",
                 "name": "kiln-firing-log", "cwd": CWD2, "agentCount": 1,
                 "agents": {"main": dict(agent(S2 + "/main", kind="main", parentAgentId=None, state="stopped", stateReason="sessionGone", runId=None, spawnDepth=0))},
                 "runs": {}},
        },
    }


def main():
    build()
    # message times for token bookkeeping: one usage per assistant() call, in order
    for key, st in EXPECT.items():
        if key == "run":
            continue
        st["_msg_times"] = []
    # Rebuild message times by re-reading the generated transcripts (keeps the bookkeeping honest).
    for path, writes in FILES.writes.items():
        if not path.endswith(".jsonl") or path.endswith("journal.jsonl"):
            continue
        seen = set()
        for t, line in writes:
            rec = json.loads(line)
            if rec.get("type") != "assistant":
                continue
            key = "%s/%s" % (rec["sessionId"], rec.get("agentId") or "main")
            mid = rec["message"]["id"]
            if mid not in seen:
                seen.add(mid)
                EXPECT[key]["_msg_times"].append((t, rec["message"]["usage"]))

    for name in ("claude-small", "claude-inflight", "recording-small"):
        shutil.rmtree(os.path.join(HERE, name), ignore_errors=True)
    write_dir(os.path.join(HERE, "claude-small"), END)
    write_dir(os.path.join(HERE, "claude-inflight"), T0)
    events, partial_seq = write_recording(os.path.join(HERE, "recording-small"))

    expected = {
        "note": "Generated by fixtures/generate.py. Times are epoch milliseconds. transcriptTokens follows the "
                "contract's transcript rule; where an agent also has 'tokens', that is the run-file value and wins.",
        "t0": T0, "end": END,
        "claude-small": expected_at(END, True),
        "claude-inflight": expected_at(T0, False),
        "recording-small": {
            "timeline": {"start": T0, "end": END}, "eventCount": len(events), "partialLineEventSeq": partial_seq,
            "atStart": "position = start (initial scan applied): identical to claude-inflight, except that both "
                       "sessions have liveness unknown, and session 2 has no transcript: hasTranscript false, "
                       "title null, its main agent has 0 tool calls and state running (status busy, not stale)",
            "atEnd": "identical to claude-small, with the same exceptions",
            "initialScanEventCount": sum(1 for e in events if e["t"] < T0 + POLL),
            "milestones": {
                "buildUiDoneAt": at(15), "buildTestsFirstSeenAt": at(16), "buildTestsDoneAt": at(45),
                "runFileAppearsAt": at(46), "sessionBecomesIdleAt": at(50)},
            "at20s": {"t": T0 + 20_000, "runFinal": False, "workflowAgentsKnown": 4,
                      "states": {W_PLAN: "done", W_API: "done", W_UI: "done", W_TESTS: "running"}},
        },
    }
    with open(os.path.join(HERE, "expected.json"), "w") as f:
        json.dump(expected, f, indent=1, sort_keys=True)
        f.write("\n")
    print("wrote fixtures: %d files in the timeline, %d recording events, partial-line event seq %s"
          % (len(FILES.writes), len(events), partial_seq))


if __name__ == "__main__":
    main()
