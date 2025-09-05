# ICFPC 2025 Labyrinthine Library Solver

Rust製のICFPC 2025問題ソルバー。六角形の部屋で構成される迷宮図書館を最小限の探索回数でマッピングします。

## ビルド方法

```bash
cd solver
cargo build --release
```

## 使用方法

### 1. チーム登録

```bash
cargo run -- register --name "TeamName" --email "your@email.com" --pl "Rust"
```

登録後に表示されるIDを保存してください。

### 2. 問題を解く

#### テスト問題（probatio）を解く

```bash
cargo run -- test --id YOUR_TEAM_ID
```

#### 特定の問題を解く

```bash
cargo run -- solve --id YOUR_TEAM_ID --problem PROBLEM_NAME --max-queries 100
```

## アルゴリズム概要

### フェーズ1: 初期探索
- 開始部屋から全ドア（0-5）を探索
- 深さ2の全経路パターンを探索
- 基本的な構造を把握

### フェーズ2: スマート探索
- 未探索のドアを優先的に調査
- 状態遷移グラフを構築
- 効率的なバッチクエリで探索回数を最小化

### グラフ再構築
- 収集した探索データから部屋のグラフを構築
- 各部屋のラベルとドア接続を推定
- APIフォーマットに変換して提出

## 主要コンポーネント

- `main.rs`: CLIインターフェースとAPI通信
- `graph.rs`: グラフ構築と探索戦略の実装
  - `LibraryGraph`: 迷宮図書館のグラフ表現
  - `SmartExplorer`: 効率的な探索経路の生成

## 最適化のポイント

1. **バッチクエリ**: 複数のルートプランを一度に送信してペナルティを削減
2. **状態ベース探索**: 既に訪問した状態を追跡し、重複探索を回避
3. **未探索ドア優先**: 新しい情報が得られる可能性の高い経路を優先
4. **早期終了**: 正しいマップが構築できた時点で即座に終了