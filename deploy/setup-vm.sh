#!/bin/bash
# VM setup script for GitHub Actions deployment

set -euo pipefail

# Create webapp system user if it doesn't exist
if ! id webapp &>/dev/null; then
    sudo useradd --system --no-create-home --shell /usr/sbin/nologin webapp
fi

# Create directories
mkdir -p ~/{api-server,webapp}
sudo mkdir -p /opt/icfpc2025/{api-server,webapp}
sudo chown -R webapp:webapp /opt/icfpc2025

echo "✅ VM setup completed"