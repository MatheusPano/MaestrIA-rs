import json
import os
import struct
import sys
import zlib

DATA = os.environ["MAESTRIA_PLUGIN_DATA"]
HERE = os.path.dirname(os.path.abspath(__file__))
queue = []
next_id = 0
sent = 0


def png(path, w, h, pixel):
    raw = b"".join(b"\x00" + b"".join(bytes(pixel(x, y)) for x in range(w)) for y in range(h))
    def chunk(kind, body):
        return struct.pack(">I", len(body)) + kind + body + struct.pack(">I", zlib.crc32(kind + body) & 0xFFFFFFFF)
    data = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 6, 0, 0, 0))
    data += chunk(b"IDAT", zlib.compress(raw)) + chunk(b"IEND", b"")
    with open(path, "wb") as f:
        f.write(data)


png(os.path.join(HERE, "sprite.png"), 8, 4, lambda x, y: (255, 40, 40, 255) if x < 4 else (40, 255, 40, 255))
LIBRARY = open(os.path.join(HERE, "ui.rfwtxt")).read()


def send(msg):
    sys.stdout.write(json.dumps(msg) + "\n")
    sys.stdout.flush()


def record(name, value):
    with open(os.path.join(DATA, name), "a") as f:
        f.write(json.dumps(value) + "\n")


def call(method, params):
    global next_id
    next_id += 1
    rid = "r%d" % next_id
    send({"jsonrpc": "2.0", "id": rid, "method": method, "params": params})
    for line in sys.stdin:
        msg = json.loads(line)
        if msg.get("id") == rid and "method" not in msg:
            return msg.get("result")
        queue.append(msg)
    sys.exit(0)


def data():
    return {"sent": sent, "zoomLabel": "mapa"}


def handle(msg):
    global sent
    method = msg.get("method")
    if method == "initialize":
        record("initialize.jsonl", msg["params"])
        send({"jsonrpc": "2.0", "id": msg["id"], "result": {}})
    elif method == "command.invoke":
        send({"jsonrpc": "2.0", "id": msg["id"], "result": None})
        record("commands.jsonl", msg["params"]["command"])
        call("view.open", {"viewId": "w", "title": "Widgets", "modal": True, "width": 820, "height": 640,
                           "rfw": {"library": LIBRARY, "root": "root"}, "data": data()})
    elif method == "view.action":
        p = msg["params"]
        record("actions.jsonl", p)
        if p.get("action") == "send":
            sent += 1
            call("view.update", {"viewId": "w", "data": {"sent": sent}})
        elif p.get("action") == "grow":
            call("view.open", {"viewId": "g", "title": "Cresceu", "modal": True, "from": "grow", "width": 500,
                               "height": 300, "rfw": {"library": LIBRARY, "root": "grown"}})
    elif method == "event":
        record("events.jsonl", msg["params"])
    elif method == "shutdown":
        sys.exit(0)
    elif "id" in msg and method:
        send({"jsonrpc": "2.0", "id": msg["id"], "result": None})


for line in sys.stdin:
    if not line.strip():
        continue
    handle(json.loads(line))
    while queue:
        handle(queue.pop(0))
