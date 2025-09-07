# CEGIS + CP-SAT (mate式) の最小実装テンプレ
# - 入力: /mnt/data/probatio.json （plans/results/N/startingRoom）
# - 参考: /mnt/data/map-probatio.json（出力フォーマット例）
# - 出力: /mnt/data/map-found.json（見つかったグラフ）
#
# 注意:
# - この環境に ortools が無い場合はインポートで失敗します。その際はコードだけ生成されます。

import argparse
import json
from pathlib import Path
from ortools.sat.python import cp_model


def load_problem(path="/mnt/data/probatio.json"):
    with open(path, "r") as f:
        data = json.load(f)
    traces = []
    for plan, obs in zip(data["plans"], data["results"]):
        # plan は "012345..." 文字列、obs は int 列（長さ = len(plan)+1）
        traces.append({"plan": [int(ch) for ch in plan], "obs": obs})
    N = data["N"]
    s0 = data.get("startingRoom", 0)
    return {"traces": traces, "N": N, "s0": s0}


def pid(q, d):
    return q * 6 + d


def room_of(p, N):
    return p // 6


def door_of(p):
    return p % 6


def simulate(mu, labels_room, s0, plan):
    """mu: list[int] mate[p]、labels_room: list[int] g(q)"""
    s = s0
    outs = [labels_room[s]]
    for a in plan:
        p = pid(s, a)
        p_to = mu[p]
        s = room_of(p_to, len(labels_room))
        outs.append(labels_room[s])
    return outs


