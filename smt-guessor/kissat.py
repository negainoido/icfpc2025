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
import shutil
import subprocess
import sys
import tempfile
from dataclasses import dataclass
from typing import Dict, List, Tuple, Iterable, Optional



def normalize_plan(plan: str) -> List[int]:
    if any(ch == "0" for ch in plan):
        return [int(ch) for ch in plan]
    return [int(ch) - 1 for ch in plan]


@dataclass
class CNF:
    num_vars: int
    clauses: List[List[int]]

    def add(self, lits: Iterable[int]) -> None:
        self.clauses.append(list(lits))


class CnfBuilder:
    def __init__(self) -> None:
        self.var_counter = 0
        self.clauses: List[List[int]] = []
        # variable maps
        self.u_map: Dict[Tuple[int, int], int] = {}
        self.x_map: Dict[Tuple[int, int, int], int] = {}
        self.b_map: Dict[Tuple[int, int], int] = {}
        self.m_map: Dict[Tuple[int, int], int] = {}

    def new_var(self) -> int:
        self.var_counter += 1
        return self.var_counter

    def lit(self, var: int, sign: bool = True) -> int:
        return var if sign else -var

    # Pair variable U_{i,j} with i<=j
    def var_u(self, i: int, j: int) -> int:
        if i <= j:
            key = (i, j)
        else:
            key = (j, i)
        v = self.u_map.get(key)
        if v is None:
            v = self.new_var()
            self.u_map[key] = v
        return v

    # X variable for plan idx, time t, room k
    def var_x(self, pid: int, t: int, k: int) -> int:
        key = (pid, t, k)
        v = self.x_map.get(key)
        if v is None:
            v = self.new_var()
            self.x_map[key] = v
        return v

    # Label bit B_{k,bit} with bit in {0,1}
    def var_b(self, k: int, bit: int) -> int:
        key = (k, bit)
        v = self.b_map.get(key)
        if v is None:
            v = self.new_var()
            self.b_map[key] = v
        return v

    # Derived room-level matching M_{p,g} = OR_{o in 0..D-1} U_{p, D*g+o}
    def var_m(self, p: int, g: int) -> int:
        key = (p, g)
        v = self.m_map.get(key)
        if v is None:
            v = self.new_var()
            self.m_map[key] = v
        return v

    def add_clause(self, *lits: int) -> None:
        self.clauses.append(list(lits))

    def add_at_least_one(self, vars_list: List[int]) -> None:
        # big OR
        assert len(vars_list) > 0
        self.add_clause(*vars_list)

    def add_at_most_one_sequential(self, vars_list: List[int]) -> None:
        # Sinz sequential encoding: O(n) clauses + (n-1) aux vars
        n = len(vars_list)
        if n <= 1:
            return
        s = [self.new_var() for _ in range(n - 1)]
        # (¬x1 ∨ s1)
        self.add_clause(-vars_list[0], s[0])
        for i in range(1, n - 1):
            # (¬x_{i+1} ∨ s_{i})
            self.add_clause(-vars_list[i], s[i])
            # (¬s_{i-1} ∨ s_{i})
            self.add_clause(-s[i - 1], s[i])
            # (¬x_{i+1} ∨ ¬s_{i-1})
            self.add_clause(-vars_list[i], -s[i - 1])
        # (¬x_n ∨ ¬s_{n-1})
        self.add_clause(-vars_list[-1], -s[-1])

    def add_exactly_one(self, vars_list: List[int]) -> None:
        self.add_at_least_one(vars_list)
        self.add_at_most_one_sequential(vars_list)

    def build(self) -> CNF:
        return CNF(self.var_counter, self.clauses)


