#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
ICFP 2025 Ædificium: SMT-based map reconstruction

Input JSON schema (example):
{
  "plans": ["0325", "510"],           # route plan strings (digits 0-5 or 1-6)
  "results": [[0,2,1,3,0], [0,1,0]],    # labels include the starting room; length = len(plan)+1
  "N": 64,                               # (optional) number of rooms; if omitted, --minN/--maxN sweep
  "startingRoom": 0                      # (optional) default 0
}

Output (printed to stdout or written via --output):
{
  "rooms": [int,...],                    # λ(q) values for q=0..N-1
  "startingRoom": 0,
  "connections": [
    {"from":{"room":q,"door":c}, "to":{"room":q2,"door":e}}, ...
  ]
}

Notes:
- Accepts plans using digits 0..5 or 1..6 (auto-normalized to 0..5).
- SMT encoding uses a Port datatype and two functions:
    delta : Port -> Port  (involution on ports)
    label : Int  -> (_ BitVec 2)
- Traces include the starting-room label and then one label per move:
    for j=0..L, label(x[i][j]) == results[i][j]  (L = len(plan))
- Requires: `pip install z3-solver`
"""
from __future__ import annotations
import json
import argparse
import sys
from typing import List, Tuple, Dict, Any, Optional

from z3 import (
    Solver,
    IntSort,
    IntVal,
    And,
    Or,
    Not,
    BoolVal,
    BitVecSort,
    BitVecVal,
    Function,
    ForAll,
    Datatype,
    simplify,
    sat,
    set_option,
)

# ----------------------------- helpers -----------------------------


def inQ(q, N):
    return And(IntVal(0) <= q, q < IntVal(N))


def inDoor(d):
    return And(IntVal(0) <= d, d <= IntVal(5))


def normalize_plan(p: str) -> List[int]:
    """Convert a plan string to list[int] doors in 0..5. Supports '0-5' or '1-6'."""
    if all(ch in "012345" for ch in p):
        return [int(ch) for ch in p]
    if all(ch in "123456" for ch in p):
        return [int(ch) - 1 for ch in p]
    raise ValueError(f"Plan contains invalid digits (expect 0-5 or 1-6): {p}")


def parse_input(
    data: Dict[str, Any],
) -> Tuple[List[List[int]], List[List[int]], Optional[int], int]:
    plans_s: List[str] = data.get("plans", [])
    results: List[List[int]] = data.get("results", [])
    if len(plans_s) != len(results):
        raise ValueError("plans and results must have the same length")
    plans = [normalize_plan(p) for p in plans_s]
    for i, (p, r) in enumerate(zip(plans, results)):
        if len(r) != len(p) + 1:
            raise ValueError(
                f"results must include starting label: expected {len(p)+1}, got {len(r)} at index {i}"
            )
        for val in r:
            if not (0 <= int(val) <= 3):
                raise ValueError(f"results[{i}] contains non 2-bit value: {val}")
    N = data.get("N")
    starting = int(data.get("startingRoom", 0))
    return plans, results, N, starting


# ----------------------------- core solver -----------------------------


def solve_with_N(
    N: int,
    plans: List[List[int]],
    results: List[List[int]],
    starting_room: int = 0,
    check_unique: bool = False,
) -> Tuple[Dict[int, int], Dict[Tuple[int, int], Tuple[int, int]], Optional[bool]]:
    """Return (labels, delta_map, is_unique) where
       labels[q] -> int in 0..3,  delta_map[(q,c)] -> (q2,e).
    Raises AssertionError if UNSAT.
    """
    # Build SMT objects
    PortDT = Datatype("Port")
    PortDT.declare("mkP", ("room", IntSort()), ("door", IntSort()))
    Port = PortDT.create()

    mkP = Port.mkP
    room_of = Port.room
    door_of = Port.door

    delta = Function("delta", Port, Port)
    label = Function("label", IntSort(), BitVecSort(2))

    s = Solver()

    # Involution and guard constraints: for all q in [0..N-1], c in [0..5]
    for q in range(N):
        for c in range(6):
            p = mkP(IntVal(q), IntVal(c))
            p2 = delta(p)
            # Guard codomain
            s.add(inQ(room_of(p2), N))
            s.add(inDoor(door_of(p2)))
            # Involution: delta(delta(p)) == p
            s.add(delta(p2) == p)

    # Trace constraints
    for i, (plan, outs) in enumerate(zip(plans, results)):
        L = len(plan)
        # create Int variables x_{i,j}
        from z3 import Int

        xs = [Int(f"x_{i}_{j}") for j in range(L + 1)]
        # init
        s.add(xs[0] == IntVal(starting_room))
        s.add(inQ(xs[0], N))
        # steps
        for j, c in enumerate(plan):
            s.add(inQ(xs[j], N))
            p = mkP(xs[j], IntVal(c))
            next_room = room_of(delta(p))
            s.add(xs[j + 1] == next_room)
            s.add(inQ(xs[j + 1], N))
        # labels: include starting room and each subsequent state
        for j in range(L + 1):
            s.add(label(xs[j]) == BitVecVal(int(outs[j]), 2))

    # Solve
    if s.check() != sat:
        raise AssertionError(
            "UNSAT with given N; try a larger N or check input traces."
        )
    m = s.model()

    is_unique = None
    if check_unique:
        s.push()
        blocking_clauses = []
        for q in range(N):
            blocking_clauses.append(label(IntVal(q)) != m.eval(label(IntVal(q))))
        for q in range(N):
            for c in range(6):
                p = mkP(IntVal(q), IntVal(c))
                blocking_clauses.append(delta(p) != m.eval(delta(p)))
        s.add(Or(blocking_clauses))
        is_unique = s.check() != sat
        s.pop()

    # Extract label map
    labels: Dict[int, int] = {}
    for q in range(N):
        lv = m.eval(label(IntVal(q)), model_completion=True)
        labels[q] = int(lv.as_long())

    # Extract delta map for all ports
    delta_map: Dict[Tuple[int, int], Tuple[int, int]] = {}
    for q in range(N):
        for c in range(6):
            p = mkP(IntVal(q), IntVal(c))
            pr = m.eval(delta(p), model_completion=True)
            q2 = m.eval(room_of(pr), model_completion=True).as_long()
            e = m.eval(door_of(pr), model_completion=True).as_long()
            delta_map[(q, c)] = (int(q2), int(e))

    return labels, delta_map, is_unique


def build_map_json(
    N: int,
    labels: Dict[int, int],
    delta_map: Dict[Tuple[int, int], Tuple[int, int]],
    starting_room: int = 0,
) -> Dict[str, Any]:
    # rooms list
    rooms_list = [int(labels[q]) for q in range(N)]

    # undirected connections: include each edge once (port-pair)
    seen = set()
    conns = []
    for (q, c), (q2, e) in delta_map.items():
        a = (q, c)
        b = (q2, e)
        key = tuple(sorted([a, b]))  # undirected pair of ports
        if key in seen:
            continue
        seen.add(key)
        conns.append({"from": {"room": q, "door": c}, "to": {"room": q2, "door": e}})

    return {
        "rooms": rooms_list,
        "startingRoom": int(starting_room),
        "connections": conns,
    }


# ----------------------------- driver -----------------------------


def try_solve(
    plans,
    results,
    starting,
    N_opt: Optional[int],
    minN: int,
    maxN: int,
    check_unique: bool = False,
):
    if N_opt is not None:
        labels, dmap, is_unique = solve_with_N(
            N_opt, plans, results, starting, check_unique
        )
        return N_opt, labels, dmap, is_unique
    # sweep N upward to find a SAT model
    for N in range(minN, maxN + 1):
        try:
            labels, dmap, is_unique = solve_with_N(
                N, plans, results, starting, check_unique
            )
            return N, labels, dmap, is_unique
        except AssertionError:
            continue
    raise AssertionError(
        f"No SAT model found in N=[{minN}..{maxN}]. Provide N explicitly."
    )


def main():
    ap = argparse.ArgumentParser(description="ICFP 2025 Ædificium SMT solver")
    ap.add_argument("--json", required=True, help="Input JSON file with plans/results")
    ap.add_argument(
        "--output", default=None, help="Write map JSON to this path (default: stdout)"
    )
    ap.add_argument(
        "--N", type=int, default=None, help="Number of rooms; if omitted, sweep"
    )
    ap.add_argument("--minN", type=int, default=1, help="Min N to try if sweeping")
    ap.add_argument("--maxN", type=int, default=128, help="Max N to try if sweeping")
    ap.add_argument(
        "--verbose",
        type=int,
        default=0,
        help="Set Z3 solver verbosity level (default: 0)",
    )
    ap.add_argument(
        "--check-unique",
        action="store_true",
        help="Check if the found solution is unique",
    )
    args = ap.parse_args()

    if args.verbose > 0:
        set_option(verbose=args.verbose)

    with open(args.json, "r", encoding="utf-8") as f:
        data = json.load(f)
    plans, results, N_in, starting = parse_input(data)

    N, labels, dmap, is_unique = try_solve(
        plans,
        results,
        starting,
        args.N or N_in,
        args.minN,
        args.maxN,
        args.check_unique,
    )
    mjson = build_map_json(N, labels, dmap, starting)

    if is_unique is not None:
        print(
            f"Solution uniqueness: {'UNIQUE' if is_unique else 'NOT UNIQUE'}",
            file=sys.stderr,
        )

    output_data = {
        "rooms": mjson["rooms"],
        "startingRoom": mjson["startingRoom"],
        "connections": mjson["connections"],
    }
    if is_unique is not None:
        output_data["is_unique"] = is_unique

    out = json.dumps(
        output_data,
        ensure_ascii=False,
        indent=2,
    )
    if args.output:
        with open(args.output, "w", encoding="utf-8") as f:
            f.write(out)
    else:
        print(out)


if __name__ == "__main__":
    main()
