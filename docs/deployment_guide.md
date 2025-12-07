# Deployment Guide for Distributed Phoenix Orchestrator

## 1. Overview

This guide provides comprehensive instructions for deploying the distributed Phoenix Orchestrator system. It incorporates all components designed in our scalability improvements:
- Kubernetes cluster setup
- Redis caching layer
- Load balancer configuration
- Resource quotas
- Monitoring stack

## 2. Prerequisites

### 2.1 Infrastructure Requirements
```yaml
Minimum Requirements:
- Kubernetes cluster v1.25+
- 3+ worker nodes (4 CPU, 8GB RAM each)
- 500GB available storage
- Network connectivity between all components
- SSL certificates for secure communication

Tools Required:
- kubectl v1.25+
- helm v3.10+
- redis-cli
- prometheus-operator
```

### 2.2 Environment Variables
```bash
# Create .env file
cat << EOF > .env
# Redis Configuration
REDIS_PASSWORD=$(openssl rand -hex 32)
REDIS_URL=redis://redis-0.redis:6379,redis-1.redis:6379,redis-2.redis:6379

# JWT Authentication
JWT_SECRET=$(openssl rand -hex 32)

# Monitoring
METRICS_ADDR=:9000

# Resource Limits
MAX_MEMORY=4Gi
MAX_CPU=2
EOF
```

## 3. Deployment Steps

### 3.1 Initialize Namespace and Quotas
```bash
# Apply namespace
kubectl apply -f k8s/namespace.yaml

# Apply resource quotas
kubectl apply -f k8s/resource-quotas.yaml

# Apply limit ranges
kubectl apply -f k8s/limit-ranges.yaml
```

### 3.2 Deploy Redis Cluster
```bash
# Add Redis Helm repo
helm repo add bitnami https://charts.bitnami.com/bitnami
helm repo update

# Install Redis cluster
helm install redis bitnami/redis \
  --namespace phoenix-orch \
  --values k8s/redis-values.yaml \
  --set password=$REDIS_PASSWORD
```

### 3.3 Deploy Master Orchestrator
```bash
# Create config maps
kubectl create configmap orchestrator-config \
  --from-file=config.toml \
  --namespace phoenix-orch

# Create secrets
kubectl create secret generic orchestrator-secrets \
  --from-literal=redis-password=$REDIS_PASSWORD \
  --from-literal=jwt-secret=$JWT_SECRET \
  --namespace phoenix-orch

# Deploy master orchestrator
kubectl apply -f k8s/master-orchestrator.yaml
```

### 3.4 Configure Load Balancer
```bash
# Install NGINX Ingress Controller
helm install nginx-ingress ingress-nginx/ingress-nginx \
  --namespace phoenix-orch \
  --values k8s/nginx-values.yaml

# Apply SSL certificates
kubectl create secret tls phoenix-orch-tls \
  --cert=ssl/phoenix-orch.crt \
  --key=ssl/phoenix-orch.key \
  --namespace phoenix-orch

# Apply ingress rules
kubectl apply -f k8s/ingress.yaml
```

### 3.5 Deploy Monitoring Stack
```bash
# Install Prometheus Operator
helm install prometheus prometheus-community/kube-prometheus-stack \
  --namespace phoenix-orch \
  --values k8s/prometheus-values.yaml

# Install Loki
helm install loki grafana/loki-stack \
  --namespace phoenix-orch \
  --values k8s/loki-values.yaml

# Install Tempo
helm install tempo grafana/tempo \
  --namespace phoenix-orch \
  --values k8s/tempo-values.yaml
```

## 4. Verification Steps

### 4.1 Check Component Status
```bash
# Verify all pods are running
kubectl get pods -n phoenix-orch

# Check Redis cluster health
kubectl exec -it redis-0 -n phoenix-orch -- redis-cli cluster info

# Verify load balancer
kubectl get svc -n phoenix-orch nginx-ingress-controller

# Check monitoring stack
kubectl get pods -n phoenix-orch -l app=prometheus
kubectl get pods -n phoenix-orch -l app=loki
kubectl get pods -n phoenix-orch -l app=tempo
```

### 4.2 Health Checks
```bash
# Master orchestrator health
curl -k https://phoenix-orch.example.com/health

# Redis health
kubectl exec -it redis-0 -n phoenix-orch -- redis-cli ping

# Prometheus targets
curl -k https://phoenix-orch.example.com/prometheus/targets
```

## 5. Post-Deployment Configuration

### 5.1 Configure Monitoring
```bash
# Import Grafana dashboards
kubectl port-forward svc/grafana 3000:80 -n phoenix-orch
# Access Grafana at http://localhost:3000 and import dashboards

# Configure alert rules
kubectl apply -f k8s/prometheus-rules.yaml
```

### 5.2 Set Up Logging
```bash
# Configure log aggregation
kubectl apply -f k8s/promtail-config.yaml

# Verify log collection
kubectl logs -l app=promtail -n phoenix-orch
```

## 6. Scaling Guidelines

### 6.1 Horizontal Scaling
```bash
# Scale master orchestrator
kubectl scale statefulset master-orchestrator --replicas=5 -n phoenix-orch

# Scale Redis cluster
helm upgrade redis bitnami/redis \
  --set cluster.nodes=5 \
  --reuse-values \
  --namespace phoenix-orch
```

