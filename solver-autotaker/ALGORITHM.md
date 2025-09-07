# 設計書（Rust, MVP）

目的：与えられた観測（`plans` と `results`）に一致し、ラベル分布制約を満たす「ポート付き無向グラフ（部屋 ×6 ポートの**完全マッチング**）」と部屋ラベル配列を構成する。初期解は**シンプルなプレフィックス骨格化**で作り、\*\*焼きなまし（2-opt／ラベル操作）\*\*で誤差を下げる。

---

## 1. 入出力仕様

### 入力（JSON）

```jsonc
{
  "plans": ["0325", "510"], // 0-5 または 1-6 の文字列
  "results": [
    [0, 2, 1, 3, 0],
    [0, 1, 0]
  ], // 各プランの観測ラベル列（開始部屋を含むので len = |plan|+1）
  "N": 64, // 省略可。省略時は --minN/--maxN で走査
  "startingRoom": 0 // 省略時は 0
}
```

- プランは 0–5 の数字列（または 1–6 を使う系も許容し 0–5 に正規化する）。
- `results` は各プランに対応し「訪ねた順のラベル列」を返す（例より開始ラベルを含み、長さは `|plan|+1`）。

### 出力（JSON）

```jsonc
{
  "rooms": [int, ...], // λ(q) ∈ {0,1,2,3} for q=0..N-1
  "startingRoom": 0,
  "connections": [
    {"from":{"room":q,"door":c}, "to":{"room":q2,"door":e}},
    ...
  ]
}
```

- `connections` はポート間の**無向マッチング**（両方向の対合）。自己ループ可。

### CLI（例）

```
solver --input in.json --output out.json \
       --seed 42 --iters 200000 \
       --minN 24 --maxN 64 --time-limit 5.0
```

- `N` 未指定時は `minN..=maxN` をスイープし、最良解（最小エネルギー）を採用。

---

## 2. データモデル（Rust）

```rust
// 扉は 0..=5 の6方位。部屋は 0..N-1。
type Room = u32;     // or usize
type Door = u8;      // 0..=5
type Label = u8;     // 0..=3

#[derive(Clone, Copy)]
struct PortIdx(pub u32); // p = q*6 + c でフラット化 (0..6N-1)

#[derive(Clone)]
struct Instance {
    plans: Vec<Vec<Door>>,    // 正規化済み (0..=5)
    results: Vec<Vec<Label>>, // 各 plan の観測 (len = |plan|+1)
    n: usize,                 // N
    s0: Room,                 // starting room
}

#[derive(Clone)]
struct Model {
    labels: Vec<Label>,       // λ(q)
    match_to: Vec<PortIdx>,   // μ: 6N 個のポートの対合（μ[μ[p]] = p）
}

#[derive(Default)]
struct Energy {
    obs: i32,                 // 観測不一致カウント
    balance: i32,             // ラベル分布ペナルティ
    total: i32,
}
```

- \*\*対合（involution）\*\*は `match_to[p]` に相手ポート `p'` を入れ、常に両方向対称を保つ。
- ポート ⇄(部屋,扉) 変換：

```rust
fn to_port(q: Room, c: Door) -> PortIdx { PortIdx(q*6 + c as u32) }
fn from_port(p: PortIdx) -> (Room, Door) { (p.0 / 6, (p.0 % 6) as u8) }
```

---

## 3. 入力正規化

- **ドア列の正規化**：

  - すべて `0..=5` ならそのまま。
  - すべて `1..=6` なら `d-1` して `0..=5` に変換。
  - 混在や範囲外はエラー。

- **`results` 一貫性**：各 `results[k].len() == plans[k].len() + 1` を検証。
- `s0` 既定：0（入力にあれば上書き）。

---

## 4. 目的関数（エネルギー）

### 4.1 観測不一致

- `simulate(model, s0, plan)` で生成ラベル列を得て、`results` と**ハミング距離**を合計。

```rust
fn simulate(model: &Model, s0: Room, plan: &[Door]) -> Vec<Label> {
    let mut q = s0;
    let mut out = Vec::with_capacity(plan.len()+1);
    out.push(model.labels[q as usize]);
    for &a in plan {
        let p = to_port(q, a);
        let p2 = model.match_to[p.0 as usize];
        let (q2, _c2) = from_port(p2);
        q = q2;
        out.push(model.labels[q as usize]);
    }
    out
}
```

### 4.2 ラベル分布ペナルティ

- $N=4m+r$（0≤r<4）。各ラベルの目標個数は $\{m\}$ をベースに **r 個だけ m+1**。
- `λ_b` を小さめ（例 0.2）ではじめ、後で上げられるように（MVP は固定でよい）。

```rust
E_balance = λ_b * Σ_ℓ (count(ℓ) - target(ℓ))^2
```

### 4.3 総和

```
E = E_obs + E_balance
```

---

## 5. 初期解（MVP、シンプル）

**方針**：プレフィックスを逐次なぞり、状態は「ラベル quota が残っていれば新規、なければ既存再利用」。ポートは「反対ポート優先 → 空いてなければ最小ポート」。結べないときは**片側保留（dangling）**。末尾で一気に閉じる。

### 5.1 下ごしらえ

- ラベル `labels[q]` を **厳密均等（±1）** に割当（シャッフル）。
- `free[q] = {0..5}` を `Vec<SmallBitSet6>` 的に管理。
- `dangling: Vec<(Room,Door,Option<Room>)>` を用意（宛先未定を許す）。

### 5.2 割当 `alloc(y)`

```rust
// 1) まだ N 未満 かつ その y の枠に余り → 新規作成
// 2) そうでなければ L[q]==y で free ありの最初の q
// 3) それも無ければ free ありの最初の q（ラベル不一致は後で直す）
```

