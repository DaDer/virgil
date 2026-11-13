#!/usr/bin/env python3
"""Record Claude Code on-disk activity for later replay.

Polls a set of paths under the Claude config dir and appends one JSON line per
change to <out>/fs-events.jsonl. Append-only files (*.jsonl) are not copied
while recording: the event carries the byte size at that moment, so a replayer
can serve the prefix of the final file. Small files that get overwritten
(*.json) are copied to <out>/snapshots/ on every change.

Recording layout (format 1):
  manifest.json      {format, claudeDir, sessionId, projectSlug, startedAt, pollMs, roots}
  fs-events.jsonl    {seq, t, op: create|modify|delete, path, size, mtimeMs, snapshot?}
                     path is relative to claudeDir, with forward slashes
  snapshots/<seq>-<basename>   content of an overwritten .json file at that moment
  final/             copy of the watched roots, taken when recording stops

Stop it by creating <out>/STOP, or with SIGTERM/SIGINT.

Usage: recorder.py [--poll-ms 500] [--max-hours 24] -- <out_dir> <project_slug> <session_id>
(the "--" is needed because project slugs start with a dash)
"""
import argparse
import json
import os
import shutil
import signal
import sys
import time

SNAPSHOT_MAX_BYTES = 8 * 1024 * 1024


def now_ms():
    return int(time.time() * 1000)


def scan(claude_dir, roots):
    """Return {relative_path: (size, mtime_ms)} for every file under the roots."""
    found = {}

    def add(full):
        try:
            st = os.stat(full)
        except OSError:
            return
        rel = os.path.relpath(full, claude_dir).replace(os.sep, "/")
        found[rel] = (st.st_size, int(st.st_mtime * 1000))

    for root in roots:
        full_root = os.path.join(claude_dir, root["path"])
        if os.path.isfile(full_root):
            add(full_root)
            continue
        if not os.path.isdir(full_root):
            continue
        suffix = root.get("suffix")
        for dirpath, _dirs, files in os.walk(full_root):
            for name in files:
                if suffix and not name.endswith(suffix):
                    continue
                add(os.path.join(dirpath, name))
    return found


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("out_dir")
    ap.add_argument("project_slug")
    ap.add_argument("session_id")
    ap.add_argument("--poll-ms", type=int, default=500)
    ap.add_argument("--max-hours", type=float, default=24)
    ap.add_argument("--claude-dir", default=os.environ.get("CLAUDE_CONFIG_DIR") or os.path.expanduser("~/.claude"))
    a = ap.parse_args()

    claude_dir = os.path.abspath(a.claude_dir)
    out = os.path.abspath(a.out_dir)
    snaps = os.path.join(out, "snapshots")
    os.makedirs(snaps, exist_ok=True)
    stop_file = os.path.join(out, "STOP")

    project = "projects/" + a.project_slug
    roots = [
        {"path": "sessions", "suffix": ".json"},
        {"path": project + "/" + a.session_id + ".jsonl"},
        {"path": project + "/" + a.session_id},
    ]
    started = now_ms()
    with open(os.path.join(out, "manifest.json"), "w") as f:
        json.dump({
            "format": 1, "claudeDir": claude_dir, "sessionId": a.session_id,
            "projectSlug": a.project_slug, "startedAt": started,
            "pollMs": a.poll_ms, "roots": roots,
        }, f, indent=1)

    stopping = {"flag": False}

    def on_signal(_sig, _frm):
        stopping["flag"] = True

    signal.signal(signal.SIGTERM, on_signal)
    signal.signal(signal.SIGINT, on_signal)

    seq = 0
    known = {}
    events = open(os.path.join(out, "fs-events.jsonl"), "a", buffering=1)

    def emit(op, rel, size, mtime):
        nonlocal seq
        seq += 1
        ev = {"seq": seq, "t": now_ms(), "op": op, "path": rel, "size": size, "mtimeMs": mtime}
        if op != "delete" and rel.endswith(".json") and size <= SNAPSHOT_MAX_BYTES:
            name = "%07d-%s" % (seq, os.path.basename(rel))
            try:
                shutil.copyfile(os.path.join(claude_dir, rel), os.path.join(snaps, name))
                ev["snapshot"] = "snapshots/" + name
            except OSError:
                pass
        events.write(json.dumps(ev) + "\n")

    deadline = time.time() + a.max_hours * 3600
    while not stopping["flag"] and not os.path.exists(stop_file) and time.time() < deadline:
        current = scan(claude_dir, roots)
        for rel, (size, mtime) in current.items():
            old = known.get(rel)
            if old is None:
                emit("create", rel, size, mtime)
            elif old != (size, mtime):
                emit("modify", rel, size, mtime)
        for rel in known:
            if rel not in current:
                emit("delete", rel, 0, 0)
        known = current
        time.sleep(a.poll_ms / 1000.0)

    events.close()
    final = os.path.join(out, "final")
    for root in roots[1:]:
        src = os.path.join(claude_dir, root["path"])
        dst = os.path.join(final, root["path"])
        if os.path.isdir(src):
            shutil.copytree(src, dst, dirs_exist_ok=True)
        elif os.path.isfile(src):
            os.makedirs(os.path.dirname(dst), exist_ok=True)
            shutil.copyfile(src, dst)
    with open(os.path.join(out, "manifest.json")) as f:
        m = json.load(f)
    m["stoppedAt"] = now_ms()
    m["eventCount"] = seq
    with open(os.path.join(out, "manifest.json"), "w") as f:
        json.dump(m, f, indent=1)
    print("recorded %d events to %s" % (seq, out))


if __name__ == "__main__":
    sys.exit(main())
