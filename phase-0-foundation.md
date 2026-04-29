# Phase 0 — Foundation (Weeks 1–6)

Phase 0 establishes the minimum viable governed array server: a fork of `solid-community-rs` that can expose Zarr v3 chunks over HTTP Range, register datasets as governed Solid resources, and support a Deno-managed governance console shell.[file:1][file:2] The strategic plan explicitly identifies this as the fastest path to a working demo by swapping the file KV backend for a chunk store and keeping the initial backend local with `mmap` rather than starting with S3.[file:1]

## Objectives

The exit condition for this phase remains a working integration in which `zarr.open(url)` can read a float32 array over HTTP from the Solid server, while the control plane can browse and register datasets against live APIs.[file:1][file:2] The updated architecture decisions add two constraints: the governance console is implemented as a Deno SPA in pure TypeScript, and local/container workflows are built around Podman rather than Docker.[page:1]

## Architecture choices

### Service core

- Fork `solid-community-rs` to `solid-zarr-rs`.[file:1]
- Preserve the upstream crate boundaries: `authz`, `identity`, `server-core`, and `solid-storage`.[file:1]
- Add a new `zarr-storage` crate implementing a binary local chunk backend using `MmapChunkStore`.[file:1]
- Keep all identity, WAC, and Solid resource semantics aligned with upstream Solid behavior wherever possible.[file:1]

### Governance console

- Build the first console as a static SPA using Deno and pure TypeScript.[page:1]
- Avoid React, Preact, and heavier SSR frameworks in this phase; route state and view rendering should be kept explicit and framework-light.[page:1]
- Use the console only for operational views needed in Phase 0: dataset list, dataset detail, registration form, token issue view, and server health panel.[file:1]

### Deployment model

- Local development uses rootless Podman.[file:1]
- Repo container definitions use `Containerfile` naming and Podman-compatible commands.[file:1]
- The console is served as static assets by the Rust server or a separate static container depending on local dev needs.[file:1]

## Monorepo layout

```text
solid-zarr-rs/
├── crates/
│   ├── authz/
│   ├── identity/
│   ├── server-core/
│   ├── solid-storage/
│   └── zarr-storage/
├── governance-console/
│   ├── deno.json
│   ├── import_map.json
│   ├── src/
│   │   ├── main.ts
│   │   ├── router.ts
│   │   ├── api/
│   │   ├── views/
│   │   └── ui/
│   └── static/
├── containers/
│   ├── Containerfile.api
│   └── Containerfile.console
├── scripts/
└── podman-compose.yml
```

This preserves the Rust-first architecture from the strategic plan while making the Deno control plane a first-class workspace component.[file:1][page:1]

## Resource model

The control plane should stay Solid-first and avoid inventing a proprietary governance schema prematurely.[file:1] In this phase, the minimum resource model is:

```text
/pods/{org}/datasets/{dataset_id}/
  ├── .zarr/
  ├── stac-item.json
  ├── policy/
  │   └── access.jsonld
  ├── audit/
  │   └── events.ndjson
  └── metadata/
      └── dataset.jsonld
```

Each dataset is an LDP container with a stable URI, Zarr chunks are addressable resources under the dataset path, policy resources are colocated with the dataset, and audit storage begins as append-only Solid-addressable content.[file:1]

## API surface

The API should be narrow and aligned with the roadmap in the strategic plan.[file:1]

### Required endpoints

- `PUT /catalog/{id}` — create a governed dataset container and initialize catalog metadata.[file:1]
- `GET /catalog/{id}` — fetch dataset metadata and policy summary.
- `POST /catalog/{id}/tokens` — mint DPoP-bound capability tokens.[file:1]
- `GET /catalog/{id}/audit` — return append-only audit resources.
- `GET /datasets/{id}/...` — serve Zarr metadata and chunks with HTTP Range.[file:1]

### Example route sketch in Rust

```rust
Router::new()
    .route("/catalog/:id", put(create_dataset).get(get_dataset))
    .route("/catalog/:id/tokens", post(issue_token))
    .route("/catalog/:id/audit", get(get_audit_stream))
    .route("/datasets/*path", get(get_dataset_resource));
```

## Storage implementation

`zarr-storage` should initially own both chunk layout and metadata address resolution because Phase 0 optimizes for a working governed array server rather than abstraction purity.[file:1] The local implementation should map logical Zarr chunk keys to offsets in one or more `mmap`-backed files, with an index file describing chunk offsets, sizes, and array metadata.[file:1]

