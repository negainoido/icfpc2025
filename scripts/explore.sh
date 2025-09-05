#!/bin/bash

set -euxo pipefail

# 全ての引数を配列として受け取る
PLANS=("$@")

# JSON配列を構築
JSON_PLANS=""
for plan in "${PLANS[@]}"; do
    if [ -z "$JSON_PLANS" ]; then
        JSON_PLANS="\"$plan\""
    else
        JSON_PLANS="$JSON_PLANS, \"$plan\""
    fi
done

curl -X POST "https://31pwr5t6ij.execute-api.eu-west-2.amazonaws.com/explore" -H 'Content-Type: application/json' -d "{\"id\":\"${TEAM_ID}\", \"plans\":[$JSON_PLANS]}"

