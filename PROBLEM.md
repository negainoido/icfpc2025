# ICFPC 2025 - Ædificium Library Exploration Problem

## 問題概要

AdsoとWilliamは、迷宮のような図書館（Ædificium）の地図を作成するため、反復的な探索を通じて情報を収集する必要があります。

### 図書館の構造
- **六角形の部屋**: 各部屋は六角形で、各辺にドア（1-6番）がある
- **部屋のラベル**: 各部屋にはラベルがあるが、Williamは視力の問題で最初の2ビットしか読み取れない
- **接続**: 各ドアは他の部屋への通路となっており、同じ部屋や同じドアに戻ることもある

### タスク
1. **ルートプラン作成**: 0-5の数字の列で探索経路を指定
2. **情報収集**: 各部屋で2ビット整数値を記録
3. **地図作成**: 収集した情報から無向グラフとして図書館の正確な地図を構築
4. **効率化**: できるだけ少ない探索回数で地図を完成させる

### スコアリング
- 正しい地図を生成するために必要な探索回数（queryCount）で評価
- 各問題でチーム間の相対的な順位によってポイントが付与される（Borda count方式）

---

## API仕様

### ベースURL
```
https://31pwr5t6ij.execute-api.eu-west-2.amazonaws.com/
```

### エンドポイント

#### 1. チーム登録 - `POST /register`

新しいチームを登録します。

**リクエスト:**
```json
{
  "name": "チーム名",
  "pl": "使用するプログラミング言語",
  "email": "連絡先メールアドレス"
}
```

**レスポンス:**
```json
{
  "id": "ランダムに生成された秘密のチームID"
}
```

**注意事項:**
- `id`は秘密情報として保管し、公開しないこと
- すべての後続のAPIリクエストでこのIDが必要

**実際のレスポンス例:**
```json
{
  "id": "a7f8d9c2-b4e1-4c8a-9f2d-1e3b5a7c9d4f"
}
```

---

#### 2. 問題選択 - `POST /select`

解く問題を選択します。

**リクエスト:**
```json
{
  "id": "チームID",
  "problemName": "問題名"
}
```

**レスポンス:**
```json
{
  "problemName": "選択された問題名"
}
```

**利用可能な問題:**
- `probatio`: テスト用の3部屋の簡単な迷宮
- その他の問題はリーダーボードページで確認可能

**注意事項:**
- 問題を選択すると、その問題用の迷宮がランダムに生成される
- 既に問題が選択されている場合、新しい問題を選択すると古い問題は破棄される

**実際のレスポンス例:**
```json
{
  "problemName": "probatio"
}
```

---

#### 3. 探索実行 - `POST /explore`

図書館を探索し、部屋の情報を収集します。

**リクエスト:**
```json
{
  "id": "チームID",
  "plans": [
    "0325",
    "142",
    "555000"
  ]
}
```

**リクエストパラメータ詳細:**
- `id`: チームID（文字列）
- `plans`: ルートプランの配列
  - 各ルートプランは0-5の数字の文字列
  - 例: "0325" = ドア0→ドア3→ドア2→ドア5の順に移動

**レスポンス:**
```json
{
  "results": [
    [2, 1, 0, 3],
    [2, 3, 1],
    [2, 0, 0, 1, 1, 1]
  ],
  "queryCount": 3
}
```

**レスポンスパラメータ詳細:**
- `results`: 各ルートプランに対する観測結果の配列
  - 各要素は訪問した部屋で観測された2ビット整数値（0-3）のリスト
- `queryCount`: これまでの総探索回数

**最適化のヒント:**
- 複数のルートプランを1回のリクエストでバッチ送信可能
- HTTPリクエストごとに1ポイントのペナルティが追加されるため、バッチ処理推奨

**実際のレスポンス例（probatioでの6つのルートプラン探索）:**
説明: "0", "1", "2", "3", "00", "11" の6つのプランを送信した結果
```json
{
  "results": [
    [0, 0],
    [0, 1],
    [0, 2],
    [0, 1],
    [0, 0, 0],
    [0, 1, 0]
  ],
  "queryCount": 7
}
```

---

#### 4. 地図提出 - `POST /guess`

