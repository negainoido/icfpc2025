#!/bin/bash

set -euxo pipefail

NAME=$1

curl -X POST "https://31pwr5t6ij.execute-api.eu-west-2.amazonaws.com/select" -H 'Content-Type: application/json' -d "{\"id\":\"${TEAM_ID}\", \"problemName\":\"${NAME}\"}"

