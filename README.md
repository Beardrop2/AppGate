# AppGate

**Mission:** Protect services that lack robust authentication by putting an OIDC front-door in front of them. AppGate authenticates users against an IdP (e.g., Keycloak) and safely brokers access to HTTP/TCP/UDP backends—plus game-specific adapters (e.g., Foundry).

**Design goals:** multi-process isolation, default-deny posture, minimal trusted computing base, modern **type-safe & memory-safe** implementation (Rust), simple config, strong observability.

---

## Repository layout

```
appgate/
├─ Cargo.toml
├─ rust-toolchain.toml
├─ Makefile
├─ docker-compose.yml
├─ config/
│  ├─ appgate.toml
│  ├─ policy/
│  │  └─ foundry.toml
│  └─ ca.pem
└─ crates/
   ├─ appgate-ipc/         # shared gRPC (tonic) + UDS helpers
   ├─ appgate-policy/      # minimal TOML policy engine (v1)
   ├─ appgate-ctrl/        # controller: health/metrics, future supervisor
   ├─ appgate-auth/        # PDP service: OIDC + decisions (MVP mocks groups)
   ├─ appgate-mod-http/    # HTTP reverse proxy (calls PDP → inject → forward)
   ├─ appgate-mod-tcp/     # TCP gateway (stub)
   ├─ appgate-mod-udp/     # UDP gateway (stub)
   └─ appgate-mod-foundry/ # Foundry adapter (stub)
```

---

## Quick start

Prereqs:

* Rust stable (see `rust-toolchain.toml`)
* Docker (optional, for Compose)
* A Keycloak realm (for now, PDP mocks groups; OIDC validation is the next task)

Build everything:

```bash
cargo build
```

Run PDP + HTTP module locally (simple demo):

```bash
# PDP (policy decision point) over Unix Domain Socket
RUST_LOG=info \
cargo run -p appgate-auth -- \
  --uds /run/appgate/pdp.sock \
  --policy config/policy/foundry.toml &

# HTTP reverse proxy → forwards to a local upstream (e.g., http://127.0.0.1:3000)
RUST_LOG=info \
cargo run -p appgate-mod-http -- \
  --bind 0.0.0.0:8080 \
  --pdp-uds /run/appgate/pdp.sock \
  --upstream http://127.0.0.1:3000
```

Or use the provided `Makefile`:

```bash
make build
make run
```

Docker Compose skeleton (edit `docker-compose.yml` and `config/appgate.toml` as needed):

```bash
docker compose up --build
```

---

## Architecture (multi-process)

```
+-------------------+     UDS/gRPC      +---------------------+
|   appgate-ctrl    | <----------------> |   appgate-auth      |
| (Controller)      |                    | (PDP: OIDC + Policy)|
+----+--------------+                    +----------+----------+
     | UDS registry                                  |
     | routes/keys                                   |
     v                                               v
+---------------------+   UDS/gRPC   +--------------------------+
| appgate-mod-http    | <----------> | PDP (authorize)          |
| (reverse proxy)     |              +--------------------------+
+---------------------+
+---------------------+              +--------------------------+
| appgate-mod-tcp     |  <---------> | PDP                      |
+---------------------+
+---------------------+              +--------------------------+
| appgate-mod-udp     |  <---------> | PDP                      |
| (+ game handlers)   |              +--------------------------+
+---------------------+
+---------------------+
| appgate-mod-foundry |
+---------------------+
```

**Data path (HTTP):**

1. Client request → `appgate-mod-http`
2. Extract session token (cookie for HTTP; opaque token for non-HTTP once implemented)
3. Call PDP (`appgate-auth`) → decision + claims + header map
4. Inject identity headers; sanitize hop-by-hop headers; proxy to upstream

**Data path (TCP/UDP):**

* First connection bytes (TCP preface) / first datagram (UDP) will carry an opaque token (AEAD) → PDP allow/deny → bind 5-tuple to session (to be implemented).

---

## Components

### `appgate-auth` (PDP)

* gRPC service over **Unix Domain Socket** at `/run/appgate/pdp.sock`
* Validates sessions (MVP: groups mocked)
* Evaluates policy (TOML; see `config/policy/foundry.toml`)
* Returns: `allow/deny`, `expiry`, claim map, and headers to inject

### `appgate-mod-http`

* Minimal reverse proxy using Hyper/Axum
* Calls PDP per request; injects headers from decision; forwards to configured upstream
* To do: WebSocket upgrades, header hygiene hardening, request IDs, H/2/H/3 options

### `appgate-mod-tcp` (stub)

* Planned: listener → optional session preface → PDP → connect upstream → zero-copy splice
* Options: SNI peek, TLS passthrough/termination, optional mTLS upstream

### `appgate-mod-udp` (stub)

* Planned: first datagram includes sealed token → PDP → bind `(src,dst)` until expiry/idle
* Optional per-game handlers (e.g., Lidgren guard/bridge)

### `appgate-ctrl`

* Health (`/healthz`) and metrics (`/metrics`) stubs
* Future: config hot-reload, key distribution, process supervision

