#!/bin/bash

# GCP VM deployment script for ICFPC 2025
set -euxo pipefail

cd "$(dirname "$0")/.."

# Configuration
GCP_PROJECT="negainoido"
GCP_ZONE="us-west1-b"
GCP_INSTANCE="instance-20250824-043241"
DEPLOY_USER="$USER"

echo "🚀 Starting deployment to GCP VM..."

# Build API Server
echo "📦 Building API Server..."
cd api-server
cargo build --release
cd ..

# Build WebApp
echo "📦 Building WebApp..."
cd webapp
npm install
npm run build
cd ..

# Transfer files to GCP VM
echo "📤 Transferring files to GCP VM..."

# Create directories on VM
gcloud compute ssh --zone "$GCP_ZONE" "$GCP_INSTANCE" --project "$GCP_PROJECT" --command "
    sudo mkdir -p /opt/icfpc2025/{api-server,webapp}
    sudo chown -R $DEPLOY_USER:$DEPLOY_USER /opt/icfpc2025
"

# Transfer API Server binary
gcloud compute scp --zone "$GCP_ZONE" --project "$GCP_PROJECT" \
    api-server/target/release/icfpc2025-api-server $GCP_INSTANCE:/opt/icfpc2025/api-server/

# Transfer API Server resources
gcloud compute scp --zone "$GCP_ZONE" --project "$GCP_PROJECT" --recurse \
    api-server/resources/ $GCP_INSTANCE:/opt/icfpc2025/api-server/

# Transfer WebApp build
gcloud compute scp --zone "$GCP_ZONE" --project "$GCP_PROJECT" --recurse \
    webapp/dist/ $GCP_INSTANCE:/opt/icfpc2025/webapp/

# Transfer configuration files
gcloud compute scp --zone "$GCP_ZONE" --project "$GCP_PROJECT" \
    ./deploy/nginx.conf $GCP_INSTANCE:~/nginx.conf

gcloud compute scp --zone "$GCP_ZONE" --project "$GCP_PROJECT" \
    icfpc2025-api.service $GCP_INSTANCE:~/icfpc2025-api.service

gcloud compute scp --zone "$GCP_ZONE" --project "$GCP_PROJECT" \
    ./deploy/.env.production $GCP_INSTANCE:/opt/icfpc2025/api-server/.env

# Setup services on VM
echo "🔧 Setting up services on VM..."
gcloud compute ssh --zone "$GCP_ZONE" "$GCP_INSTANCE" --project "$GCP_PROJECT" --command "
    # Make binary executable
    chmod +x /opt/icfpc2025/api-server/icfpc2025-api-server
    
    # Setup nginx
    sudo cp ~/nginx.conf /etc/nginx/sites-available/icfpc2025
    sudo ln -sf /etc/nginx/sites-available/icfpc2025 /etc/nginx/sites-enabled/
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
"

echo "🎉 Deployment complete!"
echo "🌐 Your application should be available at http://$(gcloud compute instances describe $GCP_INSTANCE --zone=$GCP_ZONE --project=$GCP_PROJECT --format='get(networkInterfaces[0].accessConfigs[0].natIP)')"
