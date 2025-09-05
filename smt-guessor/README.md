# smt-guessor

ICFP 2025「Ædificium」向けの SMT ベース地図再構成ツールです。Z3 で経路ログから部屋と接続を推定し、任意で可視化します。

## セットアップ
- 前提: Python 3.9+、`pip`（任意で `uv`）
- 仮想環境（推奨）:
  - macOS/Linux: `python -m venv .venv && source .venv/bin/activate`
  - Windows (PowerShell): `python -m venv .venv; .venv\\Scripts\\Activate.ps1`
- 依存関係のインストール:
  - `uv` を使う場合（推奨）: `uv sync`
  - `pip` を使う場合: `pip install z3-solver matplotlib`

## 使い方（基本）
1. サンプル入力で推定を実行:
   - `python main.py --json example/sample.json --output out.map.json`
   - 既知の部屋数がある場合: `--N 3` を付与
   - 未知のときは `--minN 1 --maxN 128` で掃き出し（デフォルト同値）
2. 生成結果を確認: `out.map.json`（`rooms`, `startingRoom`, `connections` を出力）
3. 可視化（任意）:
   - `python visualize.py --input out.map.json --output map.png`

## コマンド例（まとめ）
```
# 1) 依存関係
uv sync                       # もしくは: pip install z3-solver matplotlib

# 2) 推定の実行
python main.py --json example/sample.json --output out.map.json --N 3
# N が不明ならスイープ
python main.py --json example/sample.json --minN 1 --maxN 64 --output out.map.json

# 3) 可視化
python visualize.py --input out.map.json --output map.png
```

## オプション
- `--json <path>`: 入力 JSON（`plans`, `results`, 任意で `N`, `startingRoom`）。
- `--output <path>`: 出力ファイルパス（省略時は標準出力）。
- `--N <int>`: 部屋数を固定。未指定なら `--minN..--maxN` で探索。
- `--minN <int>`, `--maxN <int>`: 掃き出し探索範囲の下限/上限。

## ヒント
- 入力 `plans` は `0..5` または `1..6` の文字列に対応（自動正規化）。
- 大きな探索範囲は時間がかかるため、`--N` が分かる場合は指定が高速です。