### 5.3 結線 `wire(q_from, a, q_to)`

```rust
// source
if !free[q_from].contains(a) { dangling.push((q_from,a,Some(q_to))); return; }
free[q_from].remove(a);
// dest
let mut c2 = (a + 3) % 6;
if !free[q_to].contains(c2) {
    c2 = free[q_to].first().unwrap_or_default(); // 無ければダングリングにしても良い
}
if free[q_to].contains(c2) {
    connect((q_from,a), (q_to,c2)); // μ を両方向に設定
} else {
    dangling.push((q_from,a,Some(q_to))); // 片側のみ確定
}
```

### 5.4 仕上げ `close_all()`

- `dangling` と各 `free[q]` を使って**貪欲に結線**。
- 余りポートは `(q,c) ↔ (q,c)` の**自己ループ**で埋める。
- これで `match_to` は**完全な対合**になる。

> 参考：API 上も「グラフは無向・逆向きは自動」で、我々の内部表現は「ポート対合」と一致する抽象化。

---

## 6. 焼きなまし（MVP）

### 6.1 近傍

1. **2-opt**：異なる 2 ペア `(a↔b), (c↔d)` を選び、

   - パターン A：`a↔c, b↔d`
   - パターン B：`a↔d, b↔c`
     をランダム選択（自己ループ含め常に対合維持）。

2. **ラベル・スワップ**：部屋 `q1,q2` のラベルを入替。
3. **バランス微調整**（任意）：最多ラベルのノード 1 つを最少ラベルへ置換。

### 6.2 受理・温度

- `ΔE<=0` は受理、`ΔE>0` は `exp(-ΔE/T)` で確率受理。
- 温度 `T_k = T0 * α^k`（例：`T0` は初期 ΔE の p 分位、`α=0.995`）。
- **MVP ではリスタートなし**（後で追加可能）。

### 6.3 差分評価（MVP は素直でも OK）

- まずは全トレース再評価で充分（`K` は小、`|plan|≤18N`）。
- 最適化は後段（影響範囲だけ再評価）。

---

## 7. 全体フロー

```text
load input JSON -> normalize plans/results (0..5) -> choose N
build_initial(instance)  // §5
compute E
anneal(instance, model, params)  // §6
emit JSON (rooms, startingRoom, connections)
```

- `N` 未指定時：`for n in minN..=maxN` で同手順を実行し、最良 `E` の解を出力。
- 出力の `connections` は **各ポートを 1 回ずつ**列挙（`p < match_to[p]` のときのみ出す等）。

---

## 8. 例外・検証

- `results[k].len() != plans[k].len()+1` → 入力エラー。
- `N < 1`、`startingRoom >= N` → 入力エラー。
- 出力前検証：

  - `match_to` が対合（`μ[μ[p]]==p`）、全ポートが被覆（6N 本）。
  - ラベルが 0..3、`rooms.len() == N`。

---

## 9. 計算量の目安

- 初期構築：O(Σ|plan|)（貪欲配線＋最後の埋め合わせは 6N オーダ）。
- 焼きなまし：1 ムーブ評価 O(Σ|plan|)。`iters` が 2e5 程度でも N≤64, K 小なら実用。
- メモリ：O(N)（ラベル）＋ O(6N)（対合）。

---

## 10. 実装詳細（Rust）

### 依存クレート

- `serde`, `serde_json`（IO）
- `clap`（CLI）
- `rand`（乱数）
- `rayon`（N スイープ時の並列、任意）

### 主要関数シグネチャ

```rust
fn parse_input(path: &Path) -> InstanceInputRaw;
fn normalize_input(raw: InstanceInputRaw, min_n: Option<usize>, max_n: Option<usize>) -> Vec<Instance>;

fn build_initial(inst: &Instance, rng: &mut Rng) -> Model; // §5
fn energy(inst: &Instance, m: &Model, lambda_bal: f32) -> Energy;
fn anneal(inst: &Instance, m: &mut Model, params: &AnnealParams, rng: &mut Rng) -> Energy;

fn two_opt(m: &mut Model, p1: PortIdx, p2: PortIdx, rng: &mut Rng) -> (); // 近傍
fn swap_labels(m: &mut Model, q1: Room, q2: Room) -> ();

fn emit_output(model: &Model, s0: Room, path: Option<&Path>);
```

---

## 11. テスト計画

- **ユニット**：

  - 正規化（0..5 と 1..6）
  - 対合操作（2-opt で常に双方向対称になる）
  - `simulate` と例データの長さ一致
  - ラベル分布目標の算出（N=4k+r ケース）

- **プロパティ**：

  - 出力直前に `μ[μ[p]]==p` が常に真。
  - すべてのポートが被覆（`match_to[p]` が未設定なし）。

---

## 12. 将来拡張（後から差し込める）

- **プレフィックスの設計**：de Bruijn(m=2/3) 生成 → 情報量向上。
- **Warm/Kick リスタート**、**受理率監視**。
- **差分評価**（影響した区間のみ再評価）。
- **アンカー再利用**（m-gram → 状態キャッシュ）。
- **ラベル制約のハード化**（最後だけスワップで必達、ペナルティ 0）。

---

## 付記：仕様根拠（抜粋）

- プランは 0–5 の数字列。
- `results` は各訪問時の 2 ビット値列（例が `|plan|+1` を示す）。
- 出力は無向接続（逆方向は自動）という抽象に整合。

---

必要十分な**MVP**です。まずはこれで動かし、ログ（`E_obs` 推移、ダングリング件数、自己ループ率）を見ながら、de Bruijn プレフィックスや差分評価、リスタートを順次追加してください。