### Suggested internal structures

```rust
pub struct ChunkDescriptor {
    pub key: String,
    pub offset: u64,
    pub length: u64,
    pub checksum: Option<String>,
}

pub struct ArrayManifest {
    pub dataset_id: String,
    pub shape: Vec<u64>,
    pub chunk_shape: Vec<u64>,
    pub dtype: String,
    pub descriptors: Vec<ChunkDescriptor>,
}
```

This keeps the storage layout simple enough for local testing while setting up a manifest model that Phase 2 can extend for sharding and indexing.[file:1]

## DPoP-bound capability tokens

Capability tokens are already called out in the strategic plan and should remain the request-time enforcement mechanism.[file:1] The implementation should bind token claims to dataset, tier, expiry, and requester WebID.

```rust
#[derive(Serialize, Deserialize)]
pub struct CapabilityClaims {
    pub sub: String,
    pub webid: String,
    pub dataset_id: String,
    pub access_tier: String,
    pub byte_cap: Option<u64>,
    pub exp: usize,
    pub cnf: serde_json::Value,
}
```

## Governance console implementation

The Deno SPA should be deliberately small and explicit.[page:1] A minimal `deno.json` can encode the workflow:

```json
{
  "tasks": {
    "dev": "deno run -A --watch src/dev.ts",
    "lint": "deno lint",
    "fmt": "deno fmt",
    "test": "deno test -A",
    "build": "deno run -A src/build.ts"
  },
  "imports": {
    "@std/http": "jsr:@std/http",
    "@std/path": "jsr:@std/path"
  }
}
```

### Pure TypeScript SPA pattern

```ts
// governance-console/src/main.ts
import { renderRoute } from "./router.ts";

const root = document.getElementById("app")!;

window.addEventListener("popstate", () => renderRoute(root, location.pathname));
document.addEventListener("click", (event) => {
  const target = event.target as HTMLElement;
  const link = target.closest("a[data-nav]") as HTMLAnchorElement | null;
  if (!link) return;
  event.preventDefault();
  history.pushState({}, "", link.pathname);
  renderRoute(root, link.pathname);
});

renderRoute(root, location.pathname);
```

This keeps the initial app framework-free while still providing route-based navigation and a clean upgrade path if the UI later outgrows manual rendering.[page:1]

## Podman workflow

Local dev and CI should use Podman-first instructions.

```bash
podman build -f containers/Containerfile.api -t solid-zarr-api:dev .
podman build -f containers/Containerfile.console -t governance-console:dev ./governance-console
podman run --rm -p 8080:8080 -v ./data:/data:Z solid-zarr-api:dev
```

A basic `Containerfile.api` should look like:

```dockerfile
FROM rust:1.78-slim AS builder
WORKDIR /app
COPY . .
RUN cargo build --release -p server-core

FROM debian:bookworm-slim
COPY --from=builder /app/target/release/server-core /usr/local/bin/solid-zarr-api
EXPOSE 8080
CMD ["solid-zarr-api"]
```

## Testing and phase gate

Phase 0 testing should cover the existing LDP/WAC contract, new chunk serving logic, and console/API wiring.[file:1]

### Required tests

- Existing upstream integration tests still pass.[file:1]
- New Rust integration test for HTTP Range serving.
- Python test proving `zarr.open(url)` reads remote chunks.[file:1]
- Deno test for console routing and API client wrappers.

### Example Python smoke test

```python
import zarr
arr = zarr.open("http://localhost:8080/datasets/demo/embeddings.zarr", mode="r")
print(arr.shape)
print(arr[0, :8])
```

## Development workflow embedding

The development workflow for this phase should be encoded in the repo, not left in prose.[file:1] Add:
- `Makefile` or task runner aliases for cargo + deno + podman
- CI jobs for Rust tests, Deno lint/test, and image build validation
- A `scripts/bootstrap.sh` for local setup

```bash
#!/usr/bin/env bash
set -euo pipefail
cargo test
cd governance-console && deno task lint && deno task test
podman build -f ../containers/Containerfile.api -t solid-zarr-api:ci ..
```

## Exit criteria

Phase 0 is complete when the server can register a dataset, issue a token, serve Zarr chunks through HTTP Range, and the Deno SPA can browse and inspect that dataset through live APIs.[file:1][file:2]
