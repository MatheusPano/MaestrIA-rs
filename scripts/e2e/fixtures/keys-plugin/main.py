import json
import os
import sys

DATA = os.environ["MAESTRIA_PLUGIN_DATA"]
OUT = os.path.join(DATA, "out.txt")
next_id = 0


def send(msg):
    sys.stdout.write(json.dumps(msg) + "\n")
    sys.stdout.flush()


def record(value):
    with open(os.path.join(DATA, "calls.jsonl"), "a") as f:
        f.write(json.dumps(value) + "\n")


def call(method, params):
    global next_id
    next_id += 1
    rid = "r%d" % next_id
    send({"jsonrpc": "2.0", "id": rid, "method": method, "params": params})
    for line in sys.stdin:
        msg = json.loads(line)
        if msg.get("id") == rid and "method" not in msg:
            record({"method": method, "result": msg.get("result"), "error": msg.get("error")})
            return msg.get("result")
    sys.exit(0)


def type_into_terminal():
    tab = call("sessions.focused", {})["id"]
    call("terminal.pasteClipboard", {})
    call("terminal.sendKeys", {"keys": [{"text": " >> " + OUT}, "enter"]})
    call("terminal.sendKeys", {"keys": ["up", "enter"]})
    call("session.rename", {"tabId": tab, "title": "  renomeado pelo plugin  "})


for line in sys.stdin:
    if not line.strip():
        continue
    msg = json.loads(line)
    method = msg.get("method")
    if method == "initialize":
        record({"method": "initialize", "params": msg["params"]})
        send({"jsonrpc": "2.0", "id": msg["id"], "result": {}})
    elif method == "command.invoke":
        send({"jsonrpc": "2.0", "id": msg["id"], "result": None})
        type_into_terminal()
    elif method == "shutdown":
        sys.exit(0)
    elif "id" in msg and method:
        send({"jsonrpc": "2.0", "id": msg["id"], "result": None})
