#!/bin/bash
# Service deployment script for GitHub Actions

set -euo pipefail

echo "🔧 Setting up services on VM..."

# Move resources
sudo rsync -avr ~/api-server /opt/icfpc2025/
sudo rsync -avr ~/webapp /opt/icfpc2025/

# Ensure proper ownership of all transferred files
sudo mkdir -p /opt/icfpc2025
sudo chown -R webapp:webapp /opt/icfpc2025

# Make binary executable
sudo chmod +x /opt/icfpc2025/api-server/icfpc2025-api-server

# Setup nginx
sudo cp ~/nginx.conf /etc/nginx/sites-available/icfpc2025.conf
sudo ln -sf /etc/nginx/sites-available/icfpc2025.conf /etc/nginx/sites-enabled/
sudo rm -f /etc/nginx/sites-enabled/default
sudo nginx -t && sudo systemctl reload nginx

# Setup systemd service
sudo cp ~/icfpc2025-api.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable icfpc2025-api
sudo systemctl restart icfpc2025-api

# Check status
echo '✅ Service status:'
sudo systemctl status icfpc2025-api --no-pager
echo '✅ Nginx status:'
sudo systemctl status nginx --no-pager

echo "🎉 Service deployment complete!"