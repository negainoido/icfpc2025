Python 環境は [uv](https://docs.astral.sh/uv/) 使って!!

```bash
curl -LsSf https://astral.sh/uv/install.sh | sh
```

```bash
# Python+依存ライブラリの環境が勝手に降ってくる
uv sync

# 自分で追加する
uv add <package-name>  # 例: uv add requests
```

## api.py

API を素直に叩くだけ

```
# 本番サーバを叩く
TEAM_ID=*** uv run ./api.py --help

# ローカルサーバを叩く
TEAM_ID=*** api_HOST=http://localhost:8000 uv run ./api.py --help
```

## server.py

たぶん大体本番をシミュレーションするサーバをローカルに立てる

```
uv run ./server.py
```

`/register` は不要.
`/select` するときに任意の `team_id` を受け付けて自動で登録する.
重複しないようにだけ注意.

