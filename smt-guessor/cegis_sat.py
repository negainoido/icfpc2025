#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
CEGIS with SAT using PySAT CNF + Kissat binary (preferred).

- Encodes the first K steps (prefix) of each plan into CNF.
- Solves with the external Kissat binary when available, otherwise uses an
  in-process PySAT solver backend (glucose/minisat/etc.).
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import shutil
from typing import Any, Dict, List, Optional, Set, Tuple, TypedDict
from pysat.formula import CNF as SatCNF
from pysat.formula import IDPool
from pysat.card import CardEnc, EncType
from pysat.solvers import Solver  # type: ignore


STARTING_ROOM_ID = 0


def normalize_plan(plan: str) -> List[int]:
    return [int(ch) for ch in plan]


class Problem(TypedDict):
    plans: List[List[int]]  # normalized plans
    results: List[List[int]]
    N: int
    s0: int


class Solution(TypedDict):
    status: int  # 1=feasible, 0=infeasible
    rooms: List[int]  # room labels
    startingRoom: int
    connections: List[Dict[str, Dict[str, int]]]


def load_problem(path: str) -> Problem:
    with open(path, "r") as f:
        data = json.load(f)
    plans = [normalize_plan(p) for p in data["plans"]]
    results = data["results"]
    N = int(data["N"])  # rooms
    s0 = int(data.get("startingRoom", STARTING_ROOM_ID))
    return {"plans": plans, "results": results, "N": N, "s0": s0}


# bits has label val (0..3)
def label2bits(val: int) -> Tuple[bool, bool]:
    return bool((val & 2) >> 1), bool(val & 1)


def bits2label(b0: bool, b1: bool) -> int:
    return (b0 << 1) | b1


