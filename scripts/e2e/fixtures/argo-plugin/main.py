import json
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
LIBRARY = open(os.path.join(HERE, "argo.rfwtxt")).read()
next_id = 0


def send(msg):
    sys.stdout.write(json.dumps(msg) + "\n")
    sys.stdout.flush()


def request(method, params):
    global next_id
    next_id += 1
    send({"jsonrpc": "2.0", "id": "r%d" % next_id, "method": method, "params": params})


def tile(name, health, sync, op, rows):
    icons = {"Healthy": "check", "Progressing": "clock", "Degraded": "error", "Missing": "warning"}
    tones = {"Healthy": "green", "Progressing": "accent", "Degraded": "red", "Missing": "yellow", "Unknown": "faint"}
    return {
        "name": name, "a": "app:" + name, "icon": icons.get(health, "dot"), "health": tones.get(health, "faint"), "healthText": health,
        "sync": "green" if sync == "Synced" else "yellow", "syncText": sync,
        "op": op, "opTone": "accent",
        "rows": [{"k": k, "v": v} for k, v in rows],
        "sync_a": "noop", "refresh_a": "noop", "browser_a": "noop",
    }


LONG_REPO = "gitlab.example.internal/plataforma/infraestrutura/charts-dos-servicos-de-producao-com-nome-comprido"
APPS = [
    tile("api-de-pagamentos-recorrentes-com-um-nome-bem-comprido-que-nao-cabe", "Missing", "OutOfSync", "sincronizando",
         [("projeto", "plataforma-financeira-e-cobranca"), ("namespace", "pagamentos-recorrentes-producao"),
          ("repo", LONG_REPO), ("alvo", "charts/api-pagamentos @ release/2026-10-23-hotfix-urgente"), ("último sync", "há 3 minutos")]),
    tile("web", "Healthy", "Synced", "", [("projeto", "default"), ("namespace", "web"), ("repo", "web"), ("alvo", ". @ HEAD"), ("último sync", "há 2 dias")]),
    tile("worker-de-filas-assincronas", "Healthy", "OutOfSync", "sync falhou",
         [("projeto", "default"), ("namespace", "filas"), ("repo", LONG_REPO), ("alvo", "deploy/k8s/overlays/production @ main"), ("último sync", "há 1 hora")]),
    tile("relatorios", "Degraded", "Unknown", "", [("projeto", "bi"), ("namespace", "relatorios"), ("repo", "bi-reports"), ("alvo", ". @ v1.2.3"), ("último sync", "—")]),
]

HEAD = {"title": "producao-cluster-principal-sa-east-1-com-um-nome-que-nao-cabe-na-linha",
        "sub": "argocd · 4 de 4 apps · conectado via kubeconfig em ~/.kube/clusters/producao-cluster-principal.yaml",
        "buttons": [{"icon": "search", "tip": "filtrar", "a": "noop", "tone": ""},
                    {"icon": "refresh", "tip": "recarregar", "a": "noop", "tone": ""},
                    {"icon": "globe", "tip": "navegador", "a": "noop", "tone": ""}]}
STATS = [{"text": "4 apps", "tone": "accent", "a": "noop"}, {"text": "2 fora de sync", "tone": "yellow", "a": "noop"},
         {"text": "3 com problema", "tone": "red", "a": "noop"}]
NOTICE = {"text": "ComparisonError: falha ao gerar os manifestos do chart charts/api-pagamentos: rpc error: code = Unknown desc = helm template . --name-template api-pagamentos --namespace pagamentos-recorrentes-producao failed exit status 1: Error: template: api-pagamentos/templates/deployment.yaml:42:20: executing \"api-pagamentos/templates/deployment.yaml\" at <.Values.image.tag>: nil pointer evaluating interface {}.tag", "tone": "red"}

CLUSTERS = [
    {"name": "producao-cluster-principal-sa-east-1-com-nome-longo", "a": "noop", "status": "green",
     "sub": "12 apps · 3 com atenção", "subTone": "yellow", "buttons": [{"icon": "globe", "tip": "navegador", "a": "noop"}, {"icon": "stop", "tip": "desconectar", "a": "noop"}]},
    {"name": "homologacao", "a": "noop", "status": "red",
     "sub": "kubectl port-forward falhou: error: error upgrading connection: unable to upgrade connection: Unauthorized (o token do kubeconfig expirou, sincronize pelo Rancher)",
     "subTone": "red", "buttons": [{"icon": "globe", "tip": "navegador", "a": "noop"}]},
    {"name": "dev", "a": "noop", "status": "none", "sub": "", "subTone": "", "buttons": [{"icon": "globe", "tip": "navegador", "a": "noop"}]},
]


def node(i, x, y, title, sub, health):
    return {"x": x, "y": y, "sel": i == 0, "a": "noop", "health": health, "abbr": "deploy", "title": title, "sub": sub, "sync": "green"}


GRAPH = {"w": 820.0, "h": 260.0, "svg": '<svg xmlns="http://www.w3.org/2000/svg" width="820" height="260"/>', "nodes": [
    node(0, 0.0, 10.0, "api-de-pagamentos-recorrentes-com-um-nome-bem-comprido", "Application · Progressing · OutOfSync", "accent"),
    node(1, 290.0, 10.0, "api-pagamentos-deployment-principal-v2", "Deployment · 3/3 réplicas prontas e saudáveis", "green"),
    node(2, 580.0, 10.0, "api-pagamentos-7d9f8c6b5-xk2lp-pod-com-sufixo", "Pod · Running · reiniciou 14 vezes", "red"),
]}


def view(command):
    if command == "clusters":
        data = {"head": dict(HEAD, title="ArgoCD", sub="3 clusters · 1 conectado"), "crumb": {"cluster": ""}, "stats": [],
                "notice": {"text": "", "tone": "yellow"}, "clusters": CLUSTERS, "apps": []}
        return {"viewId": "argo", "title": "ArgoCD", "rfw": {"library": LIBRARY, "root": "home"}, "data": data}
    if command == "tree":
        data = {"head": HEAD, "stats": STATS, "notice": NOTICE, "graph": GRAPH}
        return {"viewId": "argo", "title": "ArgoCD", "rfw": {"library": LIBRARY, "root": "tree"}, "data": data}
    data = {"head": HEAD, "crumb": {"cluster": HEAD["title"]}, "stats": STATS, "notice": NOTICE, "clusters": [], "apps": APPS}
    return {"viewId": "argo", "title": "ArgoCD", "rfw": {"library": LIBRARY, "root": "home"}, "data": data}


for line in sys.stdin:
    if not line.strip():
        continue
    msg = json.loads(line)
    method = msg.get("method")
    if method == "shutdown":
        sys.exit(0)
    if "id" in msg and method:
        send({"jsonrpc": "2.0", "id": msg["id"], "result": None})
    if method == "command.invoke":
        request("view.open", view(msg["params"]["command"]))