def build_cnf(
    plans: List[str],
    results: List[List[int]],
    N: int,
    starting_room: int = 0,
    D: int = 6,
    progress: bool = False,
) -> Tuple[CNF, Dict[str, any]]:
    # Normalize inputs
    norm_plans = [normalize_plan(p) for p in plans]
    for i, (p, r) in enumerate(zip(norm_plans, results)):
        if len(r) != len(p) + 1:
            raise ValueError(
                f"Plan/results length mismatch at {i}: len(plan)={len(p)} vs len(results)={len(r)}"
            )

    P = D * N
    cb = CnfBuilder()

    # Label bits per room (2-bit encoding)
    B = [[cb.var_b(k, b) for b in range(2)] for k in range(N)]

    # Location variables per plan/time/room
    X: List[List[List[int]]] = []
    for pid, (plan, obs) in enumerate(zip(norm_plans, results)):
        T = len(plan)
        x_plan: List[List[int]] = []
        for t in range(T + 1):
            x_t = [cb.var_x(pid, t, k) for k in range(N)]
            cb.add_exactly_one(x_t)
            x_plan.append(x_t)
        # starting room
        cb.add_clause(cb.lit(x_plan[0][starting_room], True))
        X.append(x_plan)

        # label consistency: x[t,k] -> (B[k] == obs[t])
        for t in range(T + 1):
            r = obs[t]
            b0 = r & 1
            b1 = (r >> 1) & 1
            for k in range(N):
                # (¬x ∨ (B0 == b0)) and (¬x ∨ (B1 == b1))
                cb.add_clause(-x_plan[t][k], cb.lit(B[k][0], bool(b0)))
                cb.add_clause(-x_plan[t][k], cb.lit(B[k][1], bool(b1)))

        # transitions: for each t,k and each next room g,
        # x[t,k] ∧ (OR_{q in g} U_{p,q}) -> x[t+1,g]
        # we introduce M_{p,g} to collapse the OR over q
        for t in range(T):
            a_t = plan[t]
            for k in range(N):
                p = D * k + a_t
                for g in range(N):
                    m_pg = cb.var_m(p, g)
                    # Link M_{p,g} <-> OR_{o} U_{p, D*g+o}
                    # (¬M ∨ U1 ∨ ... ∨ U6)
                    u_literals: List[int] = []
                    for o in range(D):
                        q = D * g + o
                        u_literals.append(cb.var_u(min(p, q), max(p, q)))
                        # (¬U -> M): (¬U ∨ M)
                        cb.add_clause(-u_literals[-1], m_pg)
                    cb.add_clause(-m_pg, *u_literals)
                    # (¬x[t,k] ∨ ¬M_{p,g} ∨ x[t+1,g])
                    cb.add_clause(-X[pid][t][k], -m_pg, X[pid][t + 1][g])

    # Port matching constraints: for each port p, exactly one partner q
    for p in range(P):
        vars_list = [cb.var_u(min(p, q), max(p, q)) for q in range(P)]
        cb.add_exactly_one(vars_list)

    cnf = cb.build()
    meta = {
        "N": N,
        "D": D,
        "P": P,
        "plans": norm_plans,
        "results": results,
        "starting_room": starting_room,
        "u_map": cb.u_map,
        "x_map": cb.x_map,
        "b_map": cb.b_map,
        "m_map": cb.m_map,
    }
    if progress:
        print(
            f"[kissat] CNF built: vars={cnf.num_vars}, clauses={len(cnf.clauses)}, U={len(cb.u_map)}, M={len(cb.m_map)}, X={len(cb.x_map)}"
        )
    return cnf, meta


def cnf_to_dimacs(cnf: CNF) -> str:
    lines = [f"p cnf {cnf.num_vars} {len(cnf.clauses)}\n"]
    for cl in cnf.clauses:
        lines.append(" ".join(str(l) for l in cl) + " 0\n")
    return "".join(lines)


def solve_with_kissat(
    cnf: CNF,
    time_limit_s: Optional[float] = None,
    progress: bool = False,
) -> Tuple[str, Dict[int, bool]]:
    """Solve CNF with external 'kissat' binary. Returns (status, assignment). status in {SAT, UNSAT, UNKNOWN}"""
    # Locate kissat binary
    bin_path = shutil.which("kissat")
    if not bin_path:
        # Try alongside current Python (venv bin)
        py_dir = os.path.dirname(sys.executable)
        cand = os.path.join(py_dir, "kissat")
        if os.path.isfile(cand) and os.access(cand, os.X_OK):
            bin_path = cand
    if not bin_path:
        raise RuntimeError(
            "'kissat' binary not found. Install it (e.g., 'brew install kissat' on macOS, 'apt install kissat' on Debian/Ubuntu),\n"
            "or 'pip install passagemath-kissat' (which provides a 'kissat' console script),\n"
            "or pass its path via --kissat-bin /path/to/kissat."
        )

    dimacs = cnf_to_dimacs(cnf)
    with tempfile.TemporaryDirectory() as td:
        cnf_path = os.path.join(td, "problem.cnf")
        with open(cnf_path, "w") as f:
            f.write(dimacs)
        cmd = [bin_path, "-q", cnf_path]
        # Try to pass time limit if supported
        if time_limit_s and time_limit_s > 0:
            # Many builds support '--time=SECONDS'
            cmd = [bin_path, f"--time={int(time_limit_s)}", cnf_path]
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
    b_map: Dict[Tuple[int, int], int] = meta["b_map"]
    u_map: Dict[Tuple[int, int], int] = meta["u_map"]

    # decode labels
    rooms: List[int] = []
    for k in range(N):
        b0 = assign.get(b_map[(k, 0)], False)
        b1 = assign.get(b_map[(k, 1)], False)
        val = (1 if b0 else 0) | ((1 if b1 else 0) << 1)
        rooms.append(val)

    # decode connections
    connections: List[Dict[str, Dict[str, int]]] = []
    for (i, j), var in u_map.items():
        if assign.get(var, False):
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
    starting_room = 0

    if args.progress:
        print(f"[kissat] Building CNF… N={N}, plans={len(plans)}, time={args.time}s")
    cnf, meta = build_cnf(plans, results, N, starting_room=starting_room, progress=args.progress)
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