def build_cnf_prefix(
    plans: List[List[int]],
    results: List[List[int]],
    N: int,
    prefix_steps: List[int],
    D: int = 6,
) -> Tuple["SatCNF", Dict[str, object]]:
    """Build CNF for the first prefix_steps[t] steps of each plan t.

    Returns (cnf, meta) where meta holds IDPool, key sets for decoding, etc.
    """
    assert len(plans) == len(results) == len(prefix_steps)
    T_used: List[int] = []
    used_plans: List[List[int]] = []
    used_results: List[List[int]] = []
    for t, (pl, rs, K) in enumerate(zip(plans, results, prefix_steps)):
        K = max(0, min(int(K), len(pl)))
        if len(rs) < K + 1:
            raise ValueError(f"results[{t}] too short: need {K+1}, got {len(rs)}")
        used_plans.append(pl[:K])
        used_results.append(rs[: K + 1])
        T_used.append(K)

    # 各ラベルlに対して、 l_from -d_from:?-> l -d_to:?-> l_to という形の(l_from, d_from, d_to, l_to)の組を
    # これをlocal_window[l]という

    local_window: list[Set[Tuple[int, int, int, int]]] = [set() for _ in range(4)]
    for pl, rs, K in zip(plans, results, prefix_steps):
        # prefixに関しては遷移制約の方が強いので、local_windowの対象外とする
        for i in range(K, len(pl)):
            l_from = rs[i - 1]
            d_from = pl[i - 1]
            l = rs[i]
            d_to = pl[i]
            l_to = rs[i + 1]
            local_window[l].add((l_from, d_from, d_to, l_to))
    for l in range(4):
        lw = list(local_window[l])
        print(f"local_window[{l}]: {lw[:5]} ... (total {len(lw)})")

    P = D * N

    cnf = SatCNF()
    pool = IDPool()

    port_keys: Set[Tuple[int, int]] = set()
    move_keys: Set[Tuple[int, int]] = set()
    loc_keys: Set[Tuple[int, int, int]] = set()

    # Variable helpers
    # L[r]: 部屋rのラベルを２ビットで返す。
    def v_label(rid: int) -> tuple[int, int]:
        return pool.id(("L", rid, 0)), pool.id(("L", rid, 1))

    # Lp[p]: ポートpのラベルを2ビットで返す。
    def v_port_label(pid: int) -> tuple[int, int]:
        return pool.id(("Lp", pid, 0)), pool.id(("Lp", pid, 1))

    # P[p, q]: ポートpがqと繋がっている (p < q)
    def v_port(p: int, q: int) -> int:
        # symmetric variable id for pair (p,q)
        a, b = (p, q) if p <= q else (q, p)
        port_keys.add((a, b))
        return pool.id(("P", a, b))

    # M[p, g]: ポートpから部屋gに移動可能
    def v_move(p: int, g: int) -> int:
        move_keys.add((p, g))
        return pool.id(("M", p, g))

    # X[t, k, r]: trace tの時刻kに部屋rにいる
    def v_loc(tid: int, k: int, rid: int) -> int:
        loc_keys.add((tid, k, rid))
        return pool.id(("X", tid, k, rid))

    # == ビット等号（XNOR）の補助変数 ==
    def lit_xnor(a, b, tag):
        z = pool.id((tag, a, b))
        cnf.append([z, a, b])
        cnf.append([z, -a, -b])
        cnf.append([-z, a, -b])
        cnf.append([-z, -a, b])
        return z

    # == 2bit 等号 ==
    def lit_eq2(a1, a0, b1, b0, tag):
        e1 = lit_xnor(a1, b1, (tag, "xnor1"))
        e0 = lit_xnor(a0, b0, (tag, "xnor0"))
        z = pool.id((tag, "eq2"))
        # z <-> (e1 ∧ e0)
        cnf.append([-z, e1])
        cnf.append([-z, e0])
        cnf.append([z, -e1, -e0])
        return z

    # == 2bit 厳密比較 a<b ==
    def lit_lt2(a1, a0, b1, b0, tag):
        # (a1<b1) ∨ (a1==b1 ∧ a0<b0)
        l1 = pool.id((tag, "l1"))  # ¬a1 ∧ b1
        cnf.append([-l1, -a1])
        cnf.append([-l1, b1])

        e1 = lit_xnor(a1, b1, (tag, "xnor1"))
        l0 = pool.id((tag, "l0"))  # ¬a0 ∧ b0
        cnf.append([-l0, -a0])
        cnf.append([-l0, b0])

        u = pool.id((tag, "u"))  # u <-> (e1 ∧ l0)
        cnf.append([-u, e1])
        cnf.append([-u, l0])
        cnf.append([u, -e1, -l0])

        lt = pool.id((tag, "lt"))
        # lt <-> (l1 ∨ u)
        cnf.append([-lt, l1, u])
        cnf.append([lt, -l1])
        cnf.append([lt, -u])
        return lt

    # == ガード（ラベル等号の前提）をリテラルとして直に使う ==
    def guard_literals_for_label(r, lab):
        # lab は 0..3。label2bits(l) -> (b0,b1) で既に使っているはず
        b0, b1 = label2bits(lab)
        v0, v1 = v_label(r)  # 2bit 変数
        # 「L[r]==lab」を1本のリテラルにはできないので、
        # clauseガードに使うための“2つの前提リテラル（否定形で使う）”を返す
        lit_ok0 = v0 if b0 else -v0
        lit_ok1 = v1 if b1 else -v1
        # これらの否定を clause に足せば “(L[r]==lab) が成り立つときにだけ効く” になる
        return lit_ok0, lit_ok1

    # (1) Room Labels: ラベルは均等に分布する。
    q, r = divmod(N, 4)
    o = int(used_results[0][0]) if used_results else 0
    regular = 4 * q
    for i in range(0, regular):
        lab = (o + i) % 4
        bits = label2bits(lab)
        v0, v1 = v_label(i)
        cnf.append([v0 if bits[0] else -v0])
        cnf.append([v1 if bits[1] else -v1])

    # (2) Perfect matching on ports using symmetric variables
    for p in range(P):
        row = [v_port(p, q) for q in range(P)]
        cnf.extend(
            CardEnc.equals(
                lits=row, bound=1, vpool=pool, encoding=EncType.seqcounter
            ).clauses
        )

    # (2a) Lp[p] はリンク先の部屋のラベルと一致する
    # つまり、M[p, g] -> Lp[p] == L[g]
    for p in range(P):
        for g in range(N):
            m = v_move(p, g)
            v0p, v1p = v_port_label(p)
            v0g, v1g = v_label(g)
            cnf.append([-m, v0p, -v0g])
            cnf.append([-m, v1p, -v1g])
            cnf.append([-m, -v0p, v0g])
            cnf.append([-m, -v1p, v1g])

    # (3) Movement possibility M[p,g] linked to OR_d P[p, D*g + d]
    for p in range(P):
        for g in range(N):
            m = v_move(p, g)
            ors: List[int] = []
            base = D * g
            for d in range(D):
                to_p = base + d
                u = v_port(p, to_p)
                ors.append(u)
                # (¬U -> M)
                cnf.append([-u, m])
            # (¬M -> OR U)
            cnf.append([-m] + ors)

    # (4) Trace prefix encoding
    for tid, (pl, rs, K) in enumerate(zip(used_plans, used_results, T_used)):
        # X variables and exactly-one per time
        X_t: List[List[int]] = []
        for k in range(K + 1):
            lits = [v_loc(tid, k, r) for r in range(N)]
            cnf.extend(
                CardEnc.equals(
                    lits=lits, bound=1, vpool=pool, encoding=EncType.seqcounter
                ).clauses
            )
            X_t.append(lits)

        # start location fixed
        cnf.append([v_loc(tid, 0, STARTING_ROOM_ID)])

        # label consistency with observations
        for k in range(K + 1):
            obs = int(rs[k])
            for r in range(N):
                # X[t,k,r] -> L[r] == obs
                obs_bits = label2bits(obs)
                v0, v1 = v_label(r)
                if obs_bits[0]:
                    cnf.append([-v_loc(tid, k, r), v0])
                else:
                    cnf.append([-v_loc(tid, k, r), -v0])
                if obs_bits[1]:
                    cnf.append([-v_loc(tid, k, r), v1])
                else:
                    cnf.append([-v_loc(tid, k, r), -v1])

        # transitions across K steps
        for k in range(K):
            a = int(pl[k])
            assert 0 <= a < D
            for r in range(N):
                p = r * D + a
                for g in range(N):
                    m = v_move(p, g)
                    # X[k,r] ∧ M[p,g] -> X[k+1, g]
                    cnf.append([-v_loc(tid, k, r), -m, v_loc(tid, k + 1, g)])
                    # Linking helps propagation
                    cnf.append([-v_loc(tid, k, r), -v_loc(tid, k + 1, g), m])

    # (6) Local window constraints
    # 各ラベルlに対して、 l_from -d_from:?-> l -d_to:?-> l_to という形の(l_from, d_from, d_to, l_to)のそれぞれのくみに対して
    # ある部屋qが存在して、以下を満たす
    # L[q] == l
    # Lp[D*q + d_to] == l_to
    # or_{d in range(D)} Lp[D*q + d] == l_from
    #
    # tuple = (l_from, d_from, d_to, l_to) だが、d_from は arrival ではなく前の部屋の出口なので
    # ここでは「存在 d: Lp[q,d] == l_from」の形にする（arrival ポートを特定できないため）。
    # (6) Local window constraints (∃q)
    # tuple = (l_from, d_from, d_to, l_to)
    # 要件:
    #  ∃q.  L[q]==l  ∧  Lp[q,d_to]==l_to  ∧  (∨_d Lp[q,d]==l_from)
    # def lit_eq_lp_guarded(p, label):
    #     """ガード付きに使う z 変数（片方向）： z -> (Lp[p] == label)"""
    #     z = pool.id(("EQ_LP_G", p, label))
    #     a0, a1 = v_port_label(p)
    #     b0, b1 = label2bits(label)
    #     # 片方向のみ
    #     cnf.append([-z, a0 if b0 else -a0])
    #     cnf.append([-z, a1 if b1 else -a1])
    #     return z

    # def lit_eq_l_guarded(r, label):
    #     """ガード付きに使う z 変数（片方向）： z -> (L[r] == label)"""
    #     z = pool.id(("EQ_L_G", r, label))
    #     v0, v1 = v_label(r)
    #     b0, b1 = label2bits(label)
    #     cnf.append([-z, v0 if b0 else -v0])
    #     cnf.append([-z, v1 if b1 else -v1])
    #     return z

    # for l in range(4):
    #     tuples = list(local_window[l])
    #     for idx, (l_from, d_from, d_to, l_to) in enumerate(tuples):
    #         # ∃q: セレクタ W[idx,q]
    #         Wq = []
    #         for q in range(N):
    #             w = pool.id(("W", "lw", l, idx, q))
    #             Wq.append(w)

    #             # (1) w -> L[q] == l
    #             zL = lit_eq_l_guarded(q, l)
    #             cnf.append([-w, zL])

    #             # (2) w -> Lp[q, d_to] == l_to
    #             p_to = q * D + d_to
    #             z_to = lit_eq_lp_guarded(p_to, l_to)
    #             cnf.append([-w, z_to])

    #             # (3) w -> ∨_d (Lp[q,d] == l_from)
    #             or_lits = []
    #             base = q * D
    #             for d in range(D):
    #                 z_from_d = lit_eq_lp_guarded(base + d, l_from)
    #                 or_lits.append(z_from_d)
    #             cnf.append([-w] + or_lits)

    #         # 存在：∨_q W[idx,q]
    #         cnf.append(Wq)

    # (7) Symmetry breaking for port labels
    # 各ラベルlについて、ラベルlの部屋1 < r1 < r2に対して
    # r1のポートラベルのビット列 <= r2のポートラベルのビット列
    # (7) Symmetry breaking for port labels (lex ≤ within same-room-label)
    for lab in range(4):
        # 部屋0はアンカーなので、比較ペアから除外（r1>=1）
        for r1 in range(1, N):  # ★ ここを 1 から
            for r2 in range(r1 + 1, N):
                # ガード: (L[r1]==lab) ∧ (L[r2]==lab)
                g1a, g1b = guard_literals_for_label(r1, lab)
                g2a, g2b = guard_literals_for_label(r2, lab)

                # 各桁の等号/比較
                eq = []
                lt = []
                for d in range(D):
                    a0, a1 = v_port_label(r1 * D + d)
                    b0, b1 = v_port_label(r2 * D + d)
                    eq_d = lit_eq2(a1, a0, b1, b0, ("LEX", "EQ", lab, r1, r2, d))
                    lt_d = lit_lt2(a1, a0, b1, b0, ("LEX", "LT", lab, r1, r2, d))
                    eq.append(eq_d)
                    lt.append(lt_d)

                # prefix 等号
                pref: list[int] = [-1] * D
                for d in range(D):
                    z = pool.id(("LEX", "PREF", lab, r1, r2, d))
                    cnf.append([-z, eq[d]])
                    if d > 0:
                        cnf.append([-z, pref[d - 1]])
                        cnf.append([z, -eq[d], -pref[d - 1]])
                    else:
                        cnf.append([z, -eq[d]])
                    pref[d] = z

                # lex ≤ ： pref[5] ∨ lt[0] ∨ (pref[0]∧lt[1]) ∨ … ∨ (pref[4]∧lt[5])
                terms = [lt[0]]
                for d in range(1, D):
                    t = pool.id(("LEX", "TERM", lab, r1, r2, d))
                    cnf.append([-t, pref[d - 1]])
                    cnf.append([-t, lt[d]])
                    cnf.append([t, -pref[d - 1], -lt[d]])
                    terms.append(t)

                # ガード付き大OR
                head = [-g1a, -g1b, -g2a, -g2b, pref[D - 1]] + terms
                cnf.append(head)

    cnf.nv = max(getattr(cnf, "nv", 0) or 0, pool.top)
    print(f"CNF variables: {cnf.nv}, clauses: {len(cnf.clauses)}")

    meta = {
        "N": N,
        "D": D,
        "P": P,
        "pool": pool,
        "port_keys": port_keys,
    }
    return cnf, meta


