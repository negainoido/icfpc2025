#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Ædificium SAT solver via Kissat
- Encodes the same problem as cpsat.build_solver_fast but as CNF DIMACS
- Solves using the external 'kissat' binary (if available)

Usage: python -m smt-guessor.kissat --input in.json --output out.json [--time 600]
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import tempfile
from typing import Dict, List, Literal, Tuple, Optional, Set

from pysat.formula import CNF as SatCNF, IDPool
from pysat.card import CardEnc, EncType


STARTING_ROOM_ID = 0


def normalize_plan(plan: str) -> List[int]:
    return [int(ch) for ch in plan]


def build_cnf(
    plans: List[List[int]],
    results: List[List[int]],
    N: int,
    D: int = 6,
    progress: bool = False,
) -> Tuple[SatCNF, Dict[str, any]]:
    # Normalize inputs
    for room_id, (from_pid, r) in enumerate(zip(plans, results)):
        if len(r) != len(from_pid) + 1:
            raise ValueError(
                f"Plan/results length mismatch at {room_id}: len(plan)={len(from_pid)} vs len(results)={len(r)}"
            )

    P = D * N
    cnf = SatCNF()
    pool = IDPool()
    port_matching_keys: Set[Tuple[int, int]] = set()
    m_keys: Set[Tuple[int, int]] = set()
    x_keys: Set[Tuple[int, int, int]] = set()

    # Label bits per room (2-bit encoding)
    def label_assign_var(rid: int, bits: Literal[0, 1, 2, 3]) -> int:
        return pool.id(("L", rid, bits))

    # Helper for U and M and X
    def port_matching_var(pid0: int, pid1: int) -> int:
        port_matching_keys.add((pid0, pid1))
        return pool.id(("P", pid0, pid1))

    def move_possibility_var(pid: int, rid: int) -> int:
        m_keys.add((pid, rid))
        return pool.id(("M", pid, rid))

    def trace_location_assign_var(tid: int, timestamp: int, rid: int) -> int:
        x_keys.add((tid, timestamp, rid))
        return pool.id(("T", tid, timestamp, rid))
    
    # ---------------- (1) Label constraints ----------------

    # For all room, exactly one label in {0, 1, 2, 3}
    for rid in range(N):
        lits = [label_assign_var(rid, bits) for bits in range(4)]
        enc = CardEnc.equals(lits=lits, bound=1, vpool=pool, encoding=EncType.seqcounter)
        cnf.extend(enc.clauses)

    # For the starting room, fix the label to results[0][0]
    r0_bits = results[0][0]
    cnf.append([label_assign_var(STARTING_ROOM_ID, r0_bits)])

    # Balance the labels: each label should appear at least N/4 times
    label_counts = [0, 0, 0, 0]
    label_counts[r0_bits] += 1
    min_label_count = N // 4
    for room_id in range(1, N):
        label_to_assign = None
        for bits in range(4):
            if label_counts[bits] < min_label_count:
                label_to_assign = bits
                break
        if label_to_assign is not None:
            cnf.append([label_assign_var(room_id, label_to_assign)])
            label_counts[label_to_assign] += 1


    # print the number of variables and clauses created for label constraints
    if progress:
        print(f"[kissat] Label constraints: vars={pool.top}, clauses={len(cnf.clauses)}")
    num_label_vars = pool.top
    num_label_clauses = len(cnf.clauses)


    # ---------------- (2) Port matching constraints ----------------
    # Port matching constraints: for each port p, exactly one partner q
    port_matching_var_dict = dict()
    for to_pid in range(P):
        for from_pid in range(to_pid + 1):
            var = port_matching_var(from_pid, to_pid)
            port_matching_var_dict[(from_pid, to_pid)] = var
            port_matching_var_dict[(to_pid, from_pid)] = var

    for from_pid in range(P):
        vars_list = [port_matching_var_dict[(from_pid, q)] for q in range(P)]
        enc = CardEnc.equals(lits=vars_list, bound=1, vpool=pool, encoding=EncType.seqcounter)
        cnf.extend(enc.clauses)

    # print the number of variables and clauses created for port matching constraints
    if progress:
        print(f"[kissat] Port matching constraints: vars={pool.top - num_label_vars}, clauses={len(cnf.clauses) - num_label_clauses}")


    # ---------------- (3) Trace constraints ----------------

    # Prepare variable representing OR_{q in g} U_{p,q} (M_{p,g}) in advance
    for from_pid in range(P):
        from_rid, _ = divmod(from_pid, D)
        for to_rid in range(N):
            m = move_possibility_var(from_pid, to_rid)
            u_literals: List[int] = []
            for d in range(D):
                to_pid = D * to_rid + d
                u_literals.append(port_matching_var_dict[(from_pid, to_pid)])
                # # (¬U -> M): (¬U ∨ M)
                cnf.append([-u_literals[-1], m])
            # Link M_{p,g} <-> OR_{o} U_{p, D*g+o}
            # (¬M ∨ U1 ∨ ... ∨ U6)
            cnf.append([-m] + u_literals)


    # Location variables per plan/time/room
    X: List[List[List[int]]] = []
    for trace_id, (plan, obs) in enumerate(zip(plans, results)):
        print(trace_id, plan, obs)

        T = len(plan)
        x_plan: List[List[int]] = []
        for t in range(T + 1):
            x_t = [trace_location_assign_var(trace_id, t, rid) for rid in range(N)]
            # exactly one via PySAT encoder
            enc = CardEnc.equals(lits=x_t, bound=1, vpool=pool, encoding=EncType.seqcounter)
            cnf.extend(enc.clauses)
            x_plan.append(x_t)
        X.append(x_plan)

        # label consistency: x[t,k] -> (Label[k] == obs[t])
        for t in range(T + 1):
            for rid in range(N):
                assert obs[t] in (0, 1, 2, 3), f"Invalid observation {obs[t]} at plan {trace_id}, time {t}"
                cnf.append([-x_plan[t][rid], label_assign_var(rid, obs[t])])


        # starting room
        cnf.append([x_plan[0][STARTING_ROOM_ID]])

        # transitions: for each t,k and each next room g,
        # x[t,k] ∧ (OR_{q in g} U_{p,q}) -> x[t+1,g]
        # we introduce M_{p,g} to collapse the OR over q
        for t in range(T):
            door = plan[t]
            assert 0 <= door < D, f"Invalid action {door} at plan {trace_id}, time {t}"

            for from_rid in range(N):
                from_pid = D * from_rid + door
                for to_rid in range(N):
                    m = move_possibility_var(from_pid, to_rid)
                    cnf.append([-X[trace_id][t][from_rid], -m, X[trace_id][t + 1][to_rid]])
                    # cnf.append([-X[trace_id][t][from_rid], -X[trace_id][t + 1][to_rid]], m)



    # Ensure nv is at least the top variable id
    cnf.nv = max(getattr(cnf, 'nv', 0) or 0, pool.top)
    meta = {
        "N": N,
        "D": D,
        "P": P,
        "plans": plans,
        "results": results,
        "starting_room": 0,
        "pool": pool,
        "port_matching_keys": port_matching_keys,
    }
    if progress:
        print(f"[kissat] CNF built: vars~{pool.top}, clauses={len(cnf.clauses)}, U={len(port_matching_keys)}, M={len(m_keys)}, X={len(x_keys)}")
    return cnf, meta


