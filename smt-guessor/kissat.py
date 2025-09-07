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


STARTING_ROOM = 0


def normalize_plan(plan: str) -> List[int]:
    if any(ch == "0" for ch in plan):
        return [int(ch) for ch in plan]
    return [int(ch) - 1 for ch in plan]


def build_cnf(
    plans: List[str],
    results: List[List[int]],
    N: int,
    D: int = 6,
    progress: bool = False,
) -> Tuple[SatCNF, Dict[str, any]]:
    # Normalize inputs
    norm_plans = [normalize_plan(p) for p in plans]
    for room_id, (p, r) in enumerate(zip(norm_plans, results)):
        if len(r) != len(p) + 1:
            raise ValueError(
                f"Plan/results length mismatch at {room_id}: len(plan)={len(p)} vs len(results)={len(r)}"
            )

    P = D * N
    cnf = SatCNF()
    pool = IDPool()
    port_matching_keys: Set[Tuple[int, int]] = set()
    m_keys: Set[Tuple[int, int]] = set()
    x_keys: Set[Tuple[int, int, int]] = set()

    # Label bits per room (2-bit encoding)
    def label_assign_var(room_id: int, bits: Literal[0, 1, 2, 3]) -> int:
        return pool.id(("L", room_id, bits))

    # Helper for U and M and X
    def port_matching_var(i: int, j: int) -> int:
        port_matching_keys.add((i, j))
        return pool.id(("P", i, j))

    def move_possibility_var(p: int, g: int) -> int:
        m_keys.add((p, g))
        return pool.id(("M", p, g))

    def trace_location_assign_var(pid: int, t: int, k: int) -> int:
        x_keys.add((pid, t, k))
        return pool.id(("T", pid, t, k))
    
    # ---------------- (1) Label constraints ----------------

    label_counts = [0, 0, 0, 0]
    r0_bits = results[0][0]
    cnf.append([label_assign_var(STARTING_ROOM, r0_bits)])
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
        else:
            # In this case, room i should have one of {0, 1, 2, 3} as its label
            print("ROOM ", room_id, "can be anything")
            lits = [label_assign_var(room_id, bits) for bits in range(4)]
            CardEnc.equals(lits=lits, bound=1, vpool=pool, encoding=EncType.seqcounter, top_id=pool.top)
            cnf.extend(CardEnc.equals(lits=lits, bound=1, vpool=pool, encoding=EncType.seqcounter).clauses)

    # print the number of variables and clauses created for label constraints
    print(f"[kissat] Label constraints: vars={pool.top}, clauses={len(cnf.clauses)}")    
    num_label_vars = pool.top
    num_label_clauses = len(cnf.clauses)


    # ---------------- (2) Port matching constraints ----------------
    # Port matching constraints: for each port p, exactly one partner q
    p_var_dict = dict()
    for p in range(P):
        for q in range(p):
            var = port_matching_var(p, q)
            p_var_dict[(p, q)] = var
            p_var_dict[(q, p)] = var

    for p in range(P):
        vars_list = [p_var_dict[(p, q)] for q in range(P) if q != p]
        enc = CardEnc.equals(lits=vars_list, bound=1, vpool=pool, encoding=EncType.seqcounter)
        cnf.extend(enc.clauses)

    # print the number of variables and clauses created for port matching constraints
    print(f"[kissat] Port matching constraints: vars={pool.top - num_label_vars}, clauses={len(cnf.clauses) - num_label_clauses}")


    # ---------------- (3) Trace constraints ----------------
    # Location variables per plan/time/room
    X: List[List[List[int]]] = []
    for trace_id, (plan, obs) in enumerate(zip(norm_plans, results)):
        print(trace_id)

        T = len(plan)
        x_plan: List[List[int]] = []
        for t in range(T + 1):
            x_t = [trace_location_assign_var(trace_id, t, k) for k in range(N)]
            # exactly one via PySAT encoder
            enc = CardEnc.equals(lits=x_t, bound=1, vpool=pool, encoding=EncType.seqcounter)
            cnf.extend(enc.clauses)
            x_plan.append(x_t)
        # starting room
        cnf.append([x_plan[0][STARTING_ROOM]])
        X.append(x_plan)

        # label consistency: x[t,k] -> (Label[k] == obs[t])
        for t in range(T + 1):
            for k in range(N):
                cnf.append([-x_plan[t][k], label_assign_var(k, obs[t])])

        # transitions: for each t,k and each next room g,
        # x[t,k] ∧ (OR_{q in g} U_{p,q}) -> x[t+1,g]
        # we introduce M_{p,g} to collapse the OR over q
        for t in range(T):
            a_t = plan[t]
            for k in range(N):
                p = D * k + a_t
                for g in range(N):
                    m_pg = move_possibility_var(p, g)
                    # Link M_{p,g} <-> OR_{o} U_{p, D*g+o}
                    # (¬M ∨ U1 ∨ ... ∨ U6)
                    u_literals: List[int] = []
                    for o in range(D):
                        q = D * g + o
                        u_literals.append(p_var_dict[(p, q)])
                        # (¬U -> M): (¬U ∨ M)
                        cnf.append([-u_literals[-1], m_pg])
                    cnf.append([-m_pg] + u_literals)
                    # x[t,k] ∧ x[t+1,g] -> m_pg
                    cnf.append([-X[trace_id][t][k], -X[trace_id][t + 1][g], m_pg])

                    # x[t,k] ∧ m_pg -> x[t+1,g]
                    cnf.append([-X[trace_id][t][k], -m_pg, X[trace_id][t + 1][g]])



    # Ensure nv is at least the top variable id
    cnf.nv = max(getattr(cnf, 'nv', 0) or 0, pool.top)
    meta = {
        "N": N,
        "D": D,
        "P": P,
        "plans": norm_plans,
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
        print(k, [(pool.id(("L", k, bits)), assign.get(pool.id(("L", k, bits)), False)) for bits in range(4)])
        assert val is not None, f"Room {k} has no label assigned"
        rooms.append(val)

    # decode connections
    connections: List[Dict[str, Dict[str, int]]] = []
    for (i, j) in port_matching_keys:
        if assign.get(pool.id(("P", i, j)), False):
            ri, di = divmod(i, D)
            rj, dj = divmod(j, D)
            connections.append(
                {
                    "from": {"room": ri, "door": di},
                    "to": {"room": rj, "door": dj},
                }
            )
    return {
        "status": 1 if connections else 0,
        "rooms": rooms,
        "startingRoom": starting_room,
        "connections": connections,
    }


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

    plans = data["plans"]
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

    if args.output:
        with open(args.output, "w") as f:
            json.dump(out, f, ensure_ascii=False, indent=2)
    else:
        print(json.dumps(out, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
