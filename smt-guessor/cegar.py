#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
CEGAR-based map learner for ICFPC2025 "Ædificium".
- Variables: m[i,j] (i<j): room(i) == room(j), leader ℓ[i]: first occurrence
- Base constraints: label-consistency, leader definition, Sum(ℓ)=K, (option) local determinism seed
- Refinement: (R1) transitivity, (R2) determinism on observed edges
- Multiple plans supported; all plans share the same starting room
Usage:
  python solver.py --input in.json --output out.json
  python solver.py --input in.json --minN 8 --maxN 64 --seed-local-det
Options:
  --seed-local-det   : seed "door[i]=door[j] ⇒ m[i+1,j+1]" for all pairs (faster)
  --complete-ports {none,fill} : fill unobserved ports to 6 per room (default: none)
  --verbose          : print CEGAR loop stats
Requires:
  pip install z3-solver
"""
from __future__ import annotations
import argparse, json, sys
from dataclasses import dataclass
from typing import List, Dict, Tuple, Optional, Set

try:
    from z3 import Solver, Bool, BoolVal, Implies, And, Or, Not, If, Sum, sat
except Exception as e:
    print(
        "This script requires the 'z3-solver' package. Install via: pip install z3-solver",
        file=sys.stderr,
    )
    raise

# ---------- Data model ----------


@dataclass
class Node:
    pid: int  # plan index
    t: int  # step within plan (0..len)
    label: int  # observed label in [0..3]
    idx: int  # global node index


@dataclass
class Step:
    src: int  # global idx of (pid,t)
    dst: int  # global idx of (pid,t+1)
    door: int  # normalized door in [0..5]


class DSU:
    def __init__(self, n: int):
        self.p = list(range(n))
        self.r = [0] * n

    def find(self, x: int) -> int:
        while self.p[x] != x:
            self.p[x] = self.p[self.p[x]]
            x = self.p[x]
        return x

    def union(self, a: int, b: int):
        a = self.find(a)
        b = self.find(b)
        if a == b:
            return
        if self.r[a] < self.r[b]:
            a, b = b, a
        self.p[b] = a
        if self.r[a] == self.r[b]:
            self.r[a] += 1


# ---------- Parsing & normalization ----------


def normalize_plan(p: str) -> List[int]:
    """Convert a plan string to list[int] doors in 0..5.
    Follow the same rule as main.py:
      - If all chars are in '0'..'5' -> treat as 0-based
      - Else if all chars are in '1'..'6' -> shift to 0-based
      - Else raise
    This avoids misclassifying strings like "2" or "11" as 1-based.
    """
    if not p:
        return []
    if all(ch in "012345" for ch in p):
        return [int(ch) for ch in p]
    if all(ch in "123456" for ch in p):
        return [int(ch) - 1 for ch in p]
    raise ValueError(f"Plan contains invalid digit(s): {p}")


def read_input(path: Optional[str]) -> dict:
    data = (
        json.load(sys.stdin)
        if not path
        else json.load(open(path, "r", encoding="utf-8"))
    )
    # basic validation
    plans = data.get("plans", [])
    results = data.get("results", [])
    if len(plans) != len(results):
        raise ValueError("plans and results must have the same length.")
    for p, r in zip(plans, results):
        if len(r) != len(p) + 1:
            raise ValueError(
                f"results length must be len(plan)+1. Mismatch at plan '{p}'."
            )
    return data


# ---------- CNF helpers on top of Z3 BoolRefs ----------


class MVars:
    """m[i,j] only for i<j; m[i,i] treated as True; m[j,i] is alias to m[i,j]"""

    def __init__(self, n_nodes: int):
        self.n = n_nodes
        self.vars: Dict[Tuple[int, int], Bool] = {}

    def var(self, i: int, j: int) -> Bool:
        if i == j:
            return BoolVal(True)
        if j < i:
            i, j = j, i
        key = (i, j)
        v = self.vars.get(key)
        if v is None:
            v = Bool(f"m_{i}_{j}")
            self.vars[key] = v
        return v

    def items(self):
        return self.vars.items()


# ---------- Core solver ----------


@dataclass
class Problem:
    nodes: List[Node]
    steps: List[Step]
    start_nodes: List[int]  # idx of (pid,0)
    labels: List[int]


def build_problem(inp: dict) -> Problem:
    plans_raw: List[str] = inp["plans"]
    results_raw: List[List[int]] = inp["results"]
    plans: List[List[int]] = [normalize_plan(p) for p in plans_raw]
    # build nodes
    nodes: List[Node] = []
    steps: List[Step] = []
    idx = 0
    start_nodes: List[int] = []
    for pid, (pl, res) in enumerate(zip(plans, results_raw)):
        assert len(res) == len(pl) + 1
        base_idx = idx
        for t, lab in enumerate(res):
            nodes.append(Node(pid=pid, t=t, label=int(lab), idx=idx))
            if t == 0:
                start_nodes.append(idx)
            idx += 1
        # steps
        for t, d in enumerate(pl):
            steps.append(Step(src=base_idx + t, dst=base_idx + t + 1, door=int(d)))
    labels = [nd.label for nd in nodes]
    return Problem(nodes=nodes, steps=steps, start_nodes=start_nodes, labels=labels)


def cardinality_eq(vars_bools: List[Bool], K: int):
    # Sum(If(b,1,0)) == K
    return Sum([If(b, 1, 0) for b in vars_bools]) == K


def cegar_solve(
    problem: Problem,
    K: int,
    seed_local_det: bool = False,
    verbose: bool = False,
    progress_stdout: bool = False,
    progress_every: int = 1,
):
    n = len(problem.nodes)
    mvars = MVars(n)
    lvars = [Bool(f"lead_{i}") for i in range(n)]
    s = Solver()

    labels = problem.labels
    # (A) label consistency
    for i in range(n):
        for j in range(i + 1, n):
            if labels[i] != labels[j]:
                s.add(Not(mvars.var(i, j)))

    # starting rooms across plans are identical
    for a in range(len(problem.start_nodes)):
        for b in range(a + 1, len(problem.start_nodes)):
            i = problem.start_nodes[a]
            j = problem.start_nodes[b]
            s.add(mvars.var(i, j))  # unit

    # (B) optional local determinism seed: door[i]=door[j] ⇒ m[i+1,j+1]
    if seed_local_det:
        # Build quick lookup: for each step index (src), door
        door_at: Dict[int, int] = {st.src: st.door for st in problem.steps}
        dest_of: Dict[int, int] = {st.src: st.dst for st in problem.steps}
        for i in range(n):
            if i not in door_at:
                continue
            di = door_at[i]
            for j in range(i + 1, n):
                dj = door_at.get(j, None)
                if dj is None:
                    continue
                if di == dj:
                    s.add(Implies(mvars.var(i, j), mvars.var(dest_of[i], dest_of[j])))

    # Leader definition and count
    for i in range(n):
        if i == 0:
            s.add(lvars[i])  # lead_0 = True
            continue
        # ℓ_i ↔ ∧_{j<i} ¬m_{i,j}
        left = lvars[i]
        right = And([Not(mvars.var(i, j)) for j in range(i)])
        s.add(Implies(left, right))
        s.add(Implies(right, left))

    s.add(cardinality_eq(lvars, K))

    # CEGAR loop
    iteration = 0
    # Track which specific R2 clauses we've already added to avoid duplicates
    added_r2: Set[Tuple[int, int]] = set()
    while True:
        iteration += 1
        if verbose or progress_stdout:
            stream = sys.stdout if progress_stdout else sys.stderr
            if progress_every <= 0:
                progress_every = 1
            if iteration % progress_every == 0 or iteration == 1:
                print(
                    f"[K={K}] iterate {iteration}, clauses={len(s.assertions())}",
                    file=stream,
                    flush=True,
                )
        if s.check() != sat:
            return None  # UNSAT for this K

        model = s.model()

        def M(i: int, j: int) -> bool:
            v = mvars.var(i, j)
            vv = model.eval(v, model_completion=True)
            return bool(vv)

        # Build DSU from m-true edges
        dsu = DSU(n)
        for (i, j), v in mvars.items():
            if bool(model.eval(v, model_completion=True)):
                dsu.union(i, j)
        # witnesses for determinism
        # map (classA, door) -> (classB, witness_src_idx)
        trans: Dict[Tuple[int, int], Tuple[int, int]] = {}
        conflict_R2: Optional[Tuple[int, int]] = (
            None  # (i,j) such that m[i,j] must imply m[i+1,j+1]
        )
        for st in problem.steps:
            A = dsu.find(st.src)
            B = dsu.find(st.dst)
            key = (A, st.door)
            prev = trans.get(key)
            if prev is None:
                trans[key] = (B, st.src)
            else:
                B2, i_prev = prev
                if B2 != B:
                    # determinism violated: same (A,door) goes to different classes B and B2
                    # witness (i_prev, st.src)
                    conflict_R2 = (i_prev, st.src)
                    break
        if conflict_R2 is not None:
            i, j = conflict_R2
            ii, jj = min(i, j), max(i, j)
            # Build adjacency of currently true equalities to capture the actual
            # class connectivity used by the model, then extract a path between ii and jj
            # and guard all those edges in a single clause implying m[ii+1, jj+1].
            # This prevents the solver from escaping the cut by flipping m[ii,jj] to False
            # while keeping ii and jj connected via other edges.
            adj = [[] for _ in range(n)]
            edges = []
            for (a, b), v in mvars.items():
                if bool(model.eval(v, model_completion=True)):
                    adj[a].append(b)
                    adj[b].append(a)
                    edges.append((a, b))
            # BFS for a path ii -> jj
            from collections import deque
            prev = [-1] * n
            q = deque([ii])
            prev[ii] = ii
            while q and prev[jj] == -1:
                u = q.popleft()
                for v in adj[u]:
                    if prev[v] == -1:
                        prev[v] = u
                        q.append(v)
            clause = []
            path_len = 0
            if prev[jj] != -1:
                # reconstruct path
                u = jj
                verts = []
                while u != ii:
                    verts.append(u)
                    u = prev[u]
                verts.append(ii)
                verts.reverse()
                # add guards for each edge on the path
                for x, y in zip(verts, verts[1:]):
                    a, b = (x, y) if x < y else (y, x)
                    clause.append(Not(mvars.var(a, b)))
                    path_len += 1
            else:
                # Fallback to the simple 2-literal guard
                clause.append(Not(mvars.var(ii, jj)))
            clause.append(mvars.var(ii + 1, jj + 1))
            s.add(Or(*clause))
            if verbose or progress_stdout:
                stream = sys.stdout if progress_stdout else sys.stderr
                if path_len > 0:
                    print(
                        f"[K={K}] R2-path len={path_len} implies m[{ii+1},{jj+1}]",
                        file=stream,
                        flush=True,
                    )
                else:
                    print(
                        f"[K={K}] R2 add  (~m[{ii},{jj}] ∨ m[{ii+1},{jj+1}])",
                        file=stream,
                        flush=True,
                    )
            continue

        # R1: transitivity: find i<j<k with m[i,j] & m[j,k] & !m[i,k]
        found_R1 = False
        # Build adjacency lists around each middle j
        true_left: List[List[int]] = [[] for _ in range(n)]
        true_right: List[List[int]] = [[] for _ in range(n)]
        for i in range(n):
            for j in range(i + 1, n):
                if M(i, j):
                    true_right[i].append(j)
                    true_left[j].append(i)
        for j in range(n):
            if not true_left[j] or not true_right[j]:
                continue
            Ls = true_left[j]
            Rs = true_right[j]
            # check pairs
            for i in Ls:
                for k in Rs:
                    if not M(i, k):
                        # add (~m[i,j] ∨ ~m[j,k] ∨ m[i,k])
                        s.add(
                            Or(
                                Not(mvars.var(i, j)),
                                Not(mvars.var(j, k)),
                                mvars.var(i, k),
                            )
                        )
                        if verbose or progress_stdout:
                            stream = sys.stdout if progress_stdout else sys.stderr
                            print(
                                f"[K={K}] R1 add  (~m[{i},{j}] ∨ ~m[{j},{k}] ∨ m[{i},{k}])",
                                file=stream,
                                flush=True,
                            )
                        found_R1 = True
                        break
                if found_R1:
                    break
            if found_R1:
                break
        if found_R1:
            continue

        # Passed R1 & R2: success
        if verbose or progress_stdout:
            stream = sys.stdout if progress_stdout else sys.stderr
            print(f"[K={K}] success after {iteration} iterations", file=stream, flush=True)
        return model, dsu, mvars, lvars


# ---------- Build final map JSON ----------


def build_output(
    problem: Problem, model, dsu: DSU, K: int, complete_ports: str = "none"
):
    n = len(problem.nodes)
    # class representatives (sorted by smallest node idx in class)
    reps: Dict[int, int] = {}
    members: Dict[int, List[int]] = {}
    for i in range(n):
        r = dsu.find(i)
        reps[r] = min(reps.get(r, r), i)
        members.setdefault(r, []).append(i)
    # map rep->room_id 0..K-1 by order of first appearance
    ordered_reps = sorted(reps.keys(), key=lambda r: reps[r])
    if len(ordered_reps) != K:
        # Safety: clip or pad (should not happen if Sum(ℓ)=K)
        ordered_reps = ordered_reps[:K]
    rep_to_room = {rep: i for i, rep in enumerate(ordered_reps)}
    # rooms labels: use label of smallest member (all equal inside class)
    rooms = [0] * K
    for rep in ordered_reps:
        room_id = rep_to_room[rep]
        rooms[room_id] = problem.labels[reps[rep]]

    # starting room id: any (pid,0)
    start_idx = problem.start_nodes[0]
    start_room = rep_to_room[dsu.find(start_idx)]

    # observed port assignments
    # forward: (room, port) -> neighbor_room
    forward: Dict[Tuple[int, int], int] = {}
    # also remember any observed reverse uses to pin 'to.door' if available
    observed_rev: Dict[Tuple[int, int], Set[int]] = (
        {}
    )  # (neighbor_room, this_room) -> set(door_at_neighbor)
    # Keep witnesses to produce a helpful error if determinism is violated
    det_witness: Dict[Tuple[int, int], Tuple[int, int, int]] = {}
    for st in problem.steps:
        A = rep_to_room[dsu.find(st.src)]
        B = rep_to_room[dsu.find(st.dst)]
        # set forward port
        key = (A, st.door)
        prev = forward.get(key)
        if prev is None:
            forward[key] = B
            det_witness[key] = (st.src, st.dst, B)
        else:
            # by construction via R2 this should be consistent
            if prev != B:
                src_prev, dst_prev, b_prev = det_witness[key]
                raise AssertionError(
                    "Non-deterministic port after solve: "
                    f"room={A}, door={st.door};"
                    f" saw src#{src_prev}->dst#{dst_prev} => room {b_prev}"
                    f" and src#{st.src}->dst#{st.dst} => room {B}"
                )
        # track possible reverse door at B when observed (some step from B to A)
        observed_rev.setdefault((B, A), set())

    for st in problem.steps:
        A = rep_to_room[dsu.find(st.src)]
        B = rep_to_room[dsu.find(st.dst)]
        observed_rev.setdefault((A, B), set())
        # If later there is a step from B to A, record that door:
        # we check by scanning steps once; simpler approach: fill after a first pass
    for st in problem.steps:
        A = rep_to_room[dsu.find(st.src)]
        B = rep_to_room[dsu.find(st.dst)]
        observed_rev.setdefault((B, A), set()).add(st.door)

    # pick reverse doors
    # maintain per-room used ports
    used_ports: Dict[int, Set[int]] = {r: set() for r in range(K)}
    for (room, port), nb in forward.items():
        used_ports[room].add(port)

    connections = []
    emitted: Set[Tuple[int, int, int, int]] = set()

    # helper to get a free port index 0..5
    def free_port(room: int) -> int:
        for c in range(6):
            if c not in used_ports[room]:
                return c
        # fallback: reuse some (graph may be multi-edge); try to avoid duplicates
        return 0

    # For each observed edge, emit a connection with a concrete reverse port.
    # De-duplicate by (A,portA) pairs (only one record per forward port).
    for (A, portA), B in forward.items():
        # choose reverse port
        cand = list(observed_rev.get((B, A), set()))
        if len(cand) > 0:
            portB = min([c for c in cand if c not in used_ports[B]], default=cand[0])
        else:
            portB = free_port(B)
        used_ports[B].add(portB)
        tup = (A, portA, B, portB)
        if tup in emitted:
            continue
        emitted.add(tup)
        connections.append(
            {"from": {"room": A, "door": portA}, "to": {"room": B, "door": portB}}
        )

    # Optionally complete to 6 ports per room by pairing free ports arbitrarily.
    if complete_ports == "fill":
        # build adjacency by ports already used
        for r in range(K):
            # pair remaining free ports within the same room as self-loops
            # (you may want a smarter completion aligned with instance generator)
            free = [c for c in range(6) if c not in used_ports[r]]
            # pair them in twos as self-loops r<->r
            for i in range(0, len(free), 2):
                if i + 1 >= len(free):
                    break
                c1, c2 = free[i], free[i + 1]
                used_ports[r].add(c1)
                used_ports[r].add(c2)
                connections.append(
                    {"from": {"room": r, "door": c1}, "to": {"room": r, "door": c2}}
                )

    out = {"rooms": rooms, "startingRoom": start_room, "connections": connections}
    return out


# ---------- CLI ----------


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--input", "-i", type=str, default=None)
    ap.add_argument("--output", "-o", type=str, default=None)
    ap.add_argument(
        "--N", type=int, default=None, help="known number of rooms; overrides sweep"
    )
    ap.add_argument("--minN", type=int, default=1)
    ap.add_argument("--maxN", type=int, default=128)
    ap.add_argument("--seed-local-det", action="store_true", dest="seed_local_det")
    ap.add_argument("--complete-ports", choices=["none", "fill"], default="none")
    ap.add_argument("--verbose", action="store_true")
    ap.add_argument(
        "--progress-stdout",
        action="store_true",
        help="print progress heartbeats to stdout (safe only with --output)",
    )
    ap.add_argument(
        "--progress-every",
        type=int,
        default=1,
        help="print progress every N CEGAR iterations",
    )
    args = ap.parse_args()

    inp = read_input(args.input)
    prob = build_problem(inp)

    # choose K
    Ks: List[int]
    if args.N is not None:
        Ks = [args.N]
    else:
        Ks = list(range(args.minN, args.maxN + 1))

    model = None
    dsu = None
    for K in Ks:
        if args.progress_stdout:
            # Start-of-K heartbeat
            print(f"[K={K}] start CEGAR", file=sys.stdout, flush=True)
        res = cegar_solve(
            prob,
            K,
            seed_local_det=args.seed_local_det,
            verbose=args.verbose,
            progress_stdout=args.progress_stdout,
            progress_every=args.progress_every,
        )
        if res is not None:
            model, dsu, mvars, lvars = res
            if args.verbose or args.progress_stdout:
                stream = sys.stdout if args.progress_stdout else sys.stderr
                print(
                    f"[OK] Found consistent abstraction with K={K}",
                    file=stream,
                    flush=True,
                )
            break
        else:
            if args.verbose or args.progress_stdout:
                stream = sys.stdout if args.progress_stdout else sys.stderr
                print(
                    f"[UNSAT] No solution for K={K}",
                    file=stream,
                    flush=True,
                )

    if model is None:
        print(json.dumps({"error": "UNSAT for all K"}, ensure_ascii=False, indent=2))
        sys.exit(2)

    out = build_output(prob, model, dsu, K, complete_ports=args.complete_ports)
    if args.output:
        with open(args.output, "w", encoding="utf-8") as f:
            json.dump(out, f, ensure_ascii=False, indent=2)
    else:
        print(json.dumps(out, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
