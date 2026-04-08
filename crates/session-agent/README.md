# Session Agent MVP

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
cargo run -p session-agent
```

The service runs SQLx migrations automatically at startup.

## MVP API Surface

- `POST /sessions`
- `GET /sessions`
- `GET /sessions/{id}`
- `GET /sessions/{id}/messages`
- `POST /sessions/{id}/chat` (SSE)

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
data: {"event":"chunk","content":"..."}
data: {"event":"chunk","content":"..."}
data: {"event":"done","message_id":"...","artifacts":null}
```

On failure:

```txt
data: {"event":"error","code":"AGENT_ERROR","message":"..."}
```

### 4. Read Session Messages

```bash
curl -s "$BASE_URL/sessions/$SESSION_ID/messages" \
  -H "X-API-Key: $API_KEY"
```

## Notes

- Current context strategy is a bounded recent window.
- Current built-in tool set is intentionally minimal and demo-safe.
- The implementation is deliberately narrower than the full Torque target architecture described in `docs/superpowers/specs/`.