def solve_with_pysat(
    cnf: "SatCNF", time_limit_s: Optional[float] = None
) -> Tuple[str, Dict[int, bool]]:
    """Solve CNF using an in-process PySAT solver. Returns (status, assignment)."""
    # Prefer a modern solver if available, otherwise fallback to default

    solver_names = ["cadical153", "glucose4", "glucose3", "minisat22", None]
    last_err: Optional[Exception] = None
    for name in solver_names:
        try:
            with Solver(name=name, bootstrap_with=cnf.clauses, use_timer=False) as s:
                # Some solvers support builtin timeouts via 'solve_limited' budgets, but
                # for our small instances we solve normally.
                sat = s.solve()
                if not sat:
                    return "UNSAT", {}
                model = s.get_model() or []
                assign: Dict[int, bool] = {}
                for lit in model:
                    if lit == 0:
                        continue
                    var = abs(lit)
                    assign[var] = lit > 0
                return "SAT", assign
        except Exception as e:  # try next backend
            last_err = e
            continue
    raise RuntimeError(f"No usable PySAT solver backend found: {last_err}")


def have_kissat_binary() -> bool:
    return shutil.which("kissat") is not None


def solve_with_kissat_external(
    cnf: "SatCNF",
    time_limit_s: Optional[float] = None,
    progress: bool = False,
    seed: Optional[int] = None,
) -> Tuple[str, Dict[int, bool]]:
    """Use local kissat.py wrapper to solve via external Kissat binary."""
    import kissat as kissat_mod  # local module providing the wrapper

    return kissat_mod.solve_with_kissat(
        cnf, time_limit_s=time_limit_s, progress=progress, seed=seed
    )


