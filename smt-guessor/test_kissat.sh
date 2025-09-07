#!/bin/bash

cd $(dirname $0)

SIZE=3

tdir=$(mktemp -d)

plan=$(head /dev/urandom | tr -dc '0-5' | head -c $((SIZE * 18)) )
(
    cd ../script.py
    API_HOST=http://localhost:8000 uv run api.py select $SIZE
    API_HOST=http://localhost:8000 uv run api.py explore "$plan" | tail -n 1 | jq ".[\"N\"]=$SIZE" > $tdir/plan.json
)

uv run kissat.py --input $tdir/plan.json --output $tdir/map.json

(
    cd ../script.py
    API_HOST=http://localhost:8000 uv run api.py guess $tdir/map.json
)
