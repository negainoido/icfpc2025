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
from euler_path import doors_from_edges, euler_path, rooms_from_edges


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
    return (_label_msb(val) << 1) | (lsb & 1)


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


def _rev_port(connections: List[dict], u: int, door: int) -> tuple[int, int]:
    """Find the room connected to u via door.

    Returns the other room.
    Raises if not found.
    """
    for eid, c in enumerate(connections):
        ru = int(c["from"]["room"])  # type: ignore[index]
        du = int(c["from"]["door"])  # type: ignore[index]
        rv = int(c["to"]["room"])  # type: ignore[index]
        dv = int(c["to"]["door"])  # type: ignore[index]
        if ru == u and du == door:
            return rv, dv
        if rv == u and dv == door:
            return ru, du
    raise ValueError(f"No edge from {u} via door {door}")


def _build_explore_plans(N: int, seed: Optional[int] = None) -> List[str]:
    rng = random.Random(seed)
    L = max(1, int(N * 2))
    plans: List[str] = []
    for _ in range(10):
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
    N_total = PROBLEM_SIZES[problem_name]
    # For the double-maze, the quotient graph size is N_total/2
    N1 = N_total // 2

    # Prepare log directory near this file
    log_dir = os.path.join(os.path.dirname(__file__), "log")
    os.makedirs(log_dir, exist_ok=True)

    # Select problem on the server
    api.api.select(problem_name)

    # Phase 1: collect exploration logs and infer the quotient graph G1
    base_plans = _build_explore_plans(N_total, seed)
    exp = api.api.explore(base_plans)
    results_raw: List[List[int]] = [[int(x) for x in r] for r in exp["results"]]

    # Log: initial exploration in example/* format
    with open(os.path.join(log_dir, "phase1_explore.json"), "w", encoding="utf-8") as f:
        json.dump(
            {
                "plans": base_plans,
                "results": results_raw,
                "N": N1,
                "startingRoom": 0,
            },
            f,
            ensure_ascii=False,
            indent=2,
        )

    plans_num = [[int(ch) for ch in p] for p in base_plans]
    out, meta = cegis_sat(
        plans_num,
        results_raw,
        N1,
        init_prefix=cegis_prefix,
        max_iters=cegis_iters,
        verbose=False,
        backend=backend,
    )
    if not out:
        raise RuntimeError(f"CEGIS could not find a feasible quotient: {meta}")

    # rooms: 各部屋のラベル（0-3)
    rooms: List[int] = [int(x) for x in out["rooms"]]  # type: ignore[index]
    connections: List[dict] = list(out["connections"])  # type: ignore[index]
    starting_room = int(out.get("startingRoom", 0))

    base_map = {
        "rooms": rooms,
        "startingRoom": starting_room,
        "connections": connections,
    }

    # Log: quotient graph
    with open(os.path.join(log_dir, "quotient_graph.json"), "w", encoding="utf-8") as f:
        json.dump(base_map, f, ensure_ascii=False, indent=2)

    # Phase 2: compute Euler route and build one pass plan that writes parity LSB
    edges_order = euler_path(base_map, start_room=starting_room)
    rooms_seq = rooms_from_edges(base_map, edges_order, starting_room)
    doors_seq = doors_from_edges(base_map, edges_order, starting_room)
    print(f"rooms_seq: {rooms_seq}", file=sys.stderr)
    # Ensure the Euler trail starts from the actual starting room by rotation if needed
    assert rooms_seq is not None and rooms_seq[0] == starting_room

    # parity and visitation bookkeeping
    # やりたいこと
    # オイラー路をたどりながら、各部屋に最初に到達した時にその部屋のラベルのLSBを反転させる。
    vis: set[int] = set()
    sigma: Dict[int, int] = {}  # edge_id -> +1 / -1

    def calc_parity(room: int) -> int:
        return rooms[room] ^ 1

    tokens: List[str] = []

    s = rooms_seq[0]
    for room, door in zip(rooms_seq, doors_seq):
        print(f"At room {room}, door {door}", file=sys.stderr)
        if room not in vis:
            # First visit to s: flip LSB
            print(f" First visit to {s}, flip LSB", file=sys.stderr)
            tokens.append(f"[{calc_parity(room)}]")
            vis.add(room)
        tokens.append(str(door))

    # オイラー路をたどりつつ最初に訪れた時に部屋のLSBを反転させる

    # Send the composed plan and receive the full label stream
    plan_str = "".join(tokens)
    exp2 = api.api.explore([plan_str])
    res: List[int] = [int(x) for x in exp2["results"][0]]

    # Log: second plan
    with open(os.path.join(log_dir, "phase2_plan.json"), "w", encoding="utf-8") as f:
        json.dump(
            {
                "start": rooms_seq[0] if rooms_seq else starting_room,
                "edges_order": edges_order,
                "rooms_seq": rooms_seq,
                "plan": plan_str,
                "results": res,
            },
            f,
            ensure_ascii=False,
            indent=2,
        )

    # Decode sigma from the result stream
    # Results semantics: initial label (at start), then one label after each token
    # We started by writing at start ([0]) -> first result after token is start label with LSB=0
    # Arrival labels are at indices res[2], res[4], ..., res[2*(i+1)]
    rooms_seq = rooms_seq
    vis = set()
    idx = 0
    # 最初の部屋はバニラ確定
    is_vanilla = True
    # door_info[(room, door)] = True/False (vanilla/cross)
    door_info: Dict[Tuple[int, int], bool] = {}
    for token, obs in zip(tokens, res[1:]):
        # 現在の部屋
        room = rooms_seq[idx]
        # tokenを実行した結果obsが得られた
        if token.startswith("["):
            vis.add(room)
            continue
        # 移動したパターン
        door = int(token)
        next_room = rooms_seq[idx + 1]
        # 遷移先がバニラかダブルか判定
        next_is_vanilla = True
        if next_room not in vis:
            # 次のステップで色を替えるので、遷移先はバニラ確定
            next_is_vanilla = True
            pass
        elif obs == rooms[next_room]:
            # 色を変えたはずなのに変わってない -> クロスエッジ
            next_is_vanilla = False
        else:
            # 色を変えたノードに戻ってきた
            next_is_vanilla = True
        print(
            f" Move {room} --{door}--> {next_room}, obs={obs}, "
            f"next_is_vanilla={next_is_vanilla}, is_vanilla={is_vanilla}",
            file=sys.stderr,
        )
        edge_parity = is_vanilla ^ next_is_vanilla
        door_info[(room, door)] = edge_parity
        door_info[_rev_port(connections, room, door)] = edge_parity

        is_vanilla = next_is_vanilla
        idx += 1

    # Phase 3: reconstruct 2-lift
    rooms2: List[int] = []
    for room in rooms:
        rooms2.append(room)
        rooms2.append(room)

    connections2: List[dict] = []
    for eid, c in enumerate(connections):
        ru = int(c["from"]["room"])  # type: ignore[index]
        du = int(c["from"]["door"])  # type: ignore[index]
        rv = int(c["to"]["room"])  # type: ignore[index]
        dv = int(c["to"]["door"])  # type: ignore[index]
        assert (ru, du) in door_info, f"Missing door info for {(ru, du)}"

        if door_info[(ru, du)] == False:
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

    bundle = {
        "base": base_map,
        "sigma": sigma,
        "lift2": {
            "rooms": rooms2,
            "startingRoom": starting_room2,
            "connections": connections2,
        },
    }

    # Log: final (result) graph
    with open(os.path.join(log_dir, "lift2_graph.json"), "w", encoding="utf-8") as f:
        json.dump(bundle, f, ensure_ascii=False, indent=2)

    return bundle


def main():
    ap = argparse.ArgumentParser(description="Double Maze (2-lift) solver")
    sub = ap.add_subparsers(dest="cmd", required=True)

    sp = sub.add_parser("solve", help="Solve a selected problem via local server")
    sp.add_argument("problem", help="Problem name (e.g., primus, secundus, ...)")
    sp.add_argument(
        "--plans", type=int, default=12, help="Number of random plans for Phase1"
    )
    sp.add_argument(
        "--len-factor",
        type=float,
        default=1.5,
        help="Plan length factor times N (e.g., 1.5*N)",
    )
    sp.add_argument(
        "--seed", type=int, default=None, help="Random seed for reproducibility"
    )
    sp.add_argument(
        "--prefix", type=int, default=10, help="CEGIS initial prefix per trace"
    )
    sp.add_argument("--iters", type=int, default=30, help="CEGIS max iterations")
    sp.add_argument(
        "--backend",
        default="auto",
        choices=["auto", "kissat", "pysat"],
        help="SAT backend",
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

        resp = api.api.guess(bundle["lift2"])
        print(json.dumps(resp, ensure_ascii=False))


if __name__ == "__main__":
    main()
