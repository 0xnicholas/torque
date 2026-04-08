# Session Agent

Lightweight session-based agent system with streaming output and tool calling.

## Features

- Multi-turn conversations with session persistence
- Real-time streaming via SSE
- Tool calling loop (up to 20 calls per request)
- Sliding window context management
- API key authentication

## Quick Start

### Prerequisites

- PostgreSQL database
- OpenAI API key

### Setup

1. Copy environment file:
   ```bash
   cp .env.example .env
   # Edit .env with your configuration
   ```

2. Run migrations:
   ```bash
   sqlx migrate run
   ```

3. Start service:
   ```bash
   cargo run -p session-agent
   ```

## API Endpoints

### Create Session
```bash
POST /sessions
Headers: X-API-Key: your-api-key

Response:
{
  "id": "uuid",
  "status": "idle",
  "created_at": "2024-01-01T00:00:00Z"
}
```

### Send Message (Streaming)
```bash
POST /sessions/{id}/chat
Headers: X-API-Key: your-api-key
Content-Type: application/json

Body:
{
  "message": "Hello, agent!"
}

Response: SSE Stream
data: {"event":"chunk","content":"Hello"}
data: {"event":"chunk","content":" there"}
data: {"event":"done","message_id":"uuid","artifacts":null}
```

### List Messages
```bash
GET /sessions/{id}/messages
Headers: X-API-Key: your-api-key

Response:
[
  {
    "id": "uuid",
    "role": "user",
    "content": "Hello!",
    "created_at": "2024-01-01T00:00:00Z"
  }
]
```

## Environment Variables

- `DATABASE_URL` - PostgreSQL connection string
- `LLM_API_KEY` - OpenAI API key
- `LLM_MODEL` - Model name (default: gpt-4o-mini)
- `BIND_ADDR` - Server bind address (default: 0.0.0.0:3000)

## Architecture

```
HTTP API (Axum)
  → Auth Middleware
  → Session/Message Handlers
  → Agent Runner
    → Context Manager (sliding window)
    → LLM Client (streaming)
    → Tool Registry
  → PostgreSQL
```