def extract_solution(meta: Dict[str, object], assign: Dict[int, bool]) -> Solution:
    N = int(meta["N"])  # type: ignore[index]
    D = int(meta["D"])  # type: ignore[index]
    pool: IDPool = meta["pool"]  # type: ignore[index]
    port_keys: Set[Tuple[int, int]] = meta["port_keys"]  # type: ignore[index]

    # labels
    rooms: List[int] = []
    for r in range(N):
        lab_val = None
        b0 = assign.get(pool.id(("L", r, 0)), False)
        b1 = assign.get(pool.id(("L", r, 1)), False)
        lab_val = bits2label(b0, b1)
        rooms.append(int(lab_val))

    # connections
    connections: List[Dict[str, Dict[str, int]]] = []
    for a, b in port_keys:
        var = pool.id(("P", a, b))
        if assign.get(var, False):
            ri, di = divmod(a, D)
            rj, dj = divmod(b, D)
            connections.append(
                {
                    "from": {"room": int(ri), "door": int(di)},
                    "to": {"room": int(rj), "door": int(dj)},
                }
            )
    connections.sort(
        key=lambda e: (
            e["from"]["room"],
            e["from"]["door"],
            e["to"]["room"],
            e["to"]["door"],
        )
    )
    return {
        "status": 1 if connections else 0,
        "rooms": rooms,
        "startingRoom": STARTING_ROOM_ID,
        "connections": connections,
    }


