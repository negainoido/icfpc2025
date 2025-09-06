#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Ædificium CP-SAT solver (port-complete-matching + trace constraints, A-form)
- Input: JSON with {"plans": [...], "results": [...], "N": 64, "startingRoom": 0}
- Output: JSON with {"rooms": [...], "startingRoom": 0, "connections": [...]}

Requires: pip install ortools
"""

import json
import sys
import argparse
from typing import List, Tuple, Dict, Any

from ortools.sat.python import cp_model


def normalize_plan(plan: str) -> List[int]:
    """Convert a route plan string to a list of ints in 0..5.
    Accepts digits '0'-'5' or '1'-'6'. If any '0' appears, we assume 0-based already.
    """
    if any(ch == "0" for ch in plan):
        return [int(ch) for ch in plan]
    # assume 1-6
    return [int(ch) - 1 for ch in plan]


def build_solver(
    plans: List[str],
    results: List[List[int]],
    N: int,
    starting_room: int = 0,
    use_exactly_one_location: bool = True,
) -> Tuple[cp_model.CpModel, Dict[str, Any]]:
    """Build the CP-SAT model using port-complete-matching + trace constraints (A-form)."""
    model = cp_model.CpModel()

    # ---------- Normalize inputs ----------
    norm_plans = [normalize_plan(p) for p in plans]
    # check lengths
    for i, (p, r) in enumerate(zip(norm_plans, results)):
        if len(r) != len(p) + 1:
            raise ValueError(
                f"Plan/results length mismatch at index {i}: plan '{plans[i]}' "
                f"(len={len(p)}) vs results len={len(r)}; must be len(plan)+1."
            )

    D = 6  # doors per room
    P = D * N  # number of ports

    # ---------- Variables ----------
    # Pair variables for complete matching over ports (unordered pairs, including self-pairs for loops).
    pair_vars: Dict[Tuple[int, int], cp_model.IntVar] = {}
    for i in range(P):
        for j in range(i, P):
            pair_vars[(i, j)] = model.NewBoolVar(f"pair_{i}_{j}")

    # For each port p, collect all pair vars that include p, and enforce exactly-one
    pairs_inc = [[] for _ in range(P)]
    for (i, j), var in pair_vars.items():
        pairs_inc[i].append(var)
        if i != j:
            pairs_inc[j].append(var)

    for p in range(P):
        model.AddExactlyOne(pairs_inc[p])

    # Room label variables: L_k in {0,1,2,3}
    L = [model.NewIntVar(0, 3, f"L_{k}") for k in range(N)]

    # Location variables for each plan and time: x[plan_idx][t][k] is True if at time t we are in room k
    x: List[List[List[cp_model.BoolVar]]] = []
    for idx, (plan, obs) in enumerate(zip(norm_plans, results)):
        T = len(plan)
        x_plan: List[List[cp_model.BoolVar]] = []
        for t in range(T + 1):
            x_t = [model.NewBoolVar(f"x_{idx}_{t}_{k}") for k in range(N)]
            if use_exactly_one_location:
                model.AddExactlyOne(x_t)
            x_plan.append(x_t)
        x.append(x_plan)

        # starting room
        model.Add(x_plan[0][starting_room] == 1)

        # label consistency: x[t,k] -> L_k == obs[t]
        for t in range(T + 1):
            ot = obs[t]
            for k in range(N):
                model.Add(L[k] == ot).OnlyEnforceIf(x_plan[t][k])

        # trace constraints (A-form): (x[t,k] & x[t+1,g]) -> OR_{q in B(g)} pair( id(k, a_t), q )
        for t in range(T):
            a_t = plan[t]
            for k in range(N):
                p = D * k + a_t
                for g in range(N):
                    bucket_q_vars = []
                    for c in range(D):
                        q = D * g + c
                        i, j = (p, q) if p <= q else (q, p)
                        bucket_q_vars.append(pair_vars[(i, j)])
                    model.AddBoolOr(bucket_q_vars).OnlyEnforceIf(
                        [x_plan[t][k], x_plan[t + 1][g]]
                    )

    meta = {
        "pair_vars": pair_vars,
        "labels": L,
        "N": N,
        "D": D,
        "plans": norm_plans,
        "results": results,
        "x": x,
        "starting_room": starting_room,
    }
    return model, meta


def solve_and_extract(
    model: cp_model.CpModel,
    meta: Dict[str, Any],
    time_limit_s: float = 60.0,
    progress: bool = False,
) -> Dict[str, Any]:
    """Solve the CP-SAT model and extract the requested map as a JSON-serializable dict."""
    solver = cp_model.CpSolver()
    if time_limit_s is not None and time_limit_s > 0:
        solver.parameters.max_time_in_seconds = time_limit_s
    solver.parameters.num_search_workers = 8

    # Optional progress logging
    if progress:
        # Print solver search progress to stdout.
        # EnableOutput() typically implies log_search_progress.
        try:
            solver.EnableOutput()
        except Exception:
            # Fallback for older versions if EnableOutput is unavailable.
            solver.parameters.log_search_progress = True

    status = solver.Solve(model)
    if status not in (cp_model.OPTIMAL, cp_model.FEASIBLE):
        return {
            "status": int(status),
            "error": "No solution found by CP-SAT within limits.",
        }

    N = meta["N"]
    D = meta["D"]
    pair_vars = meta["pair_vars"]
    labels = meta["labels"]
    starting_room = meta["starting_room"]

    rooms = [int(solver.Value(Lk)) for Lk in labels]

    connections = []
    for (i, j), var in pair_vars.items():
        if solver.Value(var) == 1:
            room_i, door_i = divmod(i, D)
            room_j, door_j = divmod(j, D)
            connections.append(
                {
                    "from": {"room": room_i, "door": door_i},
                    "to": {"room": room_j, "door": door_j},
                }
            )

    return {
        "status": int(status),
        "rooms": rooms,
        "startingRoom": starting_room,
        "connections": connections,
    }


def main():
    parser = argparse.ArgumentParser(
        description="Ædificium CP-SAT solver (port matching + trace constraints)"
    )
    parser.add_argument(
        "--input",
        "-i",
        type=str,
        help="Path to input JSON. If omitted, read from stdin.",
    )
    parser.add_argument(
        "--output",
        "-o",
        type=str,
        help="Path to write output JSON. If omitted, print to stdout.",
    )
    parser.add_argument(
        "--N",
        type=int,
        help="Override number of rooms (if omitted, use input JSON's N).",
    )
    parser.add_argument(
        "--start", type=int, default=None, help="Override startingRoom (default 0)"
    )
    parser.add_argument(
        "--time", type=float, default=600.0, help="Time limit in seconds (default 60)"
    )
    parser.add_argument(
        "--progress",
        "-p",
        action="store_true",
        help="Print CP-SAT search progress and build milestones.",
    )
    args = parser.parse_args()

    if args.input:
        with open(args.input, "r") as f:
            data = json.load(f)
    else:
        data = json.load(sys.stdin)

    plans = data["plans"]
    results = data["results"]
    N = args.N if args.N is not None else data.get("N", None)
    if N is None:
        raise SystemExit(
            "N (number of rooms) must be provided either in input JSON or via --N"
        )
    starting_room = (
        args.start if args.start is not None else data.get("startingRoom", 0)
    )

    if args.progress:
        print(
            f"[cpsat] Building model… N={N}, plans={len(plans)}, time_limit={args.time}s"
        )
    model, meta = build_solver(plans, results, N, starting_room=starting_room)
    if args.progress:
        print("[cpsat] Model built. Solving…")
    out = solve_and_extract(model, meta, time_limit_s=args.time, progress=args.progress)
    if args.progress:
        print("[cpsat] Solve finished.")

    if args.output:
        with open(args.output, "w") as f:
            json.dump(out, f, ensure_ascii=False, indent=2)
    else:
        print(json.dumps(out, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
