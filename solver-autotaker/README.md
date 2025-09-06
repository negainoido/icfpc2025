# solver-autotaker CLI

ICFPC 2025 向け焼きなましベース解法（MVP）。標準入力/出力またはファイルを介して JSON を読み書きします。

## ビルドと実行
- ビルド: `cargo build -p solver-autotaker`
- 実行例（基本）: `cargo run -p solver-autotaker -- -i input.json -o out.json`
- 実行例（詳細ログ・進捗）: `cargo run -p solver-autotaker -- -i input.json -o out.json -v 1 --log-every 10000`

## オプション一覧
- `-v, --verbose <u8>`: ログ出力レベル。`0`=静か（既定）、`1`=進捗ログ、`2`=各ムーブのデバッグ（大量）。
- `--seed <u64>`: 乱数シード。未指定なら時刻から導出。
- `-i, --input <path>`: 入力 JSON パス。`"-"` で標準入力（既定: `-`）。
- `-o, --output <path>`: 出力 JSON パス。`"-"` で標準出力（既定: `-`）。
- `--iters <usize>`: 焼きなまし反復回数（既定: `50000`）。
- `--lambda-bal <f32>`: ラベル分布のバランス罰則の重み（既定: `1.0`）。
- `--time-limit <sec>`: 秒数での時間制限。到達時に途中停止。
- `--log-every <N>`: 進捗ログの出力間隔（`-v 1` 以上で有効）。
- `--save-every <N>`: ベスト更新スナップショット保存間隔。`--output` をベース名として `-NNNNNN` 付きで保存。
- `--t0 <f32>`: 初期温度（既定: `1.0`）。
- `--alpha <f32>`: 冷却率（1 に近いほどゆっくり、既定: `0.999`）。
- `--tmin <f32>`: 最低温度クランプ（既定: `1e-4`）。
- `--restarts <usize>`: マルチスタート回数。各回で初期解から独立に焼きなまし（既定: `1`）。
- `--reheat-every <N>`: ベスト更新が `N` 反復連続で停滞したら温度を再上昇（リヒート）。
- `--reheat-to <f32>`: リヒート後の温度。未指定なら `t0*0.1`（現在温度より低い場合は据え置き）。

## ログ出力の目安
- `-v 0`: 進捗なし。標準出力は最終 JSON のみ（`-o -` 時）。
- `-v 1`: 開始/進捗/保存/終了時の概要を `stderr` に出力。
- `-v 2`: 各ムーブごとの `dE`, `T`, `acc` など詳細を `stderr` に出力（大量）。

## スナップショット保存
- `--save-every N` を指定すると、`--output` のパスをベースに `name-000000.json` の形式でベスト解を保存します。
- 例: `--output smt-guessor/example/map-probatio-solver.json` の場合、`map-probatio-solver-010000.json` 等が生成。
- マルチスタート（`--restarts > 1`）時は各リスタートで同じベース名に保存されるため、上書きや混在が気になる場合はベース名を変更してください。

## チューニング例
- 局所最適からの脱出を重視: `--t0 2.0 --alpha 0.9999 --tmin 0.001 --reheat-every 5000 --reheat-to 0.02 --restarts 20`
- 挙動確認（詳細ログ）: `--iters 2000 -v 2 --log-every 200`

## 入出力
- 入力: `plans`, `results`, `N`, `startingRoom` を持つ JSON。
- 出力: `rooms`（各部屋のラベル 0..=3）, `startingRoom`, `connections`（無向辺の両端ポート）を持つ JSON。
