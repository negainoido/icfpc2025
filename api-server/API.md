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

### `POST /select`

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

### `POST /explore`

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

### `POST /guess`

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

## エラー処理

- 409 Conflict: 既にアクティブなセッションが存在する場合（`/select`時）
- 404 Not Found: アクティブなセッションが存在しない場合（`/explore`, `/guess`時）
- 400 Bad Request: セッションIDの不整合や不正なリクエスト
- 502 Bad Gateway: 本家APIとの通信エラー
- 500 Internal Server Error: データベースエラー
