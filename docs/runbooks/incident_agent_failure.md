# Runbook: Incident – Agent Failure or Degradation

This runbook describes how to detect, diagnose, and mitigate incidents where one or more agents (e.g. `llm_router_agent`, `git_agent`, `obsidian_agent`) are failing or degraded.

---

## 1. Detection

You may suspect an agent issue when:

- **User-visible symptoms**
  - `/api/v1/chat` responses return `status: "error"` for specific flows (e.g. Git or Obsidian operations).
  - The frontend repeatedly shows a degraded or error state for certain actions.
- **Metrics**
  - Spikes in:
    - `agent_call_failures_total`
    - `orchestrator_plan_failed_total`
  - Increases in:
    - `agent_call_duration_seconds`
    - `orchestrator_task_duration_seconds`
- **Health API**
  - `/api/v1/agents` shows agents with:
    - `health = "degraded"` or `"unhealthy"`
    - High `consecutive_failures`
    - Non-`None` `circuit_open_until`

---

## 2. Initial Triage

1. **Confirm Scope**

   - Is the issue affecting:
     - A single agent (e.g. only `git_agent` calls)?
     - All LLM-related operations (`llm_router_agent`)?
     - Only certain tenants/environments?

2. **Check `/api/v1/agents`**

   ```bash
   curl -H "Authorization: Bearer ${ORCH_API_TOKEN}" http://<host>:8181/api/v1/agents | jq
   ```

   - Note:
     - Which agents are `degraded` vs `unhealthy`.
     - `consecutive_failures`.
     - `last_failure_at`, `last_success_at`.
     - `circuit_open_until`.

3. **Inspect Metrics**

   - In your metrics UI:
     - Look at `agent_call_failures_total` over the last 5–30 minutes.
     - Compare `agent_call_duration_seconds` before and after the suspected incident start.
     - Check `orchestrator_plan_failed_total` for increases.

---

## 3. Detailed Diagnosis

### 3.1 Use Correlation IDs

1. Reproduce a failing request if safe:

   - Use the frontend or `curl` to send a failing `/api/v1/chat` request.
   - Capture the `correlation_id` from the response (the UI also displays it).

2. Filter logs by `correlation_id` in your log backend:

   - Look for:
     - Planner logs from `plan_and_execute_v1`.
     - Executor logs from `execute_agent_for_task`.
     - Any `ERROR` or `WARN` entries associated with the agent.

3. Identify failure classification:

   - From logs or metrics, determine if failures are:
     - Timeouts (`ToolError::Timeout`, agent code `504`).
     - Backend failures (`AgentErrorCode::BackendFailure`).
     - IO errors (`AgentErrorCode::Io`).
     - Invalid requests or unsupported actions (non-retryable).

### 3.2 Check Agent Process and Environment

On the agent host/container:

- Validate that the agent binary is present and runnable:
  - `which git_agent`, `which obsidian_agent`, `which llm_router_agent`
- Confirm environment variables:
  - For `llm_router_agent`:
    - Required provider API keys (e.g. `OPENROUTER_API_KEY`).
    - Network connectivity to the LLM provider.
  - For `git_agent`:
    - `GIT_AGENT_REPO_ROOT` exists and is writable where expected.
  - For `obsidian_agent`:
    - `OBSIDIAN_AGENT_VAULT_ROOT` points to a valid vault path.

### 3.3 Review Config & Circuit Breakers

- Inspect `data/config.<env>.toml` for the current environment:
  - Are timeout and retry settings appropriate?
  - Has `failure_threshold` or `cooldown_ms` been changed recently?
- If `circuit_open_until` is far in the future, the agent may remain unavailable longer than intended.

---

## 4. Remediation Options

### 4.1 Restart the Agent or Pod

- If the agent binary is misbehaving or stuck:
  - Restart the agent container or pod.
  - For Kubernetes: `kubectl delete pod <agent-pod> -n <namespace>` (the Deployment/ReplicaSet will recreate it).

### 4.2 Adjust Configuration (Short-Term)

**Warning:** Changes to retries, timeouts, or circuit breakers should be tested in non-prod first.

Possible mitigations:

- **Increase timeouts** for upstream LLM providers if they are slow but eventually succeed.
- **Relax retry counts** if retries are amplifying a downstream outage.
- **Shorten cooldown** windows to allow faster recovery when the provider is back.

Apply changes via:

- Editing `data/config.<env>.toml`.
- Redeploying the orchestrator (see `deploy.md`).

### 4.3 Temporarily Disable an Agent

If a particular agent is broken and causing repeated failures:

1. Update configuration to reduce or avoid its use:
   - For example, in the planner, decide to route certain intents to a different tool or block them.
2. Alternatively, mark it as inactive in the agent registry (if such a path is available via admin tooling or DB changes).
3. Communicate limitations clearly to downstream users.

### 4.4 Roll Back

If the incident started immediately after a new release:

- Follow [`docs/runbooks/rollback.md`](rollback.md:1) to revert to the previous version and config.

---

## 5. Post-Mitigation Verification

After applying a fix or mitigation:

1. Confirm `/api/v1/agents` shows improved health:
   - `consecutive_failures` stable or decreasing.
   - `circuit_open_until` either `null` or in the past.
   - `health` transitions back toward `healthy`.

2. Watch key metrics:

   - `agent_call_failures_total` plateauing or dropping.
   - `orchestrator_plan_failed_total` returning to baseline.
   - `agent_call_duration_seconds` within expected bounds.

3. Run a targeted smoke test:

   - Send representative requests exercising the previously failing path.
   - Verify responses are successful and include valid `correlation_id`s.

---

## 6. Communication & Incident Documentation

1. **During Incident**

   - Keep a live log of:
     - Timeline of events (detection, actions, results).
     - Metrics screenshots or links.
     - Configuration changes and rollouts.

2. **After Resolution**

   - File or update an incident/RCA document covering:
     - Root cause (once understood).
     - User impact and duration.
     - Remediation steps taken.
     - Follow-up work (tests, alerts, config guardrails).

3. **Preventive Actions**

   - Add or tune alerts on:
     - `agent_call_failures_total` rate.
     - `orchestrator_plan_failed_total` rate.
     - Fraction of agents in `degraded` or `unhealthy` state.
   - Consider:
     - More granular metrics (if/when labels are introduced).
     - Additional tests around newly problematic flows.