def verify_solution(
    plans: List[List[int]], results: List[List[int]], N: int, out: Solution
) -> Tuple[bool, List[str]]:
    D = 6
    P = N * D
    errs: List[str] = []

    # Build matching array
    match = [-1] * P
    for c in out.get("connections", []):  # type: ignore[union-attr]
        ri = int(c["from"]["room"])  # type: ignore[index]
        di = int(c["from"]["door"])  # type: ignore[index]
        rj = int(c["to"]["room"])  # type: ignore[index]
        dj = int(c["to"]["door"])  # type: ignore[index]
        p = ri * D + di
        q = rj * D + dj
        if match[p] not in (-1, q):
            errs.append(f"port {p} matched inconsistently: {match[p]} vs {q}")
        if match[q] not in (-1, p):
            errs.append(f"port {q} matched inconsistently: {match[q]} vs {p}")
        match[p] = q
        match[q] = p

    for p in range(P):
        if match[p] == -1:
            ri, di = divmod(p, D)
            errs.append(f"unmatched port: room={ri}, door={di}")

    labels = out.get("rooms", [])  # type: ignore[assignment]
    if len(labels) != N:
        errs.append(f"rooms length mismatch: got {len(labels)} want {N}")

    # simulate
    for idx, (pl, rs) in enumerate(zip(plans, results)):
        cur = STARTING_ROOM_ID
        if labels and labels[cur] != int(rs[0]):
            errs.append(f"plan {idx} step 0: label mismatch at room {cur}")
        for t, a in enumerate(pl):
            p = cur * D + a
            q = match[p]
            if q == -1:
                errs.append(f"plan {idx} step {t}: unmatched port p={p}")
                break
            cur = q // D
            exp = int(rs[t + 1])
            if labels and labels[cur] != exp:
                errs.append(f"plan {idx} step {t+1}: label mismatch at room {cur}")

    return len(errs) == 0, errs


