# API Documentation

## Solutions API

### GET /api/solutions

solutionsテーブルの各問題ごとの上位20件のレコードを取得します。

#### レスポンス

**成功時 (200 OK):**
```json
{
  "success": true,
  "data": [
    {
      "id": 1,
      "problem_id": 25,
      "problem_type": "spaceship",
      "status": "solved",
      "solver": "algorithm_v1",
      "score": 150,
      "ts": "2025-08-09T12:34:56Z"
    }
  ],
  "message": "Solutions retrieved successfully"
}
```

**サーバーエラー (500 Internal Server Error):**
```
HTTP 500 Internal Server Error
```

### GET /api/solutions/{id}

指定されたIDのsolutionレコードを取得します。

#### パラメータ

- `id` (path parameter, required): solution ID

#### レスポンス

**成功時 (200 OK):**
```json
{
  "success": true,
  "data": {
    "id": 1,
    "problem_id": 25,
    "problem_type": "spaceship",
    "status": "solved",
    "solver": "algorithm_v1",
    "score": 150,
    "ts": "2025-08-09T12:34:56Z"
  },
  "message": "Solution retrieved successfully"
}
```

**レコードが存在しない場合 (404 Not Found):**
```
HTTP 404 Not Found
```

**サーバーエラー (500 Internal Server Error):**
```
HTTP 500 Internal Server Error
```

### POST /api/solutions

新しいsolutionレコードを作成します。

#### リクエストボディ

```json
{
  "problem_id": 25,
  "problem_type": "spaceship",
  "status": "solved",
  "solver": "algorithm_v1",
  "score": 150
}
```

#### パラメータ

- `problem_id` (integer, required): 問題ID
- `problem_type` (string, optional): 問題タイプ
- `status` (string, optional): ステータス
- `solver` (string, required): ソルバー名
- `score` (integer, optional): スコア

#### レスポンス

**成功時 (200 OK):**
```json
{
  "success": true,
  "data": {
    "id": 1,
    "problem_id": 25,
    "problem_type": "spaceship",
    "status": "solved",
    "solver": "algorithm_v1",
    "score": 150,
    "ts": "2025-08-09T12:34:56Z"
  },
  "message": "Solution created successfully"
}
```

**サーバーエラー (500 Internal Server Error):**
```
HTTP 500 Internal Server Error
```

## Spaceship Resources API

### GET /api/spaceship/{filename}

Spaceshipリソースディレクトリ内のtxtファイルの内容を取得します。

#### パラメータ

- `filename` (path parameter, required): ファイル名（拡張子なし）
  - 例: `problem1`, `problem25`
  - 英数字とハイフンのみ許可（セキュリティ対策）

#### レスポンス

**成功時 (200 OK):**
```json
{
  "success": true,
  "data": {
    "filename": "problem1",
    "content": "1 -1\n1 -3\n2 -5\n2 -8\n3 -10\n\n"
  },
  "message": "File retrieved successfully"
}
```

**ファイルが存在しない場合 (404 Not Found):**
```
HTTP 404 Not Found
```

**不正なファイル名の場合 (400 Bad Request):**
```
HTTP 400 Bad Request
```

**サーバーエラー (500 Internal Server Error):**
```
HTTP 500 Internal Server Error
```
