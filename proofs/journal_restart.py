"""Crash and restart a broker fixture around an independently visible effect."""

import argparse
import json
import os
from pathlib import Path
import subprocess
import sys


def append(path, text):
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_APPEND, 0o600)
    with os.fdopen(fd, "w") as file:
        file.write(text + "\n")
        file.flush()
        os.fsync(file.fileno())


def recover(journal):
    if not journal.exists():
        return
    data = journal.read_bytes()
    if data and not data.endswith(b"\n"):
        raise ValueError("Incomplete journal record. Retained original bytes.")
    outstanding = set()
    for line in data.splitlines():
        event = json.loads(line)
        if event["stage"] == "dispatch":
            outstanding.add(event["action"])
        elif event["stage"] in {"completed", "unknown"}:
            outstanding.remove(event["action"])
    for action in sorted(outstanding):
        append(journal, json.dumps({"stage": "unknown", "action": action}))


def rpc(process, message):
    process.stdin.write(json.dumps(message) + "\n")
    process.stdin.flush()
    return json.loads(process.stdout.readline())


def binding(runtime):
    return {"runtime": runtime, "task": 1, "resource": 1}


def worker(driver, directory):
    journal = directory / "journal.jsonl"
    effects = directory / "effects.txt"
    recover(journal)
    with subprocess.Popen([driver, "--explicit-grant"], stdin=subprocess.PIPE,
                          stdout=subprocess.PIPE, text=True) as policy:
        for line in sys.stdin:
            command = json.loads(line)
            target = binding(command["runtime"])
            if command["type"] == "grant":
                print(json.dumps(rpc(policy, {"type": "grant", "binding": target})), flush=True)
                continue
            action = command["action"]

            def record(stage, **details):
                append(journal, json.dumps({"stage": stage, "action": action, **details}))

            record("intent", runtime=command["runtime"])
            request = {"binding": target, "revision": 0, "operation": "activate", "destination": "local"}
            result = rpc(policy, {"type": "check", "request": request})
            record("permission", denial=result["denial"])
            if result["denial"] is None:
                record("dispatch")
                append(effects, action)
                if command.get("crash"):
                    os._exit(86)
                record("completed")
            print(json.dumps(result), flush=True)
        policy.stdin.close()


def run(driver, directory):
    directory.mkdir(mode=0o700)
    args = [sys.executable, __file__, driver, str(directory), "--worker"]
    journal = directory / "journal.jsonl"
    effects = directory / "effects.txt"

    def start():
        return subprocess.Popen(args, stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)

    def perform(name, runtime):
        return {"type": "perform", "action": name, "runtime": runtime}

    with start() as before:
        assert rpc(before, {"type": "grant", "runtime": 1})["denial"] is None
        assert rpc(before, perform("completed-before-crash", 1))["denial"] is None
        before.stdin.write(json.dumps(perform("interrupted", 1) | {"crash": True}) + "\n")
        before.stdin.flush()
        assert before.wait(timeout=5) == 86
    prefix = journal.read_bytes()
    pre_effects = effects.read_text().splitlines()
    assert pre_effects == ["completed-before-crash", "interrupted"]

    with start() as after:
        assert rpc(after, perform("before-new-grant", 1))["denial"] == "no_grant"
        assert rpc(after, {"type": "grant", "runtime": 2})["denial"] is None
        assert rpc(after, perform("old-runtime", 1))["denial"] == "wrong_binding"
        assert rpc(after, perform("new-runtime", 2))["denial"] is None
        after.stdin.close()
        assert after.wait(timeout=5) == 0
    recovered = journal.read_bytes()
    assert recovered.startswith(prefix)
    assert effects.read_text().splitlines() == pre_effects + ["new-runtime"]
    assert recovered.count(b'"stage": "unknown", "action": "interrupted"') == 1
    assert b'"stage": "completed", "action": "interrupted"' not in recovered
    subprocess.run(args, input="", text=True, timeout=5, check=True)
    assert journal.read_bytes() == recovered

    with journal.open("ab") as file:
        file.write(b'{"stage":"dispatch"')
    damaged = journal.read_bytes()
    failed = subprocess.run(args, input="", text=True, capture_output=True, timeout=5)
    assert failed.returncode != 0 and "Incomplete journal record" in failed.stderr
    assert journal.read_bytes() == damaged
    (directory / "recovered.jsonl").write_bytes(recovered)
    print("Restart preserved history, marked one dispatch unknown, denied stale authority, and rejected a torn record.")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("driver", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--worker", action="store_true", help=argparse.SUPPRESS)
    options = parser.parse_args()
    (worker if options.worker else run)(str(options.driver.resolve(strict=True)), options.output.resolve())