def build_model(
    problem,
    extra_constraints=None,
    *,
    use_lex_symmetry=False,
    use_pattern_c=True,
    path_prefix=None,
):
    """CEGIS本体が足す追加制約(extra_constraints)を受け取ってモデルを再構築"""
    traces = problem["traces"]
    N = problem["N"]
    s0 = problem["s0"]

    model = cp_model.CpModel()

    # 変数
    label_room = [model.NewIntVar(0, 3, f"labelRoom[{q}]") for q in range(N)]
    P = 6 * N
    label_port = [model.NewIntVar(0, 3, f"labelPort[{p}]") for p in range(P)]
    mate = [model.NewIntVar(0, P - 1, f"mate[{p}]") for p in range(P)]

    # 定数配列
    roomOf = [room_of(p, N) for p in range(P)]  # int 配列

    # 1) 対合（自己ループ許可）: mate[mate[p]] == p
    for p in range(P):
        back = model.NewIntVar(0, P - 1, f"back_of_{p}")
        model.AddElement(mate[p], mate, back)  # back = mate[mate[p]]
        model.Add(back == p)

    # 2) チャネリング: labelPort[p] == labelRoom[ roomOf[mate[p]] ]
    for p in range(P):
        rm = model.NewIntVar(0, N - 1, f"roomOfMate_{p}")
        model.AddElement(mate[p], roomOf, rm)  # rm = roomOf[mate[p]]
        model.AddElement(rm, label_room, label_port[p])

    # 3) ラベル割当（パターンC）
    # N = 4q + r
    q, r = divmod(N, 4)
    if use_pattern_c:
        # π(i)=i として決め打ち。o = 最初のトレースの先頭観測 b0 を使用
        o = traces[0]["obs"][0] if traces else 0
        regular = 4 * q
        for i in range(regular):
            model.Add(label_room[i] == (o + i) % 4)

    # 端数 r の個数制約: 各ラベルの合計 = q + y_ell,  sum(y)=r
    # y_ell は Bool
    y = [model.NewBoolVar(f"y[{ell}]") for ell in range(4)]

    # Sum(y) == r
    model.Add(sum(y) == r)

    # 各ラベルのカウント
    # label_room[q] == ell を Bool 化して合計する
    for ell in range(4):
        bs = []
        for qq in range(N):
            b = model.NewBoolVar(f"is_{ell}_at_{qq}")
            # label_room[qq] == ell  <=> b
            model.Add(label_room[qq] == ell).OnlyEnforceIf(b)
            # != の場合の網羅: CP-SATでは等式の反対は簡単に書けないので4値→1値化のため、
            # ここは "IfNot" のみでOK（他のellで別の b が立つ）
            model.Add(label_room[qq] != ell).OnlyEnforceIf(b.Not())
            bs.append(b)
        # 合計 = q + y[ell]
        model.Add(sum(bs) == q + y[ell])

    # 4) 開始点の固定（各トレース）
    for t_idx, t in enumerate(traces):
        obs = t["obs"]
        plan = t["plan"]
        model.Add(label_room[s0] == obs[0])
        if len(plan) >= 1:
            model.Add(label_port[pid(s0, plan[0])] == obs[1])

    # 5) 局所窓（長さ3の存在制約）: 各トレース・各 i=1..k-1
    for t_idx, t in enumerate(traces):
        plan = t["plan"]
        obs = t["obs"]
        k = len(plan)
        for i in range(1, k):
            # 存在 OR: ∨_{q in S, d in D}  ( labelRoom[q]==b_i ∧ labelPort[q,a_i]==b_{i+1} ∧ labelPort[q,d]==b_{i-1} )
            lits = []
            for qid in range(N):
                for d in range(6):
                    sel = model.NewBoolVar(f"win_t{t_idx}_i{i}_q{qid}_d{d}")
                    # sel => 各条件
                    model.Add(label_room[qid] == obs[i]).OnlyEnforceIf(sel)
                    model.Add(
                        label_port[pid(qid, plan[i])] == obs[i + 1]
                    ).OnlyEnforceIf(sel)
                    model.Add(label_port[pid(qid, d)] == obs[i - 1]).OnlyEnforceIf(sel)
                    # sel が偽なら条件を緩める必要は無い（OnlyEnforceIf なので拘束されない）
                    lits.append(sel)
            # 少なくとも1つ選べ
            model.Add(sum(lits) >= 1)

    # 6) 同一ラベル内のレキシコ順（軽めの対称性除去）
    if use_lex_symmetry:
        # 実装簡易化のため、(q<q') & label等しい ⇒ 6個の逐次比較 を reify で表現
        for q1 in range(N):
            for q2 in range(q1 + 1, N):
                same = model.NewBoolVar(f"sameLabel_{q1}_{q2}")
                model.Add(label_room[q1] == label_room[q2]).OnlyEnforceIf(same)
                model.Add(label_room[q1] != label_room[q2]).OnlyEnforceIf(same.Not())
                # 簡易: same ⇒ (各dで port値の非減少)。必要時のみ有効化。
                for d in range(6):
                    model.Add(
                        label_port[pid(q1, d)] <= label_port[pid(q2, d)]
                    ).OnlyEnforceIf(same)

    # 追加制約（CEGISの反例で積み増し）
    if extra_constraints:
        for c in extra_constraints:
            typ = c["type"]
            if typ == "fix_mate":
                p, p_to = c["p"], c["p_to"]
                model.Add(mate[p] == p_to)
            elif typ == "fix_label_room":
                q, val = c["q"], c["val"]
                model.Add(label_room[q] == val)
            elif typ == "fix_label_port":
                p, val = c["p"], c["val"]
                model.Add(label_port[p] == val)
            else:
                raise ValueError(f"Unknown extra constraint type: {typ}")

    # パス接地制約（長さ prefix の実在パスを要求）
    if path_prefix:
        for t_idx, L in path_prefix.items():
            t = traces[t_idx]
            L = min(L, len(t["plan"]))
            # r[0..L] ∈ [0..N-1]
            r = [model.NewIntVar(0, N - 1, f"r_t{t_idx}_{j}") for j in range(L + 1)]
            # 開始部屋
            model.Add(r[0] == s0)
            # 観測ラベルと一致
            for j in range(L + 1):
                tmp = model.NewIntVar(0, 3, f"obs_t{t_idx}_{j}")
                model.AddElement(r[j], label_room, tmp)
                model.Add(tmp == t["obs"][j])
            # 遷移 r[j+1] = roomOf[mate[ 6*r[j] + a_j ]]
            for j in range(L):
                p_idx = model.NewIntVar(0, 6 * N - 1, f"pidx_t{t_idx}_{j}")
                model.Add(p_idx == r[j] * 6 + t["plan"][j])
                p_to = model.NewIntVar(0, 6 * N - 1, f"pto_t{t_idx}_{j}")
                model.AddElement(p_idx, mate, p_to)
                next_r = model.NewIntVar(0, N - 1, f"nextr_t{t_idx}_{j}")
                model.AddElement(p_to, roomOf, next_r)
                model.Add(r[j + 1] == next_r)

    return model, label_room, label_port, mate


