# titan-system
Hunter clone control plane for Titan AI agents, built with Rust and Axum.

## Hunter bot endpoints
- `GET /` returns the same liveness payload as `/health`.
- `GET /health` returns liveness plus current Hunter clone availability.
- `GET /ready` returns readiness based on Discord bot configuration and minimum healthy clones.
- `GET /status` returns clone, session, command, auth, persistence, and configuration status.
- `GET /history` returns append-only Hunter session audit events.
- `GET /commands` lists recorded Discord command dispatch events.
- `GET /sessions` lists tracked Hunter sessions.
- `POST /sessions` creates a Hunter clone session from a Discord context payload.
- `POST /sessions/{session_id}/assign` requires a bearer token and assigns a Titan agent clone to the session.
- `POST /sessions/{session_id}/messages` relays user, clone, operator, or system messages into the session transcript.
- `POST /sessions/{session_id}/close` requires a bearer token and closes a Hunter session.
- `POST /agents/heartbeat` refreshes Hunter clone heartbeat, capability, and assignment state.
- `POST /agents/{agent_name}/failure` requires a bearer token and records repeated clone failure for failover visibility.
- `POST /discord/commands` requires a bearer token and ingests Discord command events for spawn, assignment, relay, close, and status flows.

## Environment
- `HUNTER_IDENTITY` sets the default Hunter clone identity.
- `HUNTER_CLONES` seeds named clones using `name:kind:cap1|cap2;...` definitions.
- `ALLOWED_GUILDS` is a comma-separated allowlist of Discord guild IDs.
- `ALLOWED_CHANNELS` is a comma-separated allowlist of Discord channel IDs.
- `MINIMUM_ACTIVE_AGENTS` sets the readiness threshold for healthy Hunter clones.
- `PORT` overrides the default HTTP bind port of `8080`.
- `OPERATOR_API_TOKEN` enables authenticated operator assignment, close, and failure endpoints.
- `DISCORD_BOT_TOKEN` marks the Discord bot runtime as configured for readiness checks.
- `DISCORD_INGEST_TOKEN` enables authenticated Discord command ingestion.
- `MONITOR_STATE_PATH` persists clone, session, command, and audit state to JSON.
- `STRICT_STARTUP=true` makes the service fail fast on boot if required Hunter configuration is missing.
