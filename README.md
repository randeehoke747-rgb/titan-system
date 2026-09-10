# titan-system
Titan control plane: Rust-based distributed system with Kubernetes orchestration and CI/CD.

## Control plane endpoints
- `GET /` returns the same liveness payload as `/health` for Render-friendly root health checks.
- `GET /health` returns liveness plus current agent availability.
- `GET /ready` returns readiness based on configured safe-wallet routing and minimum healthy agents.
- `GET /status` returns queue, hold, release, redundancy, auth, persistence, and configuration status.
- `GET /history` returns append-only audit history for authorized hold/release events.
- `GET /alerts` lists queued review alerts for suspicious transfers.
- `GET /transactions/held` lists transactions currently held in the safe wallet queue.
- `POST /transactions/intake` accepts authorized USDC/USDT intake requests and places them on temporary hold, with all USDC requests automatically treated as high-alert operator-review items.
- `POST /transactions/{transaction_id}/release` requires a bearer token and records an operator-approved release.
- `POST /agents/heartbeat` refreshes redundant monitor agents.
- `POST /agents/{agent_name}/failure` records repeated agent failure for failover visibility.

## Environment
- `SAFE_WALLET_ADDRESS` sets the temporary safe wallet destination.
- `APPROVED_DESTINATIONS` is a comma-separated allowlist of release destinations.
- `MINIMUM_ACTIVE_AGENTS` sets the readiness threshold for healthy monitor agents.
- `PORT` overrides the default HTTP bind port of `8080` for Render-compatible deployments.
- `OPERATOR_API_TOKEN` enables authenticated operator approval for release requests.
- `MONITOR_STATE_PATH` persists monitor state to a JSON file that can survive redeploys when backed by persistent storage.
- `STRICT_STARTUP=true` makes the service fail fast on boot if required safe monitoring configuration is missing.
