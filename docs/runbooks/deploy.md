# Runbook: Deploying the Master Orchestrator

This runbook describes the standard deployment procedure for the master orchestrator and its agents. It assumes a containerized deployment using the workspace `Dockerfile` and a separate infrastructure layer (Kubernetes, Docker Compose, or similar).

---

## 1. Preconditions

Before deploying:

1. **Code & Config**
   - All changes merged to the main branch.
   - `CHANGELOG.md` updated (including the new version).
   - `.env` (or environment-specific secrets/config) prepared for the target environment (`APP_ENV=dev|staging|prod`).
   - Data/config overlays present:
     - [`data/config.toml`](../../data/config.toml)
     - [`data/config.<env>.toml`](../../data)

2. **Dependencies**
   - Rust toolchain installed (for local builds/tests).
   - Docker available on the deployment build host.
   - Access to:
     - LLM provider API keys (`OPENROUTER_API_KEY`, etc.).
     - Target Git repo root for `git_agent`.
     - Obsidian vault root for `obsidian_agent`.

3. **Runtime Environment**
   - Target cluster/hosts reachable.
   - Monitoring/metrics stack ready to scrape `METRICS_ADDR` (e.g. `127.0.0.1:9000` from inside the pod/container).

---

## 2. Pre-Deploy Checks

From the repo root:

```bash
cargo fmt --all
cargo test --workspace
cd frontend
npm install
npm test
cd ..
```

If any of these commands fail, **stop** and resolve the issue before proceeding.

---

## 3. Build Docker Image

From the repo root:

```bash
# Example: build image tagged with version from core/master_orchestrator/Cargo.toml
VERSION=$(grep '^version' core/master_orchestrator/Cargo.toml | head -n1 | cut -d'"' -f2)

docker build -t phoenix-orch:${VERSION} .
```

Optionally tag for a registry:

```bash
docker tag phoenix-orch:${VERSION} your-registry/phoenix-orch:${VERSION}
docker push your-registry/phoenix-orch:${VERSION}
```

Confirm the image contains:

- `master_orchestrator`, `git_agent`, `obsidian_agent`, `llm_router_agent` in `/usr/local/bin`.
- `data/` and `frontend/` copied into the image.
- `APP_ENV` default set to `prod` in the container environment.

---

## 4. Configure Environment

For each environment (e.g. `staging`, `prod`):

1. Prepare environment variables (via secrets manager, Kubernetes `Secret`, `.env` mounted as a file, etc.):

   - **Secrets**
     - `OPENROUTER_API_KEY` (or other provider keys)
     - `GEMINI_API_KEY`, `GROK_API_KEY`, `OPENAI_API_KEY`, `ANTHROPIC_API_KEY` (if used)
     - `ORCH_API_TOKEN`

   - **Non-secrets**
     - `APP_ENV=staging` or `APP_ENV=prod`
     - `METRICS_ADDR=0.0.0.0:9000` (or environment-specific address)
     - `GIT_AGENT_REPO_ROOT=/path/to/repo`
     - `OBSIDIAN_AGENT_VAULT_ROOT=/path/to/vault`

2. Ensure that:

   - `data/config.<env>.toml` reflects environment-appropriate timeouts, retries, and circuit-breaker thresholds.
   - The configured LLM provider (`llm.default_provider`) has a valid API key and base URL.

---

## 5. Deploy Procedure

The exact steps depend on your orchestrator (Kubernetes, Docker Compose, etc.). A typical deployment looks like:

### 5.1 Kubernetes (Example)

1. **Update Image Tag**

   - Edit your `Deployment` (or Helm values) to use `phoenix-orch:${VERSION}` or `your-registry/phoenix-orch:${VERSION}`.

2. **Apply Manifests**

   ```bash
   kubectl apply -f k8s/namespace.yml
   kubectl apply -f k8s/secrets.yml
   kubectl apply -f k8s/configmap.yml
   kubectl apply -f k8s/deployment.yml
   kubectl apply -f k8s/service.yml
   ```

3. **Rollout Status**

   ```bash
   kubectl rollout status deployment/master-orchestrator -n <namespace>
   ```

### 5.2 Docker Compose (Example)

1. Update `image:` tag in `docker-compose.yml`.
2. Restart services:

   ```bash
   docker-compose pull
   docker-compose up -d
   ```

---

## 6. Post-Deploy Verification

After the rollout completes:

1. **Health Endpoint**

   ```bash
   curl -H "Authorization: Bearer ${ORCH_API_TOKEN}" http://<host>:8181/health
   ```

   Verify:

   - `status: "ok"`
   - Correct `llm_provider` and `llm_model`.

2. **Chat API (v1)**

   ```bash
   curl -X POST \
     -H "Authorization: Bearer ${ORCH_API_TOKEN}" \
     -H "Content-Type: application/json" \
     -d '{"message":"smoke test from runbook"}' \
     http://<host>:8181/api/v1/chat
   ```

   Confirm:

   - HTTP `200 OK`
   - Response has `api_version`, `status`, and `correlation_id`.

3. **Agent Health**

   ```bash
   curl -H "Authorization: Bearer ${ORCH_API_TOKEN}" http://<host>:8181/api/v1/agents
   ```

   - All expected agents (`llm_router_agent`, `git_agent`, `obsidian_agent`) present.
   - Health is `healthy` or `degraded` under normal conditions, not `unhealthy`.

4. **Metrics Endpoint**

   ```bash
   curl http://<metrics-host>:9000/metrics | head
   ```

   - Confirms metrics exporter is live.
   - Check that `orchestrator_plan_started_total` and HTTP counters appear after sample traffic.

5. **Frontend**

   - Access the frontend (served by the orchestrator or via an external static host).
   - Send a test chat request and verify the UI updates and shows `correlation_id`.

---

## 7. Roll-Forward Strategy

If the deploy is successful but you need to deploy a fix:

1. Repeat the same build/tag/push process with a new version.
2. Update the deployment to point at the new version.
3. Validate using the same post-deploy steps (health, chat, agents, metrics).
4. Keep the previous version image available for rollback (see `rollback.md`).

---

## 8. Operational Notes

- Changes to retry or circuit-breaker configuration require corresponding updates in `data/config.<env>.toml`.
- LLM provider outages will surface as:
  - Increased `agent_call_failures_total`.
  - Plans failing with `ExecutionFailed` or `AgentUnavailable`.
  - Degraded/unhealthy state in `/api/v1/agents`.
- For incident handling (especially around agent failures), see:
  - [`docs/runbooks/incident_agent_failure.md`](incident_agent_failure.md:1)