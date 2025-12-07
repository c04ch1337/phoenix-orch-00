# Kubernetes Deployment Configuration

## 1. Namespace Configuration
```yaml
apiVersion: v1
kind: Namespace
metadata:
  name: phoenix-orch
```

## 2. Resource Quotas
```yaml
apiVersion: v1
kind: ResourceQuota
metadata:
  name: phoenix-orch-quota
  namespace: phoenix-orch
spec:
  hard:
    requests.cpu: "16"
    requests.memory: "32Gi"
    limits.cpu: "32"
    limits.memory: "64Gi"
    pods: "50"
```

## 3. Master Orchestrator Deployment
```yaml
apiVersion: apps/v1
kind: StatefulSet
metadata:
  name: master-orchestrator
  namespace: phoenix-orch
spec:
  serviceName: master-orchestrator
  replicas: 3
  selector:
    matchLabels:
      app: master-orchestrator
  template:
    metadata:
      labels:
        app: master-orchestrator
    spec:
      affinity:
        podAntiAffinity:
          requiredDuringSchedulingIgnoredDuringExecution:
            - labelSelector:
                matchExpressions:
                  - key: app
                    operator: In
                    values:
                      - master-orchestrator
              topologyKey: "kubernetes.io/hostname"
      containers:
        - name: master-orchestrator
          image: phoenix-orch/master-orchestrator:latest
          ports:
            - containerPort: 8080
              name: http
            - containerPort: 9000
              name: metrics
          resources:
            requests:
              cpu: "1"
              memory: "2Gi"
            limits:
              cpu: "2"
              memory: "4Gi"
          env:
            - name: REDIS_URL
              valueFrom:
                configMapKeyRef:
                  name: phoenix-orch-config
                  key: redis_url
          livenessProbe:
            httpGet:
              path: /health
              port: http
            initialDelaySeconds: 30
            periodSeconds: 10
          readinessProbe:
            httpGet:
              path: /health
              port: http
            initialDelaySeconds: 5
            periodSeconds: 5
```

## 4. Redis StatefulSet
```yaml
apiVersion: apps/v1
kind: StatefulSet
metadata:
  name: redis
  namespace: phoenix-orch
spec:
  serviceName: redis
  replicas: 3
  selector:
    matchLabels:
      app: redis
  template:
    metadata:
      labels:
        app: redis
    spec:
      containers:
        - name: redis
          image: redis:7.0-alpine
          command: ["redis-server"]
          args: ["--requirepass", "$(REDIS_PASSWORD)"]
          ports:
            - containerPort: 6379
              name: redis
          resources:
            requests:
              cpu: "2"
              memory: "4Gi"
            limits:
              cpu: "4"
              memory: "8Gi"
          env:
            - name: REDIS_PASSWORD
              valueFrom:
                secretKeyRef:
                  name: redis-secret
                  key: password
          volumeMounts:
            - name: redis-data
              mountPath: /data
  volumeClaimTemplates:
    - metadata:
        name: redis-data
      spec:
        accessModes: ["ReadWriteOnce"]
        resources:
          requests:
            storage: 50Gi
```

## 5. Load Balancer Service
```yaml
apiVersion: v1
kind: Service
metadata:
  name: master-orchestrator
  namespace: phoenix-orch
spec:
  type: LoadBalancer
  ports:
    - port: 80
      targetPort: 8080
      protocol: TCP
      name: http
  selector:
    app: master-orchestrator
```

## 6. Horizontal Pod Autoscaling
```yaml
apiVersion: autoscaling/v2
kind: HorizontalPodAutoscaler
metadata:
  name: master-orchestrator
  namespace: phoenix-orch
spec:
  scaleTargetRef:
    apiVersion: apps/v1
    kind: StatefulSet
    name: master-orchestrator
  minReplicas: 3
  maxReplicas: 10
  metrics:
    - type: Resource
      resource:
        name: cpu
        target:
          type: Utilization
          averageUtilization: 70
    - type: Resource
      resource:
        name: memory
        target:
          type: Utilization
          averageUtilization: 80
```

## 7. Network Policies
```yaml
apiVersion: networking.k8s.io/v1
kind: NetworkPolicy
metadata:
  name: master-orchestrator-policy
  namespace: phoenix-orch
spec:
  podSelector:
    matchLabels:
      app: master-orchestrator
  policyTypes:
    - Ingress
    - Egress
  ingress:
    - from:
        - podSelector:
            matchLabels:
              app: redis
      ports:
        - protocol: TCP
          port: 6379
    - ports:
        - protocol: TCP
          port: 8080
        - protocol: TCP
          port: 9000
  egress:
    - to:
        - podSelector:
            matchLabels:
              app: redis
      ports:
        - protocol: TCP
          port: 6379
    - ports:
        - protocol: TCP
          port: 53
          protocol: UDP
          port: 53
```

## 8. ConfigMap
```yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: phoenix-orch-config
  namespace: phoenix-orch
data:
  redis_url: "redis://redis-0.redis:6379,redis-1.redis:6379,redis-2.redis:6379"
  metrics_addr: ":9000"
  log_level: "info"
```

## Implementation Notes

1. **StatefulSet vs Deployment**
   - Using StatefulSet for both master-orchestrator and Redis to maintain stable network identities
   - Enables proper Redis replication configuration
   - Allows for orderly scaling and updates

2. **Anti-Affinity Rules**
   - Ensures high availability by spreading pods across nodes
   - Prevents multiple instances from running on the same node

3. **Resource Management**
   - Conservative initial resource requests
   - Room for vertical scaling if needed
   - HPA configured for automatic horizontal scaling

4. **Security**
   - Network policies restrict pod communication
   - Redis password stored in Kubernetes secret
   - Service accounts and RBAC to be configured separately

5. **Monitoring**
   - Prometheus metrics exposed on port 9000
   - Liveness and readiness probes configured
   - HPA metrics based on CPU and memory utilization

6. **Data Persistence**
   - Redis data persisted using PersistentVolumeClaims
   - 50GB storage allocated per Redis instance
   - Backup strategy to be implemented separately

7. **Networking**
   - Internal service discovery using Kubernetes DNS
   - Load balancer for external access
   - Network policies restrict pod-to-pod communication

## Next Steps

1. Create Kubernetes secrets for sensitive data
2. Configure backup and restore procedures
3. Set up monitoring and alerting
4. Implement CI/CD pipeline for deployments
5. Create runbooks for common operations