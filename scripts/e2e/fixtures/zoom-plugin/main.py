import json
import os
import sys

DATA = os.environ["MAESTRIA_PLUGIN_DATA"]
HERE = os.path.dirname(os.path.abspath(__file__))
LIBRARY = open(os.path.join(HERE, "ui.rfwtxt")).read()


def send(msg):
    sys.stdout.write(json.dumps(msg) + "\n")
    sys.stdout.flush()


def record(name, value):
    with open(os.path.join(DATA, name), "a") as f:
        f.write(json.dumps(value) + "\n")


for line in sys.stdin:
    if not line.strip():
        continue
    msg = json.loads(line)
    method = msg.get("method")
    if method == "initialize":
        send({"jsonrpc": "2.0", "id": msg["id"], "result": {}})
    elif method == "command.invoke":
        send({"jsonrpc": "2.0", "id": msg["id"], "result": None})
        record("commands.jsonl", msg["params"]["command"])
        send({"jsonrpc": "2.0", "id": "open", "method": "view.open",
              "params": {"viewId": "z", "title": "Planta", "modal": True, "width": 820, "height": 640,
                         "rfw": {"library": LIBRARY, "root": "root"}, "data": {}}})
    elif method == "view.action":
        record("actions.jsonl", msg["params"])
    elif method == "shutdown":
        sys.exit(0)
    elif "id" in msg and method:
        send({"jsonrpc": "2.0", "id": msg["id"], "result": None})