def solve_with_kissat(
    cnf: SatCNF,
    time_limit_s: Optional[float] = None,
    progress: bool = False,
) -> Tuple[str, Dict[int, bool]]:
    """Solve CNF with external 'kissat' binary. Returns (status, assignment). status in {SAT, UNSAT, UNKNOWN}"""

    with tempfile.TemporaryDirectory() as td:
        cnf_path = os.path.join(td, "problem.cnf")
        # write DIMACS using PySAT utility
        cnf.to_file(cnf_path)
        cmd = ["kissat", "-q", cnf_path]
        # Try to pass time limit if supported
        if time_limit_s and time_limit_s > 0:
            # Many builds support '--time=SECONDS'
            cmd = ["kissat", f"--time={int(time_limit_s)}", cnf_path]
        if progress:
            print("[kissat] Running:", " ".join(cmd))
        try:
            out = subprocess.run(cmd, capture_output=True, text=True, check=False)
        except Exception as e:
            raise RuntimeError(f"Failed to run kissat: {e}")

        stdout = out.stdout
        stderr = out.stderr
        if progress and stderr.strip():
            print("[kissat] stderr:\n" + stderr)

    status = "UNKNOWN"
    if "UNSATISFIABLE" in stdout:
        status = "UNSAT"
        return status, {}
    if "SATISFIABLE" in stdout:
        status = "SAT"
    # parse 'v ' model lines
    model_lits: List[int] = []
    for line in stdout.splitlines():
        print(line)
        line = line.strip()
        if line.startswith("v ") or line.startswith("V "):
            parts = line[1:].split()
            for tok in parts:
                try:
                    v = int(tok)
                except ValueError:
                    continue
                if v == 0:
                    continue
                model_lits.append(v)
    assign: Dict[int, bool] = {}
    for lit in model_lits:
        var = abs(lit)
        val = lit > 0
        assign[var] = val
    return status, assign


