# API Documentation

ICFPCのサーバーに代理でリクエストを投げるAPI群
本家のAPIについては以下のURLを参照すること
https://icfpcontest2025.github.io/specs/task_from_tex.html

## 環境変数

以下の環境変数が必要です：
- `ICFPC_AUTH_TOKEN`: ICFPC本家APIの認証トークン（`/register`で得られるID）
- `ICFPC_API_BASE_URL`: ICFPC本家APIのベースURL（デフォルト: 'https://31pwr5t6ij.execute-api.eu-west-2.amazonaws.com/'）
- `DATABASE_URL`: MySQLデータベース接続URL

## データベース

- `sessions`: セッション管理用テーブル（ステータス: `active`, `completed`, `failed`, `pending`）
- `api_logs`: APIリクエスト/レスポンスログ用テーブル  
- `pending_requests`: キューに入れられたselectリクエストの情報を保存するテーブル

## セッションキューの仕組み

このAPIでは、同時に複数のセッションを管理するためのキュー機能を提供しています：

1. **通常の動作**: `enqueue: false`の場合、アクティブなセッションがあると409エラーを返します
2. **キュー動作**: `enqueue: true`の場合、アクティブなセッションがあってもリクエストを`pending`状態でキューに保存します
3. **自動実行**: アクティブなセッションが終了（`completed`または`failed`）すると、キューの先頭にある`pending`セッションが自動的に`active`になり、保存されたselectリクエストが実行されます

## API エンドポイント

### `POST /api/select`

本家の`/select`に代理で投げるAPI。

**動作:**
- `enqueue`が`false`（デフォルト）または省略された場合：従来通りの動作。すでに進行中のセッションがある場合はエラー（409 Conflict）を返す。
- `enqueue`が`true`の場合：進行中のセッションがあっても新しいセッションを`pending`状態でキューに追加する。アクティブなセッション終了時に自動的に`active`になり、selectリクエストが実行される。

**リクエスト:**
```json
{
  "problemName": "問題名",
  "user_name": "ユーザー名（省略可能）",
  "enqueue": false
}
```

**レスポンス（activeセッションが作成された場合）:**
```json
{
  "session_id": "生成されたUUID",
  "problemName": "問題名",
  "status": "active"
}
```

**レスポンス（pendingセッションがキューに追加された場合）:**
```json
{
  "session_id": "生成されたUUID",
  "problemName": null,
  "status": "pending"
}
```

### `POST /api/explore`

セッションID付きで本家の`/explore`を叩く。リクエストの内容とその本家からのレスポンスの内容はDBにも格納される。

セッションの指定方法：
- `session_id`を指定：従来通りの動作
- `user_name`を指定：そのユーザーのアクティブセッションを使用
- 両方指定された場合：`session_id`を優先
- どちらも指定されていない場合：エラー

**リクエスト:**
```json
{
  "session_id": "セッションID（省略可能）",
  "user_name": "ユーザー名（省略可能）",
  "plans": ["plan1", "plan2", ...]
}
```

**レスポンス:**
```json
{
  "session_id": "実際に使用されたセッションID",
  "results": [[1, 2], [3, 4], ...],
  "queryCount": 10
}
```

### `POST /api/guess`

セッションID付きで本家の`/guess`を叩く。リクエストの内容とその本家からのレスポンス内容はDBにも格納される。
このAPIを叩くとセッションは終了となる。

セッションの指定方法：
- `session_id`を指定：従来通りの動作
- `user_name`を指定：そのユーザーのアクティブセッションを使用
- 両方指定された場合：`session_id`を優先
- どちらも指定されていない場合：エラー

**リクエスト:**
```json
{
  "session_id": "セッションID（省略可能）",
  "user_name": "ユーザー名（省略可能）",
  "map": {
    "rooms": [1, 2, 3, ...],
    "startingRoom": 1,
    "connections": [
      {"from": {"room": 1, "door": 0}, "to": {"room": 2, "door": 1}},
      ...
    ]
  }
}
```

**レスポンス:**
```json
{
  "session_id": "実際に使用されたセッションID",
  "correct": true
}
```

### `GET /api/sessions`

全セッションの一覧を取得する。最新のものから順に返される。

**レスポンス:**
```json
{
  "sessions": [
    {
      "id": 1,
      "session_id": "uuid-string",
      "user_name": "ユーザー名（null可）",
      "status": "completed",
      "created_at": "2025-09-06T01:00:00Z",
      "completed_at": "2025-09-06T01:30:00Z"
    },
    ...
  ]
}
```

### `GET /api/sessions/current`

現在のアクティブセッション情報を取得する。

**レスポンス（アクティブセッションが存在する場合）:**
```json
{
  "id": 1,
  "session_id": "uuid-string",
  "user_name": "ユーザー名（null可）",
  "status": "active",
  "created_at": "2025-09-06T01:00:00Z",
  "completed_at": null
}
```

**レスポンス（アクティブセッションが存在しない場合）:**
```json
null
```

### `GET /api/sessions/{session_id}`

特定のセッションの詳細情報とAPIログ履歴を取得する。

**レスポンス:**
```json
{
  "session": {
    "id": 1,
    "session_id": "uuid-string",
    "user_name": "ユーザー名（null可）",
    "status": "completed",
    "created_at": "2025-09-06T01:00:00Z",
    "completed_at": "2025-09-06T01:30:00Z"
  },
  "api_logs": [
    {
      "id": 1,
      "session_id": "uuid-string",
      "endpoint": "select",
      "request_body": "{\"problemName\":\"example\"}",
      "response_body": "{\"problemName\":\"example\"}",
      "response_status": 200,
      "created_at": "2025-09-06T01:00:00Z"
    },
    ...
  ]
}
```

### `GET /api/sessions/{session_id}/export`

特定のセッションの全情報とAPIログ履歴をJSON形式でエクスポートする。

**レスポンス:**
```json
{
  "session_info": {
    "session_id": "uuid-string",
    "user_name": "ユーザー名（null可）",
    "status": "completed",
    "created_at": "2025-09-06T01:00:00Z",
    "completed_at": "2025-09-06T01:30:00Z"
  },
  "api_history": [
    {
      "endpoint": "select",
      "timestamp": "2025-09-06T01:00:00Z",
      "request": {
        "problemName": "example"
      },
      "response": {
        "problemName": "example"
      },
      "status": 200
    },
    ...
  ]
}
```

**エラーケース:**
- 404 Not Found: 指定されたセッションIDが見つからない場合

### `PUT /api/sessions/{session_id}/abort`

アクティブなセッションを強制的に中止する。セッションのステータスが`failed`に変更され、`completed_at`が現在時刻に設定される。

**レスポンス（成功時）:**
HTTP 200 OK（JSONボディなし）

**エラーケース:**
- 404 Not Found: 指定されたセッションIDが見つからない場合
- 400 Bad Request: セッションが既に非アクティブ（completed または failed）の場合

## エラー処理

- 409 Conflict: 既にアクティブなセッションが存在する場合（`/select`時）
- 404 Not Found: アクティブなセッションが存在しない場合（`/explore`, `/guess`時）、またはセッションが見つからない場合（`/sessions/{id}`時）
- 400 Bad Request: セッションIDの不整合や不正なリクエスト
- 502 Bad Gateway: 本家APIとの通信エラー
- 500 Internal Server Error: データベースエラー
