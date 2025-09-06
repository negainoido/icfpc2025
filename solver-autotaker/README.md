solver-autotaker — 実行方法

概要
- ALGORITHM.md に基づく焼きなまし（2-opt + ラベル入替）ソルバーの MVP 実装です。
- 入力は探索プランと観測結果（plans/results）。出力は部屋ラベルとポート間の無向マッチング（connections）。

ビルド
- `cargo build -p solver-autotaker`

実行
- `cargo run -p solver-autotaker -- --input in.json --output out.json [オプション]`

主なオプション
- `--iters <N>`: 焼きなまし反復回数（既定: 50000）
- `--lambda-bal <f32>`: ラベル分布ペナルティ重み（既定: 1.0）
- `--seed <u64>`: 乱数シード（省略可）
- `--time-limit <sec>`: 早期停止の秒数（省略可）
- `-v, --verbose <0..>`: ログ詳細度（既定: 0）
- `-i, --input <path>`: 入力 JSON（`-` で stdin、既定: `-`）
- `-o, --output <path>`: 出力 JSON（`-` で stdout、既定: `-`）

入力フォーマット（JSON）
```jsonc
{
  "plans": ["0325", "510"],
  "results": [[0,2,1,3,0], [0,1,0]],
  "N": 64,
  "startingRoom": 0
}
```
- `plans`: 0–5 または 1–6 の数字列。`[d]` 形式のグラフィティは読み取りますが、MVP では書換え効果は無視します（将来対応）。
- `results`: 各プランで訪問した時のラベル列（長さは `|plan|+1`）。各値は 0..3。
- `N`: 部屋数。`startingRoom`: 開始部屋インデックス。

出力フォーマット（JSON）
```jsonc
{
  "rooms": [0,1,2,3, ...],
  "startingRoom": 0,
  "connections": [
    {"from": {"room": 0, "door": 0}, "to": {"room": 1, "door": 3}}
  ]
}
```
- `connections` はポート間の無向ペア（自己ループ可）。各ポートはちょうど 1 回現れます。

使用例
```bash
cargo run -p solver-autotaker -- \
  --input samples/in.json \
  --output out.json \
  --iters 100000 \
  --lambda-bal 1.0 \
  --seed 42
```

注意
- 初期解は観測ラベルをなるべく踏むように貪欲に配線し、残りは貪欲に閉じます。焼きなましで不一致とラベル偏りを下げます。
- グラフィティ（ラベル書換え）は ALGORITHM.md の仕様に記載がありますが、MVP では無視しています。必要なら対応を追加します。
- 入力検証: `results[k].len() == plans[k].len()+1`、`N>=1`、`startingRoom<N` をチェックします。

関連ドキュメント
- `../PROBLEM.md`: 問題仕様
- `./ALGORITHM.md`: 実装方針とメトリクス

