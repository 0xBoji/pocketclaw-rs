# PocketClaw Android-First P0/P1 Plan

## P0 (must-have)

1. Realtime gateway stream for Android app
- [x] Add authenticated WebSocket endpoint at `/ws/events`.
- [x] Stream inbound/outbound/system events + heartbeat metrics.
- [x] Android monitor can connect/disconnect and display events.

2. Production channel adapters
- [x] WhatsApp adapter (inbound + outbound + reconnect policy).
- [x] Slack adapter (inbound + outbound + auth bootstrap).
- [ ] Channel health endpoint + per-channel status in app dashboard.

3. Tooling parity baseline
- [ ] `sessions_list`, `sessions_history`, `sessions_send`.
- [ ] Channel action tools (telegram/discord/slack minimal set).
- [ ] Tool permission enforcement audit logs (skill -> allowed tools).
- [x] Platform runtime tools: `channel_health`, `metrics_snapshot`, `datetime_now`.

## P1 (next-level)

1. Android app as primary control UI
- [x] Live session viewer (active sessions, selected session timeline).
- [x] Chat stream panel (send + receive in one screen).
- [ ] Cron manager UI (list/add/enable/disable/remove).
- [ ] Skills manager UI with approval and requirement checks.

2. Session and routing model
- [ ] Per-session policy (reply mode, retry mode, activation mode).
- [ ] Group isolation and channel/account routing map.
- [ ] Persistent session index for quick resume after restart.

3. Voice/media quality
- [ ] Stable push-to-talk flow for Android background usage.
- [ ] Streaming TTS playback in app.
- [ ] Attachment pipeline consistency (upload, metadata, limits).

## Acceptance checks for current completed slice

- `GET /api/status` works.
- `GET /api/monitor/metrics` works.
- `GET /ws/events` upgrades when auth is valid.
- Android Screen 6 can:
  - Start event stream.
  - See `connected` + `heartbeat` events.
  - See `inbound_message` / `outbound_message` events.
  - Stop event stream cleanly.

## Additional APIs added in this slice

- `GET /api/sessions?limit=30`
- `GET /api/sessions/{session_key}/messages?limit=200`
- `POST /api/sessions/send`
- `POST /api/channels/whatsapp/inbound`
- `GET /api/channels/whatsapp/webhook` (Meta verify)
- `POST /api/channels/whatsapp/webhook` (Meta events)
- `POST /api/channels/slack/inbound`