### 6.2 Vertical Scaling
```bash
# Update resource limits
kubectl patch statefulset master-orchestrator \
  -p '{"spec":{"template":{"spec":{"containers":[{"name":"master-orchestrator","resources":{"limits":{"memory":"8Gi","cpu":"4"}}}]}}}}' \
  -n phoenix-orch
```

## 7. Backup and Recovery

### 7.1 Redis Backup
```bash
# Create Redis backup
kubectl exec -it redis-0 -n phoenix-orch -- redis-cli save

# Copy backup file
kubectl cp redis-0:/data/dump.rdb backup/redis-dump.rdb -n phoenix-orch
```

### 7.2 Configuration Backup
```bash
# Backup all configmaps
kubectl get configmap -n phoenix-orch -o yaml > backup/configmaps.yaml

# Backup all secrets (encrypted)
kubectl get secret -n phoenix-orch -o yaml > backup/secrets.yaml
```

## 8. Troubleshooting Guide

### 8.1 Common Issues

1. **Pod Startup Failures**
```bash
# Check pod status
kubectl describe pod <pod-name> -n phoenix-orch

# Check logs
kubectl logs <pod-name> -n phoenix-orch
```

2. **Redis Connection Issues**
```bash
# Check Redis cluster status
kubectl exec -it redis-0 -n phoenix-orch -- redis-cli cluster nodes

# Test connectivity
kubectl exec -it master-orchestrator-0 -n phoenix-orch -- nc -zv redis-0.redis 6379
```

3. **Load Balancer Issues**
```bash
# Check ingress status
kubectl describe ingress -n phoenix-orch

# Check NGINX logs
kubectl logs -l app=nginx-ingress -n phoenix-orch
```

### 8.2 Recovery Procedures

1. **Redis Recovery**
```bash
# Stop affected node
kubectl scale statefulset redis --replicas=2 -n phoenix-orch

# Restore from backup
kubectl cp backup/redis-dump.rdb redis-0:/data/dump.rdb -n phoenix-orch

# Restart node
kubectl scale statefulset redis --replicas=3 -n phoenix-orch
```

2. **Master Orchestrator Recovery**
```bash
# Identify failing pods
kubectl get pods -l app=master-orchestrator -n phoenix-orch

# Check logs
kubectl logs <pod-name> -n phoenix-orch

# Force pod recreation
kubectl delete pod <pod-name> -n phoenix-orch
```

## 9. Maintenance Procedures

### 9.1 Rolling Updates
```bash
# Update master orchestrator
kubectl set image statefulset/master-orchestrator \
  master-orchestrator=phoenix-orch/master-orchestrator:new-version \
  -n phoenix-orch

# Monitor rollout
kubectl rollout status statefulset/master-orchestrator -n phoenix-orch
```

### 9.2 Configuration Updates
```bash
# Update configmap
kubectl create configmap orchestrator-config \
  --from-file=config.toml \
  --namespace phoenix-orch \
  --dry-run=client -o yaml | kubectl apply -f -

# Restart pods to pick up new config
kubectl rollout restart statefulset/master-orchestrator -n phoenix-orch
```

## 10. Security Considerations

### 10.1 Certificate Rotation
```bash
# Generate new certificates
openssl req -x509 -nodes -days 365 -newkey rsa:2048 \
  -keyout ssl/phoenix-orch-new.key \
  -out ssl/phoenix-orch-new.crt

# Update TLS secret
kubectl create secret tls phoenix-orch-tls \
  --cert=ssl/phoenix-orch-new.crt \
  --key=ssl/phoenix-orch-new.key \
  --namespace phoenix-orch \
  --dry-run=client -o yaml | kubectl apply -f -
```

### 10.2 Secret Rotation
```bash
# Generate new secrets
NEW_REDIS_PASSWORD=$(openssl rand -hex 32)
NEW_JWT_SECRET=$(openssl rand -hex 32)

# Update secrets
kubectl create secret generic orchestrator-secrets \
  --from-literal=redis-password=$NEW_REDIS_PASSWORD \
  --from-literal=jwt-secret=$NEW_JWT_SECRET \
  --namespace phoenix-orch \
  --dry-run=client -o yaml | kubectl apply -f -
```

## 11. Monitoring and Alerts

### 11.1 Access Monitoring
```bash
# Grafana
kubectl port-forward svc/grafana 3000:80 -n phoenix-orch

# Prometheus
kubectl port-forward svc/prometheus-operated 9090:9090 -n phoenix-orch

# Loki
kubectl port-forward svc/loki 3100:3100 -n phoenix-orch
```

### 11.2 Alert Configuration
```bash
# Update alert rules
kubectl apply -f k8s/prometheus-rules.yaml

# Verify alert manager config
kubectl get secret alertmanager-prometheus-kube-prometheus-alertmanager \
  -n phoenix-orch -o yaml
```

## 12. Cleanup Procedures

### 12.1 Temporary Resources
```bash
# Clean up completed jobs
kubectl delete jobs --field-selector status.successful=1 -n phoenix-orch

# Clean up failed pods
kubectl delete pods --field-selector status.phase=Failed -n phoenix-orch
```

### 12.2 Full Uninstall
```bash
# Remove applications
helm uninstall redis -n phoenix-orch
helm uninstall prometheus -n phoenix-orch
helm uninstall loki -n phoenix-orch
helm uninstall tempo -n phoenix-orch
helm uninstall nginx-ingress -n phoenix-orch

# Remove namespace
kubectl delete namespace phoenix-orch