def extract_solution(meta: Dict[str, any], assign: Dict[int, bool]) -> Dict[str, any]:
    N = meta["N"]
    D = meta["D"]
    starting_room = meta["starting_room"]
    pool: IDPool = meta["pool"]
    port_matching_keys: Set[Tuple[int, int]] = meta["port_matching_keys"]

    # decode labels
    rooms: List[int] = []
    for k in range(N):
        val = None
        for bits in range(4):
            if assign.get(pool.id(("L", k, bits)), False):
                val = bits
                break
        assert val is not None, f"Room {k} has no label assigned"
        rooms.append(val)


    # decode connections
    connections: List[Dict[str, Dict[str, int]]] = []
    for (i, j) in port_matching_keys:
        if i <= j and assign.get(pool.id(("P", i, j)), False):
            ri, di = divmod(i, D)
            rj, dj = divmod(j, D)
            connections.append(
                {
                    "from": {"room": ri, "door": di},
                    "to": {"room": rj, "door": dj},
                }
            )
    connections.sort(key=lambda e: (e["from"]["room"], e["from"]["door"], e["to"]["room"], e["to"]["door"]))
    return {
        "status": 1 if connections else 0,
        "rooms": rooms,
        "startingRoom": starting_room,
        "connections": connections,
    }


def verify_solution(plans: List[List[int]], results: List[List[int]], N: int, out: Dict[str, any], progress: bool = True) -> Tuple[bool, List[str]]:
    D = 6
    P = N * D
    errs: List[str] = []

    # Build port matching map p -> q from connections
    match = [-1] * P
    for c in out.get("connections", []):
        ri = int(c["from"]["room"])
        di = int(c["from"]["door"]) 
        rj = int(c["to"]["room"])
        dj = int(c["to"]["door"]) 
        p = ri * D + di
        q = rj * D + dj
        if not (0 <= p < P and 0 <= q < P):
            errs.append(f"invalid connection port index: p={p}, q={q}")
            continue
        if match[p] != -1 and match[p] != q:
            errs.append(f"port {p} matched twice: {match[p]} and {q}")
        if match[q] != -1 and match[q] != p:
            errs.append(f"port {q} matched twice: {match[q]} and {p}")
        match[p] = q
        match[q] = p

    # Check all ports matched
    for p in range(P):
        if match[p] == -1:
            ri, di = divmod(p, D)
            errs.append(f"unmatched port: room={ri}, door={di}")

    labels = out.get("rooms", [])
    if len(labels) != N:
        errs.append(f"rooms label length mismatch: got {len(labels)} expected {N}")

    # Simulate each plan
    for idx, (plan, obs) in enumerate(zip(plans, results)):
        # start
        cur = 0
        if not (0 <= cur < N):
            errs.append(f"plan {idx}: starting room out of range: {cur}")
            continue
        if labels and (labels[cur] != int(obs[0])):
            errs.append(f"plan {idx} step 0: label mismatch at room {cur}: expected {obs[0]}, got {labels[cur]}")
        # steps
        for t, a in enumerate(plan):
            if not (0 <= a < D):
                errs.append(f"plan {idx} step {t}: action out of range: {a}")
                break
            p = cur * D + a
            if match[p] == -1:
                errs.append(f"plan {idx} step {t}: port {p} (room {cur}, door {a}) has no match")
                break
            q = match[p]
            nxt = q // D
            cur = nxt
            expected = int(obs[t + 1])
            if labels and (labels[cur] != expected):
                errs.append(
                    f"plan {idx} step {t+1}: label mismatch at room {cur}: expected {expected}, got {labels[cur]}"
                )

    ok = len(errs) == 0
    if progress:
        if ok:
            print("[verify] All plans consistent with solution.")
        else:
            print(f"[verify] Found {len(errs)} issues:")
            for m in errs:
                print(" -", m)
    return ok, errs


def main() -> None:
    parser = argparse.ArgumentParser(description="Ædificium SAT solver via Kissat")
    parser.add_argument("--input", "-i", type=str, help="Input JSON (stdin if omitted)")
    parser.add_argument("--output", "-o", type=str, help="Output JSON (stdout if omitted)")
    parser.add_argument("--time", type=float, default=600.0, help="Time limit in seconds")
    parser.add_argument("--progress", "-p", action="store_true", help="Print progress logs")
    args = parser.parse_args()

    if args.input:
        with open(args.input, "r") as f:
            data = json.load(f)
    else:
        data = json.load(sys.stdin)

    plans = [normalize_plan(p) for p in data["plans"]]
    results = data["results"]
    N = data["N"]

    if args.progress:
        print(f"[kissat] Building CNF… N={N}, plans={len(plans)}, time={args.time}s")
    cnf, meta = build_cnf(plans, results, N, progress=args.progress)
    if args.progress:
        print("[kissat] Solving…")
    status, assign = solve_with_kissat(
        cnf,
        time_limit_s=args.time,
        progress=args.progress,
    )
    if args.progress:
        print(f"[kissat] Solve status: {status}")

    if status != "SAT":
        out = {"status": 0, "error": f"Kissat returned {status}"}
    else:
        out = extract_solution(meta, assign)
        # Post-verify the solution against input plans/results
        ok, errs = verify_solution(plans, results, N, out, progress=args.progress)
        out["verified"] = bool(ok)
        if not ok:
            out["verifyErrors"] = errs

    if args.output:
        with open(args.output, "w") as f:
            json.dump(out, f, ensure_ascii=False, indent=2)
    else:
        print(json.dumps(out, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
