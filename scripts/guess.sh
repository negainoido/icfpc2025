#!/bin/bash

set -euxo pipefail

# このsolutionは手でつくったもの
SOLUTION=$(cat << 'EOF'
{
  "rooms": [0,1,2],
  "startingRoom": 0,
  "connections": [
    {
      "from": { "room": 0, "door": 0 },
      "to": { "room": 1, "door": 1 }
    },
    {
      "from": { "room": 0, "door": 1 },
      "to": { "room": 0, "door": 1 }
    },
    {
      "from": { "room": 0, "door": 2 },
      "to": { "room": 2, "door": 3 }
    },
    {
      "from": { "room": 0, "door": 3 },
      "to": { "room": 0, "door": 3 }
    },
    {
      "from": { "room": 0, "door": 4 },
      "to": { "room": 2, "door": 5 }
    },
    {
      "from": { "room": 0, "door": 5 },
      "to": { "room": 1, "door": 5 }
    },
    {
      "from": { "room": 1, "door": 0 },
      "to": { "room": 2, "door": 1 }
    },
    {
      "from": { "room": 1, "door": 2 },
      "to": { "room": 2, "door": 4 }
    },
    {
      "from": { "room": 1, "door": 3 },
      "to": { "room": 1, "door": 3 }
    },
    {
      "from": { "room": 1, "door": 4 },
      "to": { "room": 1, "door": 4 }
    },
    {
      "from": { "room": 2, "door": 0 },
      "to": { "room": 2, "door": 0 }
    },
    {
      "from": { "room": 2, "door": 2 },
      "to": { "room": 2, "door": 2 }
    }
  ]
}
EOF
)

curl -X POST "https://31pwr5t6ij.execute-api.eu-west-2.amazonaws.com/guess" -H 'Content-Type: application/json' -d "{\"id\":\"${TEAM_ID}\", \"map\":${SOLUTION}}"

