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
TEAM_ID=*** API_HOST=http://localhost:8000 uv run ./api.py --help
```

## server.py

たぶん大体本番をシミュレーションするサーバをローカルに立てる

```
uv run ./server.py
```

`/register` は不要.
`/select` するときに任意の `team_id` を受け付けて自動で登録する.
重複しないようにだけ注意.

problemName は本来の

```
probatio: 3,
primus: 6,
secundus: 12,
tertius: 18,
quartus: 24,
quintus: 30,
```

以外に直接 1 以上の整数を問題名としてもよい.
このときその数を部屋数として問題を作る.