def cegis_solve(
    problem,
    max_iters=20,
    time_limit_s=10.0,
    verbose=True,
    *,
    use_lex_symmetry=False,
    use_pattern_c=True,
    snapshot_output: Path | None = None,
):
    extra = []  # 反例から積む制約（ports固定モード用）
    path_prefix = {}  # t_idx -> L（パス接地モード用）
    fixed_room = {}
    fixed_port = {}
    for it in range(max_iters):
        model, label_room, label_port, mate = build_model(
            problem,
            extra_constraints=extra,
            use_lex_symmetry=use_lex_symmetry,
            use_pattern_c=use_pattern_c,
            path_prefix=path_prefix,
        )

        solver = cp_model.CpSolver()
        solver.parameters.max_time_in_seconds = time_limit_s
        solver.parameters.num_search_workers = 8

        res = solver.Solve(model)
        if verbose:
            print(f"[iter {it}] status:", solver.StatusName(res))
            try:
                stats = solver.ResponseStats()
                print(f"[iter {it}] cpsat:\n{stats}")
            except Exception:
                # Fallback: minimal counters if ResponseStats is unavailable
                try:
                    print(
                        f"[iter {it}] cpsat: wall={solver.WallTime():.3f}s, "
                        f"conflicts={getattr(solver, 'NumConflicts', lambda: 'n/a')()}, "
                        f"branches={getattr(solver, 'NumBranches', lambda: 'n/a')()}"
                    )
                except Exception:
                    pass

        if res not in (cp_model.OPTIMAL, cp_model.FEASIBLE):
            return None, {"status": solver.StatusName(res), "iter": it, "extra": extra}

        # 候補を取り出し
        N = problem["N"]
        P = 6 * N
        labels_room = [solver.Value(label_room[q]) for q in range(N)]
        mu = [solver.Value(m) for m in mate]

        # スナップショット保存（各反復の候補解）
        if snapshot_output is not None:
            snap = _suffixed_path(Path(snapshot_output), it)
            _write_map_json(snap, labels_room, mu, problem["s0"], N)

        # 検証
        traces = problem["traces"]
        s0 = problem["s0"]
        any_fail = False
        for t_idx, t in enumerate(traces):
            want = t["obs"]
            got = simulate(mu, labels_room, s0, t["plan"])
            if got != want:
                any_fail = True
                # 最短失敗位置 i*
                i_star = next(i for i, (x, y) in enumerate(zip(got, want)) if x != y)
                # 反例位置までの実在パスを要求（アンカーを強くしていく）
                prev = path_prefix.get(t_idx, 0)
                path_prefix[t_idx] = max(prev, i_star)
                if verbose:
                    print(f"  -> counterexample on trace {t_idx} at i*={i_star}, path_prefix={path_prefix[t_idx]}")
                break

        if not any_fail:
            # 解が見つかった
            return {"labels_room": labels_room, "mu": mu}, {
                "status": "FEASIBLE",
                "iter": it,
                "extra_cnt": len(extra),
            }

    return None, {"status": "MAX_ITERS", "iter": max_iters, "extra": extra}


def mu_to_connections(mu, N):
    """mate配列から出力JSONの connections へ変換（重複を排除）"""
    seen = set()
    conns = []
    for q in range(N):
        for d in range(6):
            p = pid(q, d)
            p2 = mu[p]
            q2, d2 = room_of(p2, N), door_of(p2)
            key = tuple(sorted([(q, d), (q2, d2)]))
            if key in seen:
                continue
            seen.add(key)
            conns.append(
                {"from": {"room": q, "door": d}, "to": {"room": q2, "door": d2}}
            )
    return conns


def _suffixed_path(base: Path, it: int) -> Path:
    suf = f"-{it:02d}"
    if base.suffix:
        return base.with_name(base.stem + suf + base.suffix)
    return base.with_name(base.name + suf)


def _write_map_json(out_path: Path, labels_room, mu, s0, N):
    out = {
        "rooms": list(labels_room),
        "startingRoom": s0,
        "connections": mu_to_connections(list(mu), N),
    }
    out_path.parent.mkdir(parents=True, exist_ok=True)
    with open(out_path, "w") as f:
        json.dump(out, f, ensure_ascii=False, indent=2)


def main():
    parser = argparse.ArgumentParser(description="CEGIS CP-SAT solver")
    parser.add_argument(
        "--input",
        "-i",
        type=str,
        default="/mnt/data/probatio.json",
        help="Input trace JSON (plans/results/N/startingRoom)",
    )
    parser.add_argument(
        "--output",
        "-o",
        type=str,
        default="/mnt/data/map-found.json",
        help="Output map JSON path",
    )
    parser.add_argument(
        "--iters",
        type=int,
        default=20,
        help="Max CEGIS iterations",
    )
    parser.add_argument(
        "--time-limit",
        type=float,
        default=10.0,
        help="CP-SAT solve time limit per iteration (seconds)",
    )
    parser.add_argument(
        "--quiet",
        action="store_true",
        help="Reduce logging",
    )
    parser.add_argument(
        "--lex-sym",
        action="store_true",
        help="Enable lexicographic symmetry breaking within equal labels",
    )
    parser.add_argument(
        "--no-pattern-c",
        action="store_true",
        help="Disable pattern-C fixed prefix labeling",
    )
    parser.add_argument(
        "--pattern-c",
        action="store_true",
        help="Enable pattern-C fixed prefix labeling (overrides --no-pattern-c)",
    )
    args = parser.parse_args()

    prob = load_problem(args.input)

    use_pattern_c = (args.pattern_c and not args.no_pattern_c)
    sol, meta = cegis_solve(
        prob,
        max_iters=args.iters,
        time_limit_s=args.time_limit,
        verbose=not args.quiet,
        use_lex_symmetry=args.lex_sym,
        use_pattern_c=use_pattern_c,
        snapshot_output=Path(args.output),
    )
    if sol is None:
        print("CEGIS 失敗:", meta)
        return

    N = prob["N"]
    out_path = Path(args.output)
    _write_map_json(out_path, sol["labels_room"], sol["mu"], prob["s0"], N)
    print("解を書き出しました:", str(out_path))


if __name__ == "__main__":
    main()
