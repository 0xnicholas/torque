# Agent Runtime Service MVP

Product-facing single-agent MVP for Torque.

This crate demonstrates:

- persistent sessions
- multi-turn chat
- SSE streaming responses
- bounded context window
- minimal demo-safe tool support

It does not yet demonstrate team orchestration, approvals UI, or recovery UI.

## Prerequisites

- Rust toolchain
- PostgreSQL
- OpenAI-compatible API key

## Environment

Create a `.env` file from `.env.example`, then set:

- `DATABASE_URL` (required)
- `LLM_API_KEY` (required)
- `LLM_BASE_URL` (optional, default `https://api.openai.com/v1`)
- `LLM_AGENT_MODEL` (optional, default `gpt-4o-mini`)
- `BIND_ADDR` (optional, default `0.0.0.0:3000`)

## Run Locally

From repo root:

```bash
cargo run -p agent-runtime-service
```

The service runs SQLx migrations automatically at startup.

## MVP API Surface

- `POST /sessions`
- `GET /sessions`
- `GET /sessions/{id}`
- `GET /sessions/{id}/messages`
- `POST /sessions/{id}/chat` (SSE)
- `GET /metrics`

All endpoints require header:

```txt
X-API-Key: <your-key>
```

## Demo Flow

Set once:

```bash
export API_KEY="demo-key"
export BASE_URL="http://127.0.0.1:3000"
```

### 1. Create Session

```bash
curl -s -X POST "$BASE_URL/sessions" \
  -H "X-API-Key: $API_KEY"
```

Example response:

```json
{
  "id": "4f3c2f13-45d3-4bc2-a0f6-a9b6d0c1c2fd",
  "status": "idle",
  "created_at": "2026-04-08T10:20:30Z"
}
```

Store the returned `id` in `SESSION_ID`.

### 2. List Sessions

```bash
curl -s "$BASE_URL/sessions" \
  -H "X-API-Key: $API_KEY"
```

### 3. Send Message (Streaming)

```bash
curl -N -X POST "$BASE_URL/sessions/$SESSION_ID/chat" \
  -H "X-API-Key: $API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"message":"Give me a concise overview of Torque MVP."}'
```

Expected stream shape:

```txt
data: {"event":"start","session_id":"..."}
data: {"event":"tool_call","name":"web_search","arguments":{"query":"..."}}
data: {"event":"tool_result","name":"web_search","success":true,"content":"...","error":null}
data: {"event":"chunk","content":"..."}
data: {"event":"chunk","content":"..."}
data: {"event":"done","message_id":"...","artifacts":null}
```

On failure:

```txt
data: {"event":"error","code":"AGENT_ERROR","message":"..."}
```

Terminal event contract:

- every `/chat` stream ends with exactly one terminal event: `done` or `error`
- terminal event is always the last event before stream close
- clients should treat stream close without terminal event as transport failure and retry/reconcile

Concurrent chat contract:

- per session, only one `/chat` request may run at a time
- competing request receives HTTP `409 Conflict`
- server increments `session_gate_contention_total` metric for each gate conflict

### 4. Read Session Messages

```bash
curl -s "$BASE_URL/sessions/$SESSION_ID/messages" \
  -H "X-API-Key: $API_KEY"
```

### 5. Read Runtime Metrics

```bash
curl -s "$BASE_URL/metrics" \
  -H "X-API-Key: $API_KEY"
```

Example response:

```json
{
  "session_gate_contention_total": 0
}
```

## Notes

- Current context strategy is a bounded recent window.
- Current built-in tool set is intentionally minimal and demo-safe.
- The implementation is deliberately narrower than the full Torque target architecture described in `docs/superpowers/specs/`.
