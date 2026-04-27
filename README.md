# solid-community-rs

A Rust port of the [Solid Community Server](https://github.com/CommunitySolidServer/CommunitySolidServer) (CSS) — a standards-compliant [Solid](https://solidproject.org/) server implementing the LDP, WAC, and WebID-TLS specifications.

---

## Table of Contents

- [Requirements](#requirements)
- [Workspace layout](#workspace-layout)
- [Quickstart](#quickstart)
- [Running the server](#running-the-server)
- [Running integration tests](#running-integration-tests)
- [Unit tests](#unit-tests)
- [CLI reference](#cli-reference)
  - [solid-server](#solid-server)
  - [solid-test](#solid-test)
- [Environment variables](#environment-variables)
- [Architecture](#architecture)
- [Contributing](#contributing)

---

## Requirements

| Tool | Minimum version |
|------|----------------|
| Rust | 1.76 (stable) |
| Cargo | ships with Rust |

Install Rust via [rustup](https://rustup.rs/):

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

---

## Workspace layout

```
solid-community-rs/
├── Cargo.toml                  # workspace manifest
└── crates/
    ├── http-types/             # core domain types (ResourceIdentifier, SolidError, …)
    ├── storage/                # ResourceStore + KeyValueStorage traits & backends
    ├── authz/                  # authorisation traits (WAC / ACP)
    ├── identity/               # account, pod, WebID, client-credentials traits
    ├── static-assets/          # static file serving (URL prefix → filesystem)
    ├── server-core/            # Axum router, middleware, request pipeline, App lifecycle
    ├── cli/                    # two binaries: solid-server and solid-test
    └── integration-tests/      # HTTP integration test library (consumed by solid-test)
```

---

## Quickstart

```bash
# 1. Clone
git clone https://github.com/tuned-org-uk/solid-community-rs.git
cd solid-community-rs

# 2. Build everything
cargo build --release

# 3. Start the server (defaults: http://localhost:3000/)
cargo run --bin solid-server

# 4. In a second terminal — run the integration tests
cargo run --bin solid-test
```

---

## Running the server

```bash
# Development build (fastest compile)
cargo run --bin solid-server

# Release build (optimised)
cargo run --release --bin solid-server

# Custom port, host, and base URL
cargo run --bin solid-server -- \
  --port 4000 \
  --host 0.0.0.0 \
  --base-url http://my-server.example/

# File-backed storage (persists data to disk)
cargo run --bin solid-server -- --root-dir ./data

# Verbose logging
cargo run --bin solid-server -- --log-level debug
```

---

## Running integration tests

The `solid-test` binary connects to a **running** Solid server and exercises its HTTP API with a TAP-formatted report. It works against both this Rust server and the original TypeScript CSS.

```bash
# 1. Start the server (in one terminal)
cargo run --bin solid-server -- --port 3001

# 2. Run all integration tests (in another terminal)
cargo run --bin solid-test -- --base-url http://localhost:3001/

# Run only a specific suite (substring match, case-insensitive)
cargo run --bin solid-test -- --filter resource-crud

# Verbose: print every request and response line
cargo run --bin solid-test -- --verbose

# Against an external / TypeScript CSS instance
cargo run --bin solid-test -- --base-url https://my-css-instance.example/
```

### Example output

```
TAP version 14
# health
ok 1 - GET / returns 200 or 401
ok 2 - OPTIONS / returns 204 or 200 with Allow header
ok 3 - HEAD / does not return a body
# resource-crud
ok 4 - PUT creates a new document and returns 201
ok 5 - GET returns the stored body after PUT
ok 6 - GET on absent resource returns 404
ok 7 - PUT on existing resource overwrites and returns 200 or 204
ok 8 - DELETE returns 204 and removes the resource
ok 9 - DELETE on absent resource returns 404
ok 10 - GET returns matching Content-Type
# containers
ok 11 - GET / responds with 200 or 401 (root is a container)
ok 12 - PUT to container URL creates container (201)
ok 13 - POST to container creates child resource (201)
ok 14 - GET on container includes ldp:Container Link header
ok 15 - DELETE on empty container returns 204
# content-negotiation
ok 16 - GET with Accept: text/turtle returns Turtle
ok 17 - GET with Accept: application/ld+json returns JSON-LD or 406
ok 18 - GET plain-text resource echoes text/plain
ok 19 - GET with unsupported Accept type returns 406 or 200
# error-responses
ok 20 - GET unknown path returns 404
ok 21 - PATCH without supported patch Content-Type returns 415 or 405
ok 22 - 404 response body describes the error
ok 23 - PUT to path with missing parent returns 201 (auto-create) or 404/409
1..23
# passed: 23  failed: 0
```

The runner exits with code `0` on full pass and `1` if any test fails, making it suitable for CI pipelines.

---

## Unit tests

Each crate carries its own `#[cfg(test)]` modules. Run them with:

```bash
# All crates
cargo test

# Single crate
cargo test -p http-types
cargo test -p storage

# Single test by name
cargo test -p http-types not_found_is_client_error

# With output (useful for debugging)
cargo test -- --nocapture
```

### Test coverage by crate

| Crate | Test suites ported from TypeScript |
|-------|-----------------------------------|
| `http-types` | `HttpError.test.ts`, `ResourceIdentifier.test.ts` |
| `storage` | `BaseResourceStore.test.ts`, `PassthroughStore.test.ts`, `ReadOnlyStore.test.ts`, `MemoryMapStorage.test.ts` |

---

## CLI reference

### `solid-server`

Starts the HTTP server.

```
Usage: solid-server [OPTIONS]

Options:
  -b, --base-url <URL>     Base URL advertised to clients
                           [env: CSS_BASE_URL] [default: http://localhost:3000/]
  -p, --port <PORT>        TCP port to listen on
                           [env: CSS_PORT] [default: 3000]
      --host <HOST>        Hostname or IP to bind to
                           [env: CSS_HOST] [default: localhost]
  -l, --log-level <LEVEL>  Log level: trace | debug | info | warn | error
                           [env: CSS_LOG_LEVEL] [default: info]
      --root-dir <PATH>    Root directory for file-backed storage
                           [env: CSS_ROOT_DIR] (omit for in-memory storage)
  -h, --help               Print help
  -V, --version            Print version
```

### `solid-test`

Runs the HTTP integration test suite against any live Solid server.

```
Usage: solid-test [OPTIONS]

Options:
  -b, --base-url <URL>       Base URL of the server under test
                             [env: CSS_BASE_URL] [default: http://localhost:3000/]
      --filter <SUBSTR>      Only run suites whose name contains SUBSTR
                             (case-insensitive)
  -v, --verbose              Print each request/response line for all tests
      --timeout-ms <MS>      Per-request timeout in milliseconds [default: 10000]
  -h, --help                 Print help
  -V, --version              Print version
```

---

## Environment variables

All CLI flags can be set via environment variables, which take precedence over defaults but are overridden by explicit flags.

| Variable | Flag equivalent | Default |
|----------|----------------|---------|
| `CSS_BASE_URL` | `--base-url` | `http://localhost:3000/` |
| `CSS_PORT` | `--port` | `3000` |
| `CSS_HOST` | `--host` | `localhost` |
| `CSS_LOG_LEVEL` | `--log-level` | `info` |
| `CSS_ROOT_DIR` | `--root-dir` | *(in-memory)* |
| `RUST_LOG` | *(overrides `--log-level` entirely)* | — |

`RUST_LOG` follows the standard `tracing-subscriber` filter syntax, e.g.:

```bash
RUST_LOG=solid_storage=debug,solid_server=info cargo run --bin solid-server
```

---

## Architecture

The server is structured as a set of focused crates that mirror the TypeScript CSS package layout:

```
Request
  │
  ▼
solid-server (CLI / AppConfig)
  │
  ▼
server-core  ─── Axum router + middleware (CORS, request-id)
  │               └─ RequestPipeline
  │                    └─ per-method handlers
  │
  ├─► http-types      ResourceIdentifier, Operation, Representation, SolidError
  ├─► storage         ResourceStore (CRUD) + KeyValueStorage + backends
  ├─► authz           Authorizer / PermissionReader traits (WAC / ACP)
  ├─► identity        Account, Pod, WebID, ClientCredentials traits
  └─► static-assets   URL-prefix → filesystem mapping, MIME detection
```

### Key design principles

- **Trait-based, not class-based.** Every storage backend, authoriser, and handler is an async trait — swap implementations without touching call sites.
- **`ChangeMap` for reactive updates.** Every mutating `ResourceStore` method returns a `HashMap<url, ChangeMetadata>` so monitoring layers (notifications, webhooks) can react to fine-grained changes.
- **Mirror the TypeScript.** Module paths, type names, and test names intentionally match their CSS counterparts to make cross-referencing straightforward.

---

## Contributing

```bash
# Format
cargo fmt --all

# Lint
cargo clippy --all-targets --all-features -- -D warnings

# Test
cargo test --all

# Check before pushing
cargo fmt --all && cargo clippy --all-targets -- -D warnings && cargo test --all
```

Please open an issue before submitting large pull requests.