def cegis_sat(
    plans: List[List[int]],
    results: List[List[int]],
    N: int,
    *,
    init_prefix: int = 10,
    max_iters: int = 30,
    time_limit_s: Optional[float] = None,
    verbose: bool = True,
    backend: str = "auto",  # auto|kissat|pysat
) -> Tuple[Optional[Solution], Dict[str, object]]:
    # initialize per-trace prefixes
    prefixes = [max(20, len(pl)) for pl in plans]
    prefixes[0] = len(plans[0])  # always use full first trace

    for it in range(max_iters):
        chosen = backend
        if backend == "auto":
            if have_kissat_binary():
                chosen = "kissat"
            else:
                chosen = "pysat"
        if chosen not in ("kissat", "pysat"):
            raise ValueError(f"unknown backend: {chosen}")

        cnf, meta = build_cnf_prefix(plans, results, N, prefixes)
        if chosen == "kissat":
            status, assign = solve_with_kissat_external(
                cnf, time_limit_s=time_limit_s, progress=verbose
            )
            if verbose:
                print(f"[iter {it}] kissat status: {status}, prefix={prefixes}")
        else:
            status, assign = solve_with_pysat(cnf, time_limit_s=time_limit_s)
            if verbose:
                print(f"[iter {it}] pysat status: {status}, prefix={prefixes}")
        if status != "SAT":
            return None, {
                "status": status,
                "iter": it,
                "prefix": prefixes,
                "backend": chosen,
            }
        out = extract_solution(meta, assign)
        ok, errs = verify_solution(plans, results, N, out)
        if ok:
            return out, {"status": "FEASIBLE", "iter": it, "prefix": prefixes}

        # find earliest mismatch per trace, increase that prefix
        # For simplicity, re-simulate and find first differing step
        # using the decoded solution
        # Build move map for quick sim
        D = 6
        P = N * D
        match = [-1] * P
        for c in out.get("connections", []):  # type: ignore[union-attr]
            ri = int(c["from"]["room"])  # type: ignore[index]
            di = int(c["from"]["door"])  # type: ignore[index]
            rj = int(c["to"]["room"])  # type: ignore[index]
            dj = int(c["to"]["door"])  # type: ignore[index]
            p = ri * D + di
            q = rj * D + dj
            match[p] = q
            match[q] = p
        labels = out["rooms"]  # type: ignore[index]

        updated = False
        for tid, (pl, rs) in enumerate(zip(plans, results)):
            cur = STARTING_ROOM_ID
            want = rs
            got = [labels[cur]]
            for a in pl:
                p = cur * D + a
                q = match[p]
                if q == -1:
                    # diverged at next step; set i* to current step
                    break
                cur = q // D
                got.append(labels[cur])
            # find first mismatch
            i_star = None
            for i, (x, y) in enumerate(zip(got, want)):
                if int(x) != int(y):
                    i_star = i
                    break
            if i_star is None:
                # If we ran out of got due to unmatched, treat as mismatch at len(got)
                if len(got) < len(want):
                    i_star = len(got)
            if i_star is not None:
                old = prefixes[tid]
                prefixes[tid] = max(old, i_star)
                updated = True
                if verbose:
                    print(
                        f"  -> counterexample on trace {tid} at i*={i_star}, prefix={prefixes[tid]}"
                    )

        if not updated:
            # Should not happen if verify failed, but guard anyway
            break

    return None, {"status": "MAX_ITERS", "iter": max_iters, "prefix": prefixes}


def main() -> None:
    parser = argparse.ArgumentParser(description="CEGIS (PySAT) solver")
    parser.add_argument(
        "--input", "-i", type=str, required=True, help="Input trace JSON path"
    )
    parser.add_argument(
        "--output", "-o", type=str, required=True, help="Output map JSON path"
    )
    parser.add_argument("--iters", type=int, default=30, help="Max CEGIS iterations")
    parser.add_argument(
        "--init-prefix", type=int, default=10, help="Initial prefix length per trace"
    )
    parser.add_argument("--quiet", action="store_true", help="Reduce logging output")
    parser.add_argument(
        "--backend",
        choices=["auto", "kissat", "pysat"],
        default="auto",
        help="SAT backend to use",
    )
    args = parser.parse_args()

    prob = load_problem(args.input)
    plans = prob["plans"]  # type: ignore[index]
    results = prob["results"]  # type: ignore[index]
    N = int(prob["N"])  # type: ignore[index]

    out, meta = cegis_sat(
        plans,
        results,
        N,
        init_prefix=args.init_prefix,
        max_iters=args.iters,
        verbose=not args.quiet,
        backend=args.backend,
    )
    if out is None:
        print("CEGIS failed:", meta)
        return

    out_path = Path(args.output)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    with open(out_path, "w") as f:
        json.dump(out, f, ensure_ascii=False, indent=2)
    if not args.quiet:
        print("Wrote:", out_path)


if __name__ == "__main__":
    main()