構築した地図を提出して答え合わせをします。

**リクエスト:**
```json
{
  "id": "チームID",
  "map": {
    "rooms": [0, 1, 2, 1, 3],
    "startingRoom": 0,
    "connections": [
      {
        "from": { "room": 0, "door": 0 },
        "to": { "room": 1, "door": 3 }
      },
      {
        "from": { "room": 0, "door": 1 },
        "to": { "room": 2, "door": 2 }
      },
      {
        "from": { "room": 1, "door": 0 },
        "to": { "room": 3, "door": 5 }
      }
    ]
  }
}
```

**リクエストパラメータ詳細:**
- `id`: チームID
- `map`: 地図の構造
  - `rooms`: 各部屋の2ビットラベル値の配列（部屋のインデックスは0から）
  - `startingRoom`: 開始部屋のインデックス
  - `connections`: 部屋間の接続情報
    - `from`: 接続元（room: 部屋インデックス, door: ドア番号0-5）
    - `to`: 接続先（room: 部屋インデックス, door: ドア番号0-5）

**レスポンス:**
```json
{
  "correct": true
}
```

**レスポンスパラメータ詳細:**
- `correct`: 提出した地図が正しい場合`true`、間違っている場合`false`

**注意事項:**
- グラフは無向グラフとして構築される
- 一度接続を定義すれば、逆方向の接続は自動的に作成される
- 提出後は問題がリセットされる（不正解の場合は最初からやり直し）
- 正解の場合、queryCountが以前の記録より良ければスコアが更新される

**実際のレスポンス例（不正解）:**
```json
{
  "correct": false
}
```

**実際のレスポンス例（エラー）:**
```json
{
  "error": "Error: Door IV of room 0 is not connected to anything"
}
```

**エラーメッセージについて:**
- 提出した地図に構造的な問題がある場合、エラーメッセージが返される
- すべてのドアが適切に接続されていることを確認する必要がある

---

## テスト用の環境変数設定

APIをテストする際は、以下のように環境変数`TEAM_ID`を設定してください：

```bash
export TEAM_ID="your-team-id-here"
```

### テストコマンド例

```bash
# チーム登録
curl -X POST https://31pwr5t6ij.execute-api.eu-west-2.amazonaws.com/register \
  -H "Content-Type: application/json" \
  -d '{"name":"TestTeam","pl":"Python","email":"test@example.com"}'

# 問題選択（TEAM_ID環境変数を使用）
curl -X POST https://31pwr5t6ij.execute-api.eu-west-2.amazonaws.com/select \
  -H "Content-Type: application/json" \
  -d "{\"id\":\"$TEAM_ID\",\"problemName\":\"probatio\"}"

# 探索実行
curl -X POST https://31pwr5t6ij.execute-api.eu-west-2.amazonaws.com/explore \
  -H "Content-Type: application/json" \
  -d "{\"id\":\"$TEAM_ID\",\"plans\":[\"0\",\"1\",\"2\"]}"

# 地図提出
curl -X POST https://31pwr5t6ij.execute-api.eu-west-2.amazonaws.com/guess \
  -H "Content-Type: application/json" \
  -d "{\"id\":\"$TEAM_ID\",\"map\":{\"rooms\":[0,1,2],\"startingRoom\":0,\"connections\":[{\"from\":{\"room\":0,\"door\":0},\"to\":{\"room\":1,\"door\":3}}]}}"
```

---

## エラーハンドリング

各APIエンドポイントは以下のHTTPステータスコードを返す可能性があります：

- `200 OK`: 成功
- `400 Bad Request`: 無効なチームID、不正なルートプラン形式（0-5以外の数字）など
- `404 Not Found`: リソースが見つからない
- `500 Internal Server Error`: サーバー内部エラー

### 注意事項

- **レート制限**: APIのレート制限については公式ドキュメントを参照
- **タイムアウト**: 大量のルートプランを送信する場合、処理時間に注意
- **問題の状態管理**: 問題選択後、guessまたは新しいselectを行うまで状態が維持される
- **並行アクセス**: 同一チームIDでの並行アクセスは避ける

エラー時は適切なリトライロジックを実装することを推奨します。