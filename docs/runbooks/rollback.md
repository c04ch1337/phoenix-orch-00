# Runbook: Rolling Back the Master Orchestrator

This runbook describes how to safely roll back the master orchestrator and agents to a previously known-good version.

---

## 1. When to Roll Back

Consider a rollback when:

- A new deployment causes widespread failures:
  - Increased `orchestrator_plan_failed_total` or `agent_call_failures_total`.
  - `/api/v1/chat` requests frequently return `status: "error"`.
- Critical regressions in:
  - Agent behavior (e.g., unexpected Git or Obsidian modifications).
  - Performance (significant increases in task or agent latency).
- Configuration errors that cannot be quickly corrected with a forward fix.

Roll back instead of patching in place when:

- The root cause is not yet well understood.
- The impact is severe (e.g. production outage).
- A previous version is known to be stable.

---

## 2. Preconditions

- You have identified a **known-good version** (e.g. a tagged image `phoenix-orch:v0.1.0`).
- The previous version’s configuration is still valid or can be re-applied.
- You have sufficient access to:
  - Kubernetes cluster or Docker host.
  - Monitoring dashboards and logs.
  - Git repo and CI/CD configuration.

---

## 3. Kubernetes Rollback (Example)

### 3.1 Identify Current and Previous Versions

1. Check the current image and tags used by the `Deployment`:

   ```bash
   kubectl get deployment master-orchestrator -o yaml -n <namespace> | grep image:
   ```

2. Identify the version to roll back to (`vX.Y.Z`) from:

   - `CHANGELOG.md`
   - Git tags (`git tag --list`)
   - Image registry (`docker images` or registry UI)

### 3.2 Roll Back the Deployment

**Option A: kubectl rollout undo**

If deployments are tracked normally by Kubernetes:

```bash
kubectl rollout undo deployment/master-orchestrator -n <namespace>
kubectl rollout status deployment/master-orchestrator -n <namespace>
```

This reverts to the previous ReplicaSet configuration (including image).

**Option B: Explicitly Set Image Tag**

If you prefer an explicit image tag:

1. Edit the deployment:

   ```bash
   kubectl set image deployment/master-orchestrator \
     master-orchestrator=your-registry/phoenix-orch:<previous-version> \
     -n <namespace>
   ```

2. Wait for the rollout:

   ```bash
   kubectl rollout status deployment/master-orchestrator -n <namespace>
   ```

---

## 4. Docker Compose Rollback (Example)

1. Update `docker-compose.yml` to reference the previous tag:

   ```yaml
   services:
     master-orchestrator:
       image: your-registry/phoenix-orch:<previous-version>
   ```

2. Apply the change:

   ```bash
   docker-compose pull
   docker-compose up -d
   ```

3. Confirm the running container uses the expected image:

   ```bash
   docker ps --filter "name=master-orchestrator"
   ```

---

## 5. Config Rollback Considerations

- If the issue was caused by config (e.g. timeouts, retries, circuit breaker thresholds), ensure you also roll back:

  - `data/config.<env>.toml` to the previous version, or
  - Environment-specific ConfigMap / secret versions.

- Double-check critical settings:

  - `llm.default_provider` and its API keys.
  - Agent-specific timeouts and retries under `[agents.*]`.

- For complex rollbacks, consider:

  - Reverting the Git commit that changed config.
  - Re-deploying any associated `ConfigMap` or secret.

---

## 6. Post-Rollback Verification

After the rollback completes:

1. **Health Endpoint**

   ```bash
   curl -H "Authorization: Bearer ${ORCH_API_TOKEN}" http://<host>:8181/health
   ```

   - Expect `status: "ok"`.
   - LLM provider/model should match the previous known-good configuration.

2. **Chat API (v1) Smoke Test**

   ```bash
   curl -X POST \
     -H "Authorization: Bearer ${ORCH_API_TOKEN}" \
     -H "Content-Type: application/json" \
     -d '{"message":"rollback smoke test"}' \
     http://<host>:8181/api/v1/chat
   ```

   - Check for HTTP `200` and `status: "success"` or a well-formed error.
   - Confirm `correlation_id` is present.

3. **Agent Health**

   ```bash
   curl -H "Authorization: Bearer ${ORCH_API_TOKEN}" http://<host>:8181/api/v1/agents
   ```

   - Agent health should return to expected values (`healthy` or occasionally `degraded`).
   - `consecutive_failures` should stabilize or decrease over time.

4. **Metrics & Dashboards**

   - Validate:
     - `orchestrator_plan_failed_total` stops increasing at an abnormal rate.
     - `agent_call_failures_total` returns to baseline.
     - Task latency histograms (`orchestrator_task_duration_seconds`, `agent_call_duration_seconds`) are normal.

5. **Frontend**

   - Verify the UI still loads and can issue chat requests successfully.
   - Confirm `correlation_id` is displayed and can be used to trace logs.

---

## 7. Communication & Follow-Up

1. **Communicate Rollback**

   - Notify relevant stakeholders (engineering, SRE, product) that a rollback was executed:
     - When and to which version.
     - Observed impact before and after.

2. **Open/Update Incident Record**

   - Document:
     - Symptoms leading to rollback.
     - Logs/metrics used in the decision.
     - Any partial mitigations tried before rollback.

3. **Plan Forward Fix**

   - Root cause analysis and RCA document.
   - Additional tests or safeguards (e.g. feature flags, canary releases, more granular metrics) to prevent recurrence.