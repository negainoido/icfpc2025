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
D = 6


def normalize_stage1_plan(plan: str) -> List[int]:
    return [int(ch) for ch in plan]

def normalize_stage2_plan(plan: str) -> tuple[List[int], List[int]]:
    doors = []
    overwrites = []    
    curr = 0
    while curr < len(plan):
        doors.append(int(plan[curr]))
        curr += 1
        if curr < len(plan) and plan[curr] == '[':
            curr += 1
            overwrites.append(int(plan[curr]))
            curr += 1
            assert curr < len(plan) and plan[curr] == ']', f"Invalid overwrite syntax in plan at position {curr}: missing ']'"
            curr += 1
        else:
            raise RuntimeError("We always overwrite: missing '['")
    return doors, overwrites


def normalize_stage2_result(result: list[int], plan: tuple[List[int], List[int]]) -> List[int]:
    doors, overwrites = plan


    normalized_result = [result[0]]
    curr = 1
    for i in range(len(doors)):
        normalized_result.append(result[curr])
        curr += 1
        assert overwrites[i] is not None
        curr += 1
    assert curr == len(result), f"Result length {len(result)} does not match expected length {curr} from plan"
    return normalized_result


def build_stage1_cnf(
    plans: List[List[int]],
    results: List[List[int]],
    N: int,
    D: int = 6,
    progress: bool = False,
    prefix_steps: Optional[int] = None,
) -> Tuple[SatCNF, Dict[str, any]]:
    assert len(plans) == 1
    assert len(plans) == len(results)

    # Validate and optionally truncate to prefix_steps
    used_plans: List[List[int]] = []
    used_results: List[List[int]] = []
    for idx, (plan, r) in enumerate(zip(plans, results)):
        if prefix_steps is None:
            T = len(plan)
        else:
            if prefix_steps < 0:
                raise ValueError("prefix_steps must be non-negative")
            T = min(prefix_steps, len(plan))
        if len(r) < T + 1:
            raise ValueError(
                f"results[{idx}] too short for requested prefix: need {T+1}, got {len(r)}"
            )
        used_plans.append(plan[:T])
        used_results.append(r[: T + 1])

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

    # For the starting room, fix the label to each plan's first observation
    for obs in used_results:
        cnf.append([label_assign_var(STARTING_ROOM_ID, int(obs[0]))])

    # Balanced distribution: for each label bits in {0,1,2,3}, enforce at least floor(N/4)
    base = N // 4
    if base > 0:
        for bits in range(4):
            lits = [label_assign_var(rid, bits) for rid in range(N)]
            enc = CardEnc.atleast(lits=lits, bound=base, vpool=pool, encoding=EncType.seqcounter)
            cnf.extend(enc.clauses)


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
    for trace_id, (plan, obs) in enumerate(zip(used_plans, used_results)):

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
                    cnf.append([-X[trace_id][t][from_rid], -X[trace_id][t + 1][to_rid], m])
                    
        # Knowledge-based pruning: if the future identical action prefixes diverge in labels,
        # then positions immediately after t1 and t2 cannot be the same room.
        added = 0
        T = len(plan)
        for t1 in range(T):
            for t2 in range(t1 + 1, T):
                if obs[t1 + 1] != obs[t2 + 1]:
                    continue
                # find longest k >= 1 such that plan[t1+1..t1+k] == plan[t2+1..t2+k]
                k = 0
                j = 1
                while (t1 + j) < T and (t2 + j) < T and plan[t1 + j] == plan[t2 + j]:
                    k += 1
                    j += 1
                if k == 0:
                    continue
                # if any idx in 1..k yields differing observed labels, enforce inequality at t1+1 vs t2+1
                differing = False
                for i in range(1, k + 1):
                    if int(obs[t1 + 1 + i]) != int(obs[t2 + 1 + i]):
                        differing = True
                        break
                if not differing:
                    continue
                for rid in range(N):
                    cnf.append([-X[trace_id][t1 + 1][rid], -X[trace_id][t2 + 1][rid]])
                    added += 1
        if progress and added:
            print(f"[kissat] added {added} pruning binary clauses for trace {trace_id}")



    # Ensure nv is at least the top variable id
    cnf.nv = max(getattr(cnf, 'nv', 0) or 0, pool.top)
    meta = {
        "N": N,
        "D": D,
        "P": P,
        "plans": used_plans,
        "results": used_results,
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
    seed: Optional[int] = None,
) -> Tuple[str, Dict[int, bool]]:
    """Solve CNF with external 'kissat' binary. Returns (status, assignment). status in {SAT, UNSAT, UNKNOWN}"""

    with tempfile.TemporaryDirectory() as td:
        cnf_path = os.path.join(td, "problem.cnf")
        # write DIMACS using PySAT utility
        cnf.to_file(cnf_path)
        cmd = ["kissat", "-q"]
        # Try to pass time limit if supported
        if time_limit_s and time_limit_s > 0:
            # Many builds support '--time=SECONDS'
            cmd.append(f"--time={int(time_limit_s)}")
        if seed is not None:
            cmd.append(f"--seed={int(seed)}")
        cmd.append(cnf_path)
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


def build_stage2_cnf(
    stage1_rooms: List[int],
    stage1_connections: List[Dict[str, Dict[str, int]]],
    plan: tuple[list[int], list[int]],
    result: list[int],
    C: int,
    progress: bool = False,
) -> tuple[SatCNF, dict[str, any]]:
    """
    Stage 1で得られたグラフから真のグラフを構成する。
    真のグラフはStage1のグラフの各ノードをC個にコピーしたもの。
    各辺についてコピーしたグラフの対応する辺とランダムに交差させる。

    下記の解法を実装する。
    * plan ではランダムウォークとランダムなラベル書き換えを行っており、移動と書き換えのリストのペアとなっている
    * この結果を元にSATとして定式化する。
    * 論理式は Stage1 と同様に PySAT の CNF 形式で構築する

    変数
     * M (n, c, d’, c’): G1のノードnのコピーcからドアd’を開けたときにコピーc’に遷移する
     * X (t, c) tステップ後にc個目のコピーにいる
    制約
     * \sum_{c’} M (n, c, d’, c’) == 1
     * \sum_{c} X(t, c) == 1
     * X(t, c) -> (X(t + 1, c’) == M (n(t), c, d(t), c’)) 
       * n(t): tステップ後にいるノード (G1の中のノードなので2の結果から定まる) 
       * d(t): tステップ目であけるドア
     * X(0, 0) == 1
     * 2つ目のプランから得られる同一性に関する制約
       * 準備
         * t 回目の移動後に観測されたラベルを L(t) とする (t = 0, …, T) 
         * t 回目の移動のあとに上書きしたラベルを U(t) とする (t = 0, …, T) 
         メモ: U(t) 2つめのランダムウォークにおいてランダム決まっています
       * 各 t に対して t’ < t でかつ n(t’) == n(t) ∧ U(t’) == L(t) となる最大の t’ を見つける 
       * （異なり条件）t’ < t’’ < t でかつ n(t’’) == n(t) となる各 t’’ に対して
         * X (t, c) != X(t’’, c) 
       *（同一条件）
         * 準備
           * t 回目の移動直後における各ノード n のC個のコピーにかかれている 2-bit ラベル l のカウントを Count (t, n, l) と書く。
         * C (t’, n (t), U(t’)) == 0 であれば、ステップ t に観測された L(t) はステップ  t’ に書き込まれた U(t’) (t’の定義から L(t)) であることが分かるので
           * X(t, c) == X(t’, c) 

    """
    pass


def extract_stage1_solution(meta: Dict[str, any], assign: Dict[int, bool]) -> Dict[str, any]:
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
    parser.add_argument("--seed", type=int, default=None, help="Random seed passed to Kissat (--seed)")
    parser.add_argument("--prefix-steps", type=int, default=None, help="Use only the first K steps of each plan when building CNF")
    parser.add_argument("--progress", "-p", action="store_true", help="Print progress logs")
    args = parser.parse_args()

    if args.input:
        with open(args.input, "r") as f:
            data = json.load(f)
    else:
        data = json.load(sys.stdin)

    assert len(data["plans"]) == 2


    print("[Stage1]: Find the underlying structure…")
    stage1_plan = normalize_stage1_plan(data["plans"][0])
    stage1_result = data["results"][0]
    N = data["N"]
    C = data["C"]
    assert N % C == 0, f"N={N} must be multiple of C={C}"
    stage1_N = N // C

    if args.progress:
        print(f"[kissat] Building CNF… N={N}, time={args.time}s")
    if args.progress and args.prefix_steps is not None:
        print(f"[kissat] Using only the first {args.prefix_steps} steps of each plan")
    cnf, meta = build_stage1_cnf([stage1_plan], [stage1_result], stage1_N, progress=args.progress, prefix_steps=args.prefix_steps)
    if args.progress:
        print("[kissat] Solving…")
    status, assign = solve_with_kissat(
        cnf,
        time_limit_s=args.time,
        progress=args.progress,
        seed=args.seed,
    )
    if args.progress:
        print(f"[kissat] Solve status: {status}")

    if status != "SAT":
        out = {"status": 0, "error": f"Kissat returned {status}"}
    else:
        out = extract_stage1_solution(meta, assign)
        # Post-verify against FULL plans/results (not truncated by --prefix-steps)
        if args.progress and args.prefix_steps is not None:
            print("[verify] Using full plans/results for verification (ignoring prefix truncation).")
        ok, errs = verify_solution([stage1_plan], [stage1_result], stage1_N, out, progress=args.progress)
        out["verified"] = bool(ok)
        if not ok:
            out["verifyErrors"] = errs

    print("[Stage2]: Assign copy ID to each room…")
    stage1_rooms = out["rooms"]
    stage1_connections = out["connections"]
    stage2_plan = normalize_stage2_plan(data["plans"][1])
    stage2_result = normalize_stage2_result(data["results"][1], stage2_plan)
    cnf, meta = build_stage2_cnf(stage1_rooms, stage1_connections, stage2_plan, stage2_result, C=C, progress=args.progress)


    if args.output:
        with open(args.output, "w") as f:
            json.dump(out, f, ensure_ascii=False, indent=2)
    else:
        print(json.dumps(out, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
