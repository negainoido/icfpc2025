Python 環境は [uv](https://docs.astral.sh/uv/) 使って!!

```bash
curl -LsSf https://astral.sh/uv/install.sh | sh
```

```bash
# Python+依存ライブラリの環境を揃える
uv sync

# 動かす
TEAM_ID=*** uv run ./api.py
uv run ./server.py
```
