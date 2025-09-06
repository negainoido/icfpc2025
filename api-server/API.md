# API Documentation

ICFPCのサーバーに代理でリクエストを投げるAPI群
本家のAPIについては以下のURLを参照すること
https://icfpcontest2025.github.io/specs/task_from_tex.html

## 環境変数

以下の環境変数が必要です：
- `ICFPC_AUTH_TOKEN`: ICFPC本家APIの認証トークン（`/register`で得られるID）
- `ICFPC_API_BASE_URL`: ICFPC本家APIのベースURL（デフォルト: https://icfpcontest2025.github.io/api）
- `DATABASE_URL`: MySQLデータベース接続URL

## データベース

- `sessions`: セッション管理用テーブル
- `api_logs`: APIリクエスト/レスポンスログ用テーブル

## API エンドポイント

### `POST /api/select`

本家の`/select`に代理で投げるAPI。すでに進行中のセッションがある場合はエラー（409 Conflict）を返す。
そうでない場合は新たなセッション番号を発行した上で`/select`を投げ、セッションを進行中にする。

**レスポンス:**
```json
{
  "success": true,
  "data": {
    "session_id": "生成されたUUID",
    ...  // 本家APIからのレスポンス内容
  },
  "message": "Session created and select request completed"
}
```

### `POST /api/explore`

セッションID付きで本家の`/explore`を叩く。リクエストの内容とその本家からのレスポンスの内容はDBにも格納される。

**リクエスト:**
```json
{
  "session_id": "セッションID",
  ...  // 本家APIへ送信するデータ
}
```

**レスポンス:**
```json
{
  "success": true,
  "data": {
    ...  // 本家APIからのレスポンス内容
  },
  "message": "Explore request completed"
}
```

### `POST /api/guess`

セッションID付きで本家の`/guess`を叩く。リクエストの内容とその本家からのレスポンス内容はDBにも格納される。
このAPIを叩くとセッションは終了となる。

**リクエスト:**
```json
{
  "session_id": "セッションID",
  ...  // 本家APIへ送信するデータ
}
```

**レスポンス:**
```json
{
  "success": true,
  "data": {
    ...  // 本家APIからのレスポンス内容
  },
  "message": "Guess request completed and session terminated"
}
```

### `GET /api/sessions`

全セッションの一覧を取得する。最新のものから順に返される。

**レスポンス:**
```json
{
  "success": true,
  "data": {
    "sessions": [
      {
        "id": 1,
        "session_id": "uuid-string",
        "status": "completed",
        "created_at": "2025-09-06T01:00:00Z",
        "completed_at": "2025-09-06T01:30:00Z"
      },
      ...
    ]
  },
  "message": "Sessions retrieved successfully"
}
```

### `GET /api/sessions/current`

現在のアクティブセッション情報を取得する。

**レスポンス:**
```json
{
  "success": true,
  "data": {
    "id": 1,
    "session_id": "uuid-string",
    "status": "active",
    "created_at": "2025-09-06T01:00:00Z",
    "completed_at": null
  },
  "message": "Current session retrieved successfully"
}
```

アクティブセッションが存在しない場合:
```json
{
  "success": true,
  "data": null,
  "message": "No active session"
}
```

### `GET /api/sessions/{session_id}`

特定のセッションの詳細情報とAPIログ履歴を取得する。

**レスポンス:**
```json
{
  "success": true,
  "data": {
    "session": {
      "id": 1,
      "session_id": "uuid-string",
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
  },
  "message": "Session detail retrieved successfully"
}
```

### `PUT /api/sessions/{session_id}/abort`

アクティブなセッションを強制的に中止する。セッションのステータスが`failed`に変更され、`completed_at`が現在時刻に設定される。

**レスポンス（成功時）:**
```json
{
  "success": true,
  "data": null,
  "message": "Session aborted successfully"
}
```

**エラーケース:**
- 404 Not Found: 指定されたセッションIDが見つからない場合
- 400 Bad Request: セッションが既に非アクティブ（completed または failed）の場合

## エラー処理

- 409 Conflict: 既にアクティブなセッションが存在する場合（`/select`時）
- 404 Not Found: アクティブなセッションが存在しない場合（`/explore`, `/guess`時）、またはセッションが見つからない場合（`/sessions/{id}`時）
- 400 Bad Request: セッションIDの不整合や不正なリクエスト
- 502 Bad Gateway: 本家APIとの通信エラー
- 500 Internal Server Error: データベースエラー