### `appgate-ipc`

* `tonic` gRPC definitions and helpers (PDP proto)
* UDS server/client adapters

### `appgate-policy`

* Minimal TOML-backed rules:

  * match by `protocol` + `resource` prefix
  * require group(s)
  * optional header injection map

---

## Configuration

`config/appgate.toml` (global, referenced by services)

```toml
[global]
run_dir = "/run/appgate"
log_level = "info"

[certs]
trust_store = "/etc/appgate/ca.pem"

[auth.oidc]
issuer = "https://keycloak/realms/main"
client_id = "appgate"
client_secret = "env:APPGATE_OIDC_SECRET"
redirect_uri = "https://appgate.example.com/oidc/callback"
cookie_name = "appg_sess"
cookie_domain = "example.com"
session_ttl_seconds = 3600

[modules.http]
listeners = [{ bind="0.0.0.0:8080", h3=false }]
```

Example policy (`config/policy/foundry.toml`):

```toml
[[rules]]
name = "foundry-admin"
protocol = "http"
resource = "http://foundry/"
require_groups = ["foundry-admin"]
[rules.inject]
"X-Role" = "admin"

[[rules]]
name = "foundry-players"
protocol = "http"
resource = "http://foundry/"
require_groups = ["foundry-players"]
```

---

## Security posture (MVP → target)

* **Default-deny**: only configured routes/ports are exposed.
* **Sessions**

  * HTTP: cookie (HttpOnly, Secure, SameSite=Lax/Strict), **encrypted & authenticated** (AEAD) — **to be wired**.
  * TCP/UDP: short-lived opaque AEAD tokens — **to be wired**.
* **Header hygiene**: strip inbound `X-Forwarded-*`, `Forwarded`, `Authorization`, `Via`, `TE`, `Upgrade`; inject only configured identity headers.
* **mTLS (optional)**: modules → upstreams.
* **Rate limits**: per-IP tokenless caps; per-session caps (esp. UDP) — **to be wired**.
* **Audit**: structured JSONL; daily signatures — **planned**.

I fancy calling out the obvious: forwarding raw IdP tokens downstream is **off by default**; use minimal, purpose-built headers (sub/email/groups) to reduce leakage risk.

---

## Observability

* **Controller**

  * `/healthz` (liveness/readiness)
  * `/metrics` (Prometheus text) — stubs included
* **Planned**

  * Per-process Prometheus metrics: auth latency, decision rates, active sessions, bytes in/out, drops (reason)
  * OpenTelemetry tracing across PDP ↔ modules ↔ upstream

---

## Running with Keycloak (outline)

1. Create an OIDC client (confidential or public + PKCE).
2. Configure `issuer`, `client_id`, `client_secret`, `redirect_uri` in `config/appgate.toml`.
3. Map groups/roles in Keycloak to your policy expectations (e.g., `foundry-players`).
4. For MVP testing, PDP currently **mocks** groups; the next task is to validate the AppGate session (or ID token) and derive groups from claims.

---

## Development

Format & lint:

```bash
make fmt
make clippy
```

Rebuild proto after editing `crates/appgate-ipc/proto/pdp.proto`:

```bash
cargo clean -p appgate-ipc && cargo build -p appgate-ipc
```

---

## Roadmap (short)

* **Auth**: implement AEAD-sealed session cookies (HTTP) + opaque tokens (TCP/UDP); proper OIDC verification (JWKS, iss/aud/exp/nbf).
* **HTTP**: WS upgrades; header hygiene; request IDs; optional H/3.
* **TCP/UDP**: real forwarders (preface token / first-datagram token), expiry bindings, rate limits.
* **Foundry module**: SSO bridge (AppGate session → Foundry session), minimal auto-provision.
* **Controller**: config hot-reload; key rotation; signed audit logs.
* **Policy**: enrich matcher (host/path globs, SNI rules); consider Cedar/OPA integration.

---

## Threat model (abridged)

* **Spoofing**: OIDC + session proof; sanitize inbound headers; modules never trust client-set identity headers.
* **Tampering**: AEAD on sessions/tokens; TLS on external legs; optional mTLS to upstream.
* **Repudiation**: structured audit logs + daily signatures (planned).
* **Info disclosure**: minimal claim projection; tokens not forwarded by default.
* **DoS**: tokenless caps, per-session limits, circuit breakers (planned).
* **Privilege escalation**: single PDP as decision point; modules enforce, do not decide.

---

## License

MIT (workspace-wide). See `Cargo.toml` and include a `LICENSE` file before publishing.

---

## Contributing

* Keep modules thin; push policy/identity decisions into PDP.
* Prefer small, auditable dependencies.
* Add tests: unit (policy, header sanitation), integration (PDP + HTTP proxy), and later soak tests (UDP/TCP).

---

## Status

* PDP service + HTTP reverse proxy are runnable.
* TCP/UDP/Foundry modules are stubs awaiting implementation.
* Config, policy, metrics, and health endpoints are scaffolded.

