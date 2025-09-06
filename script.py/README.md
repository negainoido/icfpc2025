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

negainoido.garasubo.com / 本番サーバ / (下記の) ローカルサーバを対象に API を叩く.
環境変数は `.env` に書いておいてもOK.

```bash
# garasubo.com (CLIENT_ID, CLIENT_SECRET は Google Docs 参照)
CLIENT_ID=*** CLIENT_SECRET=*** uv run ./api.py --help

# (!!) 本番サーバを直接叩く (TEAM_ID は Gooegle Docs 参照)
TEAM_ID=*** API_HOST=https://31pwr5t6ij.execute-api.eu-west-2.amazonaws.com/ uv run ./api.py --help

# ローカルサーバを叩く (TEAM_ID はダミー文字列でOK)
TEAM_ID=$(whoami) API_HOST=http://localhost:8000 uv run ./api.py --help
```

## server.py

本番をシミュレーションするサーバをローカルに立てる.

```bash
# デフォルト, localhost:8000 で立つ
uv run ./server.py

# ポート指定 & オートリロード
fastapi run server.py --host 0.0.0.0 --port 8000 --reload
```

`/register` は不要.
`/select` するときに任意の `team_id` を受け付けて自動で登録する.
並列に動かすなら重複しないようにだけ注意.

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
