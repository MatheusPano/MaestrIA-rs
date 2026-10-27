import json
import os
import sys

DATA = os.environ["MAESTRIA_PLUGIN_DATA"]
queue = []
next_id = 0


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


MODAL = {"viewId": "m", "title": "Modal da sonda", "modal": True, "blocks": [
    {"type": "heading", "text": "Modal da sonda"},
    {"type": "input", "id": "nota", "label": "Nota", "value": "inicial"},
    {"type": "button", "action": "clicou", "label": "Clique aqui", "style": "primary"},
]}


def ask():
    picked = call("window.pick", {"title": "Escolha", "items": [
        {"value": "alfa", "label": "Alfa"}, {"value": "beta", "label": "Beta"}, {"value": "gama", "label": "Gama"}]})
    record("answers.jsonl", {"pick": picked})
    typed = call("window.input", {"title": "Digite", "value": "rascunho"})
    record("answers.jsonl", {"input": typed})
    form = call("window.form", {"title": "Formulario", "fields": [
        {"id": "nome", "label": "Nome", "required": True},
        {"id": "tipo", "label": "Tipo", "type": "select", "options": ["um", "dois"], "value": "dois"}],
        "buttons": [{"action": "salvar", "label": "Salvar", "style": "primary"}]})
    record("answers.jsonl", {"form": form})


def handle(msg):
    method = msg.get("method")
    if method == "initialize":
        send({"jsonrpc": "2.0", "id": msg["id"], "result": {}})
    elif method == "command.invoke":
        command = msg["params"]["command"]
        send({"jsonrpc": "2.0", "id": msg["id"], "result": None})
        record("commands.jsonl", command)
        if command == "modal":
            call("view.open", MODAL)
        elif command == "ask":
            ask()
    elif method == "view.action":
        record("actions.jsonl", msg["params"])
        call("view.update", {"viewId": "m", "blocks": MODAL["blocks"] + [{"type": "text", "text": "clicado"}]})
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
