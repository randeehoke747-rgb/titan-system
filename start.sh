#!/usr/bin/env bash
set -euo pipefail

IMAGE="titan-system:local"

echo "==> Building Titan"
docker build -t "$IMAGE" .

echo "==> Checking Kubernetes"
kubectl version --client

echo "==> Applying Kubernetes manifests"
kubectl apply -k k8s/

echo "==> Updating deployment image"
kubectl -n titan set image \
    deployment/titan-control-plane \
    titan-control-plane="$IMAGE"

echo "==> Waiting for rollout"
kubectl -n titan rollout status \
    deployment/titan-control-plane \
    --timeout=120s

echo
echo "Titan deployment is ready."

echo
kubectl -n titan get pods
kubectl -n titan get svc
