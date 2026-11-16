#!/usr/bin/env python3
"""The MCP tool registry the viewer keeps, the services its servers connect to (services.json),
and which service each server is (servers.json).

    python3 scripts/mcp_registry.py slim ~/Desktop/registry_tools.jsonl
        Cuts the registry export down to what the app shows, one line per server, into
        data/mcp/registry.json: each tool's name, category, action and grade reason.
    python3 scripts/mcp_registry.py suggest
        Lists registry servers servers.json does not place yet (new ones after a `slim`), with
        their tools, to add to servers.json by hand.
    python3 scripts/mcp_registry.py check
        Checks services.json, servers.json and registry.json against each other.
    python3 scripts/mcp_registry.py actions
        Writes data/mcp/tool_actions.json, which the app carries to tell what an MCP tool does
        from its name: the action most registry tools whose name starts with a word have
        (`verbs`), and the tools whose own action differs from their first word's (`names`).
"""

import collections
import json
import re
import sys
from pathlib import Path

DATA = Path(__file__).resolve().parent.parent / "data" / "mcp"
SERVICES = DATA / "services.json"
REGISTRY = DATA / "registry.json"
SERVERS = DATA / "servers.json"
FIELDS = ["name", "category", "action", "grade_reason"]


def slim(src):
    servers = collections.defaultdict(list)
    for line in open(src, encoding="utf-8"):
        if not line.strip():
            continue
        r = json.loads(line)
        if r.get("removed_at"):
            continue
        servers[r["source_id"]].append([r.get(k) for k in FIELDS])
    for tools in servers.values():
        tools.sort(key=lambda t: t[0])
    head = {"source": Path(src).name, "fields": FIELDS}
    lines = [f"  {json.dumps(sid, ensure_ascii=False)}: {json.dumps(servers[sid], ensure_ascii=False)}" for sid in sorted(servers, key=str.lower)]
    out = "{\n" + f'  "about": {json.dumps(head, ensure_ascii=False)},\n' + '  "servers": {\n' + ",\n".join("  " + l for l in lines) + "\n  }\n}\n"
    REGISTRY.write_text(out, encoding="utf-8")
    print(f"{REGISTRY.relative_to(DATA.parent.parent)}: 서버 {len(servers)}개, 도구 {sum(map(len, servers.values()))}개, {len(out) / 1e6:.1f}MB")


def load():
    services = json.loads(SERVICES.read_text(encoding="utf-8"))
    servers = json.loads(SERVERS.read_text(encoding="utf-8"))["servers"]
    registry = json.loads(REGISTRY.read_text(encoding="utf-8"))["servers"]
    return services, servers, registry


def suggest():
    """The registry's servers servers.json does not place yet, with what tells what they are."""
    services, servers, registry = load()
    by_category = {c: a["id"] for a in services["areas"] for c in a["categories"]}
    left = [sid for sid in registry if sid not in servers]
    for sid in sorted(left, key=str.lower):
        tools = registry[sid]
        top = collections.Counter(t[1] for t in tools).most_common(1)[0][0]
        print(f"{sid}  도구 {len(tools)}개, 분야 후보 {by_category.get(top, '?')}: {', '.join(t[0] for t in tools[:8])}")
    print(f"자리 없는 서버 {len(left)}개" if left else "모든 서버에 서비스가 있음")


def check():
    services, servers, registry = load()
    bad = []
    areas = {a["id"] for a in services["areas"]}
    by_category = {}
    for a in services["areas"]:
        for c in a["categories"]:
            if c in by_category:
                bad.append(f"분류 {c}가 두 분야({by_category[c]}, {a['id']})에 있음")
            by_category[c] = a["id"]
    known = {}
    for s in services["services"]:
        if s["id"] in known or s["id"] in ("self", "multi"):
            bad.append(f"서비스 id {s['id']}가 두 번 나오거나 예약어임")
        known[s["id"]] = s
        if s["area"] not in areas:
            bad.append(f"{s['id']}: 없는 분야 {s['area']}")
    for sid, v in servers.items():
        if sid not in registry:
            bad.append(f"{sid}: registry.json에 없는 서버")
        if v["service"] in ("self", "multi"):
            if v.get("area") not in areas or not v.get("name"):
                bad.append(f"{sid}: {v['service']}에는 분야와 이름이 있어야 함")
        elif v["service"] not in known:
            bad.append(f"{sid}: 없는 서비스 {v['service']}")
    left = [sid for sid in registry if sid not in servers]
    if left:
        bad.append(f"서비스가 없는 서버 {len(left)}개 (suggest로 보기)")
    categories = collections.Counter(t[1] for tools in registry.values() for t in tools)
    for c, n in categories.items():
        if c is not None and c not in by_category and c != "other":
            bad.append(f"분류 {c}(도구 {n}개)가 어느 분야에도 없음")
    for b in bad:
        print("문제:", b)
    used = collections.Counter(v["service"] for v in servers.values())
    idle = [k for k in known if k not in used]
    print(f"서비스 {len(known)}개(레지스트리에 서버가 없는 것 {len(idle)}개), 분야 {len(areas)}개")
    print(f"서버 {len(servers)}개: 서비스에 묶임 {len(servers) - used['self'] - used['multi']}, 혼자 도는 것(self) {used['self']}, 게이트웨이(multi) {used['multi']}")
    sys.exit(1 if bad else 0)


def first_word(name):
    """A tool name's first word, lowercased: `sendEmail`, `send_email` and `send-email` give `send`."""
    spaced = re.sub(r"(?<=[a-z0-9])(?=[A-Z])", "_", name)
    return re.split(r"[^A-Za-z0-9]+", spaced.strip("_- ."))[0].lower()


def actions():
    by_name = collections.defaultdict(collections.Counter)
    by_verb = collections.defaultdict(collections.Counter)
    for tools in load()[2].values():
        for name, _category, action, _reason in tools:
            by_name[name.lower()][action] += 1
            by_verb[first_word(name)][action] += 1
    # A word is kept when enough tools start with it to trust what most of them do.
    verbs = {v: c.most_common(1)[0][0] for v, c in sorted(by_verb.items()) if v and sum(c.values()) >= 3}
    names = {}
    for name, c in sorted(by_name.items()):
        action = c.most_common(1)[0][0]
        if verbs.get(first_word(name)) != action:
            names[name] = action
    out = DATA / "tool_actions.json"
    out.write_text(json.dumps({"verbs": verbs, "names": names}, ensure_ascii=False, separators=(",", ":")) + "\n")
    print(f"{out}: 첫 단어 {len(verbs)}개, 예외 이름 {len(names)}개, {out.stat().st_size // 1024}KB")


if __name__ == "__main__":
    cmd, *rest = sys.argv[1:] or ["help"]
    if cmd == "slim" and rest:
        slim(rest[0])
    elif cmd == "suggest":
        suggest()
    elif cmd == "check":
        check()
    elif cmd == "actions":
        actions()
    else:
        print(__doc__)
