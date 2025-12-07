# Phoenix Orchestrator Operations Guide

## Table of Contents
1. [Overview](#overview)
2. [Canary Deployments](#canary-deployments)
3. [Monitoring and Observability](#monitoring-and-observability)
4. [Alerting System](#alerting-system)
5. [Disaster Recovery](#disaster-recovery)
6. [Rollback Procedures](#rollback-procedures)

## Overview

This guide documents the operational improvements implemented to meet SpaceX production standards, including canary deployments, enhanced monitoring, automated rollbacks, and disaster recovery procedures.

## Canary Deployments

### Architecture
The canary deployment system uses a progressive rollout strategy with automated validation and rollback capabilities. The system consists of:

- Traffic control layer (NGINX Ingress)
- Health validation service
- Automated rollback controller
- Metrics-based validation

### Deployment Phases
1. **Initial Canary (5% traffic)**
   ```yaml
   # Configure via k8s/canary/traffic-control.yaml
   nginx.ingress.kubernetes.io/canary-weight: "5"
   ```
   - Duration: 10 minutes
   - Error threshold: 0.1%
   - Latency threshold: 100ms

2. **Expanded Canary (20% traffic)**
   - Duration: 15 minutes
   - Error threshold: 0.5%
   - Latency threshold: 200ms

3. **Full Rollout**
   - Progressive increase: 20% increments
   - Interval: 5 minutes
   - Continuous validation

### Health Validation
The health validator service (`core/health-validator`) monitors:
- Error rates
- Latency (p95, p99)
- Resource utilization
- Business metrics

### Usage
```bash
# Deploy canary version
kubectl apply -f k8s/canary/deployment.yaml

# Monitor progress
kubectl logs -f -l app=health-validator -n phoenix-orch

# View traffic distribution
kubectl get ingress -n phoenix-orch
```

## Monitoring and Observability

### Components
1. **Metrics Collection**
   - Prometheus for time-series data
   - Custom metrics via `/metrics` endpoints
   - Node exporter for system metrics

2. **Log Aggregation**
   - Loki for log storage and querying
   - Structured logging format
   - Log shipping via Promtail

3. **Distributed Tracing**
   - Tempo for trace storage
   - OpenTelemetry instrumentation
   - Trace context propagation

### Key Metrics
```yaml
# Application Metrics
- requests_total
- request_duration_seconds
- error_rate
- task_success_rate

# Resource Metrics
- cpu_usage_percent
- memory_usage_bytes
- disk_io_operations
- network_traffic
```

### Dashboards
Access Grafana at `http://<cluster-ip>/grafana`:
- System Overview
- Application Performance
- Canary Comparison
- Resource Utilization

## Alerting System

### Alert Levels
1. **Critical**
   - Response time: 15 minutes
   - Channels: PagerDuty, SMS, Slack
   - Auto-remediation: Yes

2. **Warning**
   - Response time: 30 minutes
   - Channels: PagerDuty, Slack
   - Auto-remediation: Configurable

3. **Info**
   - Response time: Best effort
   - Channel: Slack
   - Auto-remediation: No

### Alert Rules
```yaml
# Configure in k8s/monitoring/alert-rules.yaml
- alert: HighErrorRate
  expr: job:error_rate:5m > 0.05
  for: 5m
  labels:
    severity: warning
  annotations:
    description: "Error rate above 5%"

- alert: CriticalLatency
  expr: job:request_duration_seconds:p95 > 2
  for: 5m
  labels:
    severity: critical
  annotations:
    description: "P95 latency above 2s"
```

### Escalation Procedures
1. First responder (15 minutes)
2. Secondary on-call (30 minutes)
3. Engineering manager (45 minutes)

## Disaster Recovery

### Backup Procedures
1. **Redis Data**
   - Frequency: Every 15 minutes
   - Retention: 24 hourly, 7 daily, 4 weekly
   - Storage: S3 with encryption

2. **Configuration**
   - Frequency: Hourly
   - Includes: ConfigMaps, Secrets
   - Version controlled

3. **Application State**
   - Frequency: Every 2 hours
   - Type: Volume snapshots
   - Consistency checks included

### Recovery Procedures
1. **Redis Recovery**
   ```bash
   # Restore from backup
   kubectl exec -it redis-0 -n phoenix-orch -- redis-cli RESTORE
   
   # Verify replication
   kubectl exec -it redis-0 -n phoenix-orch -- redis-cli INFO replication
   ```

2. **Config Recovery**
   ```bash
   # Restore configs
   kubectl apply -f backup/configs.yaml
   
   # Verify services
   kubectl get pods -n phoenix-orch
   ```

3. **State Recovery**
   ```bash
   # Restore volume
   kubectl apply -f backup/volume-restore.yaml
   
   # Verify consistency
   kubectl exec -it master-orchestrator-0 -- /usr/local/bin/verify-state
   ```

## Rollback Procedures

### Automatic Rollbacks
The rollback controller (`core/rollback-controller`) monitors:
- Error rates
- Latency spikes
- Resource exhaustion
- Custom health checks

### Manual Rollback
```bash
# View deployment history
kubectl rollout history deployment/master-orchestrator -n phoenix-orch

# Rollback to previous version
kubectl rollout undo deployment/master-orchestrator -n phoenix-orch

# Monitor rollback
kubectl rollout status deployment/master-orchestrator -n phoenix-orch
```

### Verification Steps
1. Check service health endpoints
2. Verify metrics in Grafana
3. Confirm log patterns
4. Test critical paths

## Maintenance Procedures

### Regular Tasks
1. **Daily**
   - Review error rates
   - Check backup success
   - Verify metrics collection

2. **Weekly**
   - Test recovery procedures
   - Rotate secrets
   - Review resource usage

3. **Monthly**
   - Full DR test
   - Certificate rotation
   - Capacity planning

### Best Practices
1. Always use canary deployments
2. Monitor rollout progress
3. Keep backups verified
4. Test recovery regularly
5. Document all changes

## Troubleshooting

### Common Issues
1. **High Error Rates**
   - Check logs in Loki
   - Review recent changes
   - Verify dependencies

2. **Performance Issues**
   - Check resource usage
   - Review trace spans
   - Analyze DB queries

3. **Recovery Failures**
   - Verify backup integrity
   - Check storage permissions
   - Review consistency checks

### Support Escalation
1. Consult runbooks
2. Check status page
3. Contact on-call team
4. Escalate per severity

## Security Considerations

### Access Control
- RBAC for Kubernetes resources
- Encrypted secrets
- Audit logging enabled

### Data Protection
- Encrypted backups
- Secure communication
- Regular security scans

### Compliance
- Audit trails maintained
- Access logs retained
- Regular security reviews