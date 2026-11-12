#!/usr/bin/env python3
"""Turns a viewer fixture (the Neo4j-era API's answers) into INSERTs for the engine's PG schema,
so the viewer's PG queries can be checked against the demo (src/pg.rs, the_store_answers_as_the_demo).

    python3 scripts/fixture-to-pg.py mock/fixture.json | psql "$DATABASE_URL"
"""

import hashlib
import json
import sys

NODE_COLUMNS = [
    "event_name", "observed_at", "client_id", "sequence", "prompt_id", "agent_id", "agent_type", "cwd",
    "tool_use_id", "turn_id", "repository", "intent", "subgraph", "payload_json", "decision",
    "decision_reason", "decision_rule", "decision_start", "decision_version", "decision_mode",
    "decision_jev_json", "decided_at", "parent", "parents", "trace_json", "intent_score", "trace_error",
]


def lit(v):
    if v is None:
        return "NULL"
    if isinstance(v, bool):
        return "true" if v else "false"
    if isinstance(v, (int, float)):
        return repr(v)
    if isinstance(v, list):
        return "ARRAY[" + ",".join(lit(x) for x in v) + "]::text[]" if v else "'{}'::text[]"
    return "'" + str(v).replace("'", "''") + "'"


def insert(table, row):
    cols = ", ".join(f'"{k}"' for k in row)
    vals = ", ".join(lit(v) for v in row.values())
    return f"INSERT INTO {table} ({cols}) VALUES ({vals});"


def main():
    data = json.load(open(sys.argv[1]))
    out = ["BEGIN;"]
    for s in data["sessions"]:
        out.append(insert("sessions", {"session_id": s["id"], "version": s.get("version") or 0, "created_at": s.get("created_at")}))
    for sid, g in data["graphs"].items():
        nodes = {n["props"]["seq"]: n for n in g["nodes"]}
        nxt = {}
        tool_use = {}
        for e in g["edges"]:
            if e["type"] == "NEXT" and e["props"].get("kind") != "cause":
                nxt[e["target"]] = (e["source"], e["props"].get("via"))
            if e["type"] == "RESULT_OF":
                call = nodes.get(e["target"])
                if call and call["props"].get("tool_use_id"):
                    tool_use[e["source"]] = call["props"]["tool_use_id"]
        for seq, n in nodes.items():
            p = n["props"]
            row = {"session_id": sid, "seq": seq, "kind": n["kind"], "order": p["order"]}
            for c in NODE_COLUMNS:
                if c in p and p[c] is not None:
                    row[c] = p[c]
            row.setdefault("event_name", p.get("event_name") or n["kind"])
            if seq in tool_use and not row.get("tool_use_id"):
                row["tool_use_id"] = tool_use[seq]
            if seq in nxt:
                row["next_seq"], row["next_via"] = nxt[seq]
            out.append(insert("nodes", row))
        for sg in g.get("subgraphs", []):
            p = sg["props"]
            row = {"session_id": sid, "number": p["number"]}
            for c in ("parent", "parents", "parent_call_seq", "parent_call_order"):
                if p.get(c) is not None:
                    row[c] = p[c]
            out.append(insert("subgraphs", row))
        for e in g["edges"]:
            if e["type"] == "NEXT" and e["props"].get("kind") == "cause":
                p = e["props"]
                row = {"session_id": sid, "call_seq": e["source"], "target_seq": e["target"],
                       "target_index": e["paragraph"] if e.get("paragraph") is not None else -1}
                for c in ("probability", "rank", "parent"):
                    if p.get(c) is not None:
                        row[c] = p[c]
                out.append(insert("cause_edges", row))
        seen = set()
        for t in g.get("tokens", []):
            node = nodes.get(t["seq"])
            k = (t["kind"], t["key"], t["slot"])
            if k not in seen:
                seen.add(k)
                out.append(insert("tokens", {"session_id": sid, "key_kind": t["kind"], "key": t["key"], "slot": t["slot"],
                                             "node_seq": t["seq"], "node_order": node["props"]["order"] if node else 0,
                                             "decoded": t.get("text"), "category": t.get("category")}))
            mention = insert("token_mentions", {"session_id": sid, "node_seq": t["seq"], "key_kind": t["kind"],
                                                "key": t["key"], "slot": t["slot"]})
            out.append(mention[:-1] + " ON CONFLICT DO NOTHING;")
        for seq, detail in data.get("nodes", {}).get(sid, {}).items():
            for para in detail.get("paragraphs") or []:
                row = {"session_id": sid, "seq": int(seq), "index": para["index"],
                       "hash": hashlib.sha256((para.get("text") or "").encode()).hexdigest()}
                for c in ("text", "source", "hidden", "hidden_rules", "instructs"):
                    if para.get(c) is not None:
                        row[c] = para[c]
                out.append(insert("paragraphs", row))
    out.append("COMMIT;")
    print("\n".join(out))


if __name__ == "__main__":
    main()
