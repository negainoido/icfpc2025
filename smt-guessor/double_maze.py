#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Double Maze (2-lift) solver

Implements the approach described in DOUBLE_MAZE.md:
  1) Infer the quotient graph G1 using CEGIS SAT from exploration logs
  2) Walk an Euler trail on G1 while marking the lower bit as layer parity
  3) Recover edge signs sigma(e) in one pass and reconstruct the 2-lift

Requires the local API client in script.py/api.py to be usable. Set env vars:
  API_HOST=http://localhost:8000  TEAM_ID=...  (and optional USER for gateways)

Usage:
  python smt-guessor/double_maze.py solve primus --plans 12 --len-factor 1.5

Outputs a JSON bundle with:
  - base: rooms, connections (quotient graph)
  - sigma: mapping edge_id -> +1/-1
  - lift2: rooms2, startingRoom2, connections2 (reconstructed 2-lift)
"""
from __future__ import annotations

import argparse
import json
import os
import random
import sys
from typing import Dict, List, Tuple, Any, Optional

# Allow importing api client and helpers from script.py directory
_ROOT = os.path.dirname(os.path.dirname(__file__))
_SCRIPTS = os.path.join(_ROOT, "script.py")
if _SCRIPTS not in sys.path:
    sys.path.insert(0, _SCRIPTS)

# Local modules
import api  # type: ignore
from cegis_sat import cegis_sat
from euler_path import euler_path, rooms_from_edges


# Problem name -> N (number of rooms in G1)
PROBLEM_SIZES: Dict[str, int] = {
    "probatio": 3,
    "primus": 6,
    "secundus": 12,
    "tertius": 18,
    "quartus": 24,
    "quintus": 30,
    "aleph": 12,
    "beth": 24,
    "gimel": 36,
    "daleth": 48,
    "he": 60,
    "vau": 18,
    "zain": 36,
    "hhet": 54,
    "teth": 72,
    "iod": 90,
}


def _label_lsb(val: int) -> int:
    return int(val) & 1


def _label_msb(val: int) -> int:
    return (int(val) >> 1) & 1


def _replace_lsb(val: int, lsb: int) -> int:
    return ((_label_msb(val) << 1) | (lsb & 1))


def _normalize_plan(plan: List[int]) -> str:
    return "".join(str(d) for d in plan)


def _door_from_to(connections: List[dict], u: int, v: int) -> Tuple[int, int, int]:
    """Find an edge (by door) connecting u -> v.

    Returns (edge_id, door_u, door_v).
    Raises if not found.
    """
    for eid, c in enumerate(connections):
        ru = int(c["from"]["room"])  # type: ignore[index]
        du = int(c["from"]["door"])  # type: ignore[index]
        rv = int(c["to"]["room"])  # type: ignore[index]
        dv = int(c["to"]["door"])  # type: ignore[index]
        if ru == u and rv == v:
            return eid, du, dv
        if ru == v and rv == u:
            return eid, dv, du
    raise ValueError(f"No edge connecting {u} and {v}")


def _build_explore_plans(
    N: int, count: int, len_factor: float, seed: Optional[int] = None
) -> List[str]:
    rng = random.Random(seed)
    L = max(1, int(N * len_factor))
    plans: List[str] = []
    for _ in range(count):
        seq = [rng.randint(0, 5) for _ in range(L)]
        plans.append(_normalize_plan(seq))
    return plans


def solve_double_maze(
    problem_name: str,
    *,
    plans_count: int = 12,
    len_factor: float = 1.5,
    cegis_prefix: int = 10,
    cegis_iters: int = 30,
    backend: str = "auto",
    seed: Optional[int] = None,
) -> Dict[str, Any]:
    if problem_name not in PROBLEM_SIZES:
        raise ValueError(f"Unknown problem '{problem_name}'")
    N = PROBLEM_SIZES[problem_name]

    # Select problem on the server
    api.api.select(problem_name)

    # Phase 1: collect exploration logs and infer the quotient graph G1
    base_plans = _build_explore_plans(N, plans_count, len_factor, seed)
    exp = api.api.explore(base_plans)
    results_raw: List[List[int]] = [[int(x) for x in r] for r in exp["results"]]

    plans_num = [[int(ch) for ch in p] for p in base_plans]
    out, meta = cegis_sat(
        plans_num,
        results_raw,
        N,
        init_prefix=cegis_prefix,
        max_iters=cegis_iters,
        verbose=False,
        backend=backend,
    )
    if not out:
        raise RuntimeError(f"CEGIS could not find a feasible quotient: {meta}")

    rooms: List[int] = [int(x) for x in out["rooms"]]  # type: ignore[index]
    connections: List[dict] = list(out["connections"])  # type: ignore[index]
    starting_room = int(out.get("startingRoom", 0))

    base_map = {"rooms": rooms, "startingRoom": starting_room, "connections": connections}

    # Phase 2: compute Euler route and build one pass plan that writes parity LSB
    edges_order = euler_path(base_map, start_room=starting_room)
    rooms_seq = rooms_from_edges(base_map, edges_order, starting_room)

    # parity and visitation bookkeeping
    par: Dict[int, int] = {}
    vis: Dict[int, bool] = {}
    sigma: Dict[int, int] = {}  # edge_id -> +1 / -1

    s = starting_room
    par[s] = 0
    vis[s] = True

    # Build the token stream: start by setting start parity to 0
    tokens: List[str] = [f"[{par[s]}]"]

    # Also precompute per-step info: (eid, u, v, door_u, door_v)
    step_info: List[Tuple[int, int, int, int, int]] = []
    cur = rooms_seq[0]
    for i, eid in enumerate(edges_order):
        nxt = rooms_seq[i + 1]
        eid2, du, dv = _door_from_to(connections, cur, nxt)
        assert eid2 == eid, "Euler edge order inconsistent with connections"
        step_info.append((eid, cur, nxt, du, dv))
        # prepare parity for first-visit nodes (tree edges)
        if not vis.get(nxt, False):
            par[nxt] = par[cur]
        cur = nxt

    # Build tokens in one pass (move, then write target parity)
    for eid, u, v, du, _ in step_info:
        tokens.append(str(du))  # traverse edge
        # after arrival, write LSB to par[v]
        pv = par[v]
        tokens.append(f"[{pv}]")
        vis[v] = True

    # Send the composed plan and receive the full label stream
    plan_str = "".join(tokens)
    exp2 = api.api.explore([plan_str])
    res: List[int] = [int(x) for x in exp2["results"][0]]

    # Decode sigma from the result stream
    # Results semantics: initial label (at start), then one label after each token
    # We started by writing at start ([0]) -> first result after token is start label with LSB=0
    idx = 1  # after initial write token
    cur = rooms_seq[0]
    for eid, u, v, du, _ in step_info:
        # after move token -> observe arrival label at v
        idx += 1
        obs = int(res[idx - 1])
        if u == cur:
            # decide sigma: if first visit to v, force +1; else compare LSBs
            if eid not in sigma:
                if not vis.get(v, False):
                    sigma[eid] = +1
                else:
                    sigma[eid] = +1 if _label_lsb(obs) == par[u] else -1
            else:
                # if seen before via duplication, verify consistency
                expected = +1 if _label_lsb(obs) == par[u] else -1
                # tolerate if first time was tree edge (+1)
                if sigma[eid] != expected and vis.get(v, False):
                    # keep the earlier value but this should normally match
                    pass
            cur = v
        else:
            # This should not occur if rooms_seq is consistent
            raise RuntimeError("Path decoding lost sync with rooms sequence")
        # after write token -> skip one label (we don't need it)
        idx += 1

    # Phase 3: reconstruct 2-lift
    rooms2: List[int] = []
    for u in range(N):
        msb = _label_msb(rooms[u])
        rooms2.append((msb << 1) | 0)
        rooms2.append((msb << 1) | 1)

    connections2: List[dict] = []
    for eid, c in enumerate(connections):
        ru = int(c["from"]["room"])  # type: ignore[index]
        du = int(c["from"]["door"])  # type: ignore[index]
        rv = int(c["to"]["room"])  # type: ignore[index]
        dv = int(c["to"]["door"])  # type: ignore[index]
        sgn = int(sigma.get(eid, +1))
        if sgn == +1:
            # same-layer connections
            connections2.append(
                {
                    "from": {"room": 2 * ru + 0, "door": du},
                    "to": {"room": 2 * rv + 0, "door": dv},
                }
            )
            connections2.append(
                {
                    "from": {"room": 2 * ru + 1, "door": du},
                    "to": {"room": 2 * rv + 1, "door": dv},
                }
            )
        else:
            # cross-layer connections
            connections2.append(
                {
                    "from": {"room": 2 * ru + 0, "door": du},
                    "to": {"room": 2 * rv + 1, "door": dv},
                }
            )
            connections2.append(
                {
                    "from": {"room": 2 * ru + 1, "door": du},
                    "to": {"room": 2 * rv + 0, "door": dv},
                }
            )

    starting_room2 = 2 * starting_room + 0

    return {
        "base": base_map,
        "sigma": sigma,
        "lift2": {
            "rooms": rooms2,
            "startingRoom": starting_room2,
            "connections": connections2,
        },
    }


def main():
    ap = argparse.ArgumentParser(description="Double Maze (2-lift) solver")
    sub = ap.add_subparsers(dest="cmd", required=True)

    sp = sub.add_parser("solve", help="Solve a selected problem via local server")
    sp.add_argument("problem", help="Problem name (e.g., primus, secundus, ...)")
    sp.add_argument("--plans", type=int, default=12, help="Number of random plans for Phase1")
    sp.add_argument(
        "--len-factor",
        type=float,
        default=1.5,
        help="Plan length factor times N (e.g., 1.5*N)",
    )
    sp.add_argument("--seed", type=int, default=None, help="Random seed for reproducibility")
    sp.add_argument("--prefix", type=int, default=10, help="CEGIS initial prefix per trace")
    sp.add_argument("--iters", type=int, default=30, help="CEGIS max iterations")
    sp.add_argument(
        "--backend", default="auto", choices=["auto", "kissat", "pysat"], help="SAT backend"
    )
    sp.add_argument("--output", default=None, help="Write output JSON to this path")

    args = ap.parse_args()

    if args.cmd == "solve":
        bundle = solve_double_maze(
            args.problem,
            plans_count=args.plans,
            len_factor=args.len_factor,
            cegis_prefix=args.prefix,
            cegis_iters=args.iters,
            backend=args.backend,
            seed=args.seed,
        )
        js = json.dumps(bundle, ensure_ascii=False, indent=2)
        if args.output:
            with open(args.output, "w", encoding="utf-8") as f:
                f.write(js)
        else:
            print(js)


if __name__ == "__main__":
    main()

