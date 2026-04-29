# Phase 1 — Governed Embedding Store GA (Weeks 6–18)

Phase 1 makes the platform customer-facing by wiring the governance console to the live control plane, enforcing access tiers, exposing audit trails, and shipping the first CLI and Python client around the governed embedding store.[file:1][file:2] The strategic plan defines this phase as the beachhead product because it directly upgrades existing embedding-serving users with lineage, access control, and auditability.[file:1]

## Objectives

The main objective is a usable governed embedding product with Discovery, Evaluation, Training, and initial Inference tier support, delivered as both self-hosted and hosted deployments.[file:1] The updated decisions add that the console remains a Deno SPA in pure TypeScript and the audit model stays Solid-native as the source of truth, with any telemetry/export layer treated as a projection rather than a replacement.[file:1][page:1]

## Architectural direction

### Solid-native audit model

The audit ledger should begin as append-only Solid-addressable resources under each dataset, because the instruction is to use Solid wherever possible and only go custom where there is no standard alternative.[file:1] A practical pattern is one append-only NDJSON resource per dataset version, plus a summary projection generated for UI filtering and metering.

```text
/pods/{org}/datasets/{dataset_id}/audit/
  ├── events-2026-04.ndjson
  ├── events-2026-05.ndjson
  └── summary.json
```

The authoritative record is the NDJSON event stream; `summary.json` is a derived materialized view for the UI and billing dashboards.

### Tier enforcement

The strategic plan already defines the tier model and pricing logic.[file:1] Request processing should enforce tier semantics in middleware before chunk retrieval reaches storage.

| Tier | Enforcement |
|---|---|
| Discovery | Metadata and preview chunks only |
| Evaluation | Capped byte budget per token or organization |
| Training | Full-resolution access with audit trail |
| Inference | Result-only egress through a separate handler |

## Dataset policy representation

Policies should remain linked to dataset containers and represent both default entitlements and issued capabilities.[file:1]

```json
{
  "dataset_id": "org-a/bert-v2",
  "default_tier": "discovery",
  "rules": [
    {
      "principal": "https://partner.example/webid#me",
      "tier": "evaluation",
      "expires_at": "2026-12-31T23:59:59Z",
      "byte_cap": 20000000000
    }
  ]
}
```

This can live in a policy resource beside the dataset while still being enforced via DPoP-bound runtime tokens.[file:1]

## CLI design

The strategic plan explicitly calls for `solid-zarr-cli` with `upload`, `register`, `set-policy`, and `audit` commands.[file:1] The CLI should be treated as a first-class operator and customer tool, not a debug utility.

### Command shape

```bash
solid-zarr upload --source ./embeddings.zarr --dataset org-a/bert-v2
solid-zarr register --dataset org-a/bert-v2 --title "BERT v2 Embeddings"
solid-zarr set-policy --dataset org-a/bert-v2 --principal https://partner.example/webid#me --tier evaluation --expires 90d
solid-zarr audit --dataset org-a/bert-v2 --since 7d
```

### Rust CLI sketch

```rust
#[derive(clap::Subcommand)]
enum Commands {
    Upload { source: String, dataset: String },
    Register { dataset: String, title: String },
    SetPolicy { dataset: String, principal: String, tier: String, expires: String },
    Audit { dataset: String, since: String },
}
```

## Python client

The Phase 1 client should make governed arrays feel native to ML engineers.[file:1] The client can wrap Zarr HTTP access while injecting DPoP-bound authorization headers.

```python
from genefold import load_array, issue_token

token = issue_token(
    server="https://pods.genefold.ai",
    dataset="org-a/bert-v2",
    tier="evaluation",
)

arr = load_array(
    uri="https://pods.genefold.ai/datasets/org-a/bert-v2/embeddings.zarr",
    token=token,
)
```

### Client architecture

- Lightweight HTTP store wrapper for `zarr-python`
- Auth header and DPoP proof injection
- Lazy chunk fetch on slice access
- Optional helpers for audit inspection and metadata fetch

## Governance console scope

The Deno SPA should now grow into a real control plane, but still stay framework-light in implementation.[page:1] The UI modules for this phase should be:

- Dataset catalog view
- Dataset detail and versions view
- Policy editor
- Token issuance modal
- Audit trail browser
- Pricing/metering summary view

### Suggested TypeScript module layout

```text
governance-console/src/
  api/
    catalog.ts
    tokens.ts
    audit.ts
    policy.ts
  state/
    app_state.ts
  views/
    catalog_view.ts
    dataset_view.ts
    policy_view.ts
    audit_view.ts
  ui/
    table.ts
    modal.ts
    badge.ts
```

### Example API client wrapper

```ts
export async function fetchDataset(id: string) {
  const res = await fetch(`/catalog/${id}`, {
    headers: { "Accept": "application/json" },
  });
  if (!res.ok) throw new Error(`Failed to load dataset ${id}`);
  return await res.json();
}
```

## Deployment and packaging

The strategic plan requires both self-hosted and hosted options from the early phases.[file:1] Phase 1 should produce:
- A Podman-based self-hosted install path
- A hosted multi-tenant pod offering
- Versioned release artifacts for the Rust server, CLI, and console build

### Example Podman compose

```yaml
services:
  api:
    image: solid-zarr-api:latest
    ports: ["8080:8080"]
    volumes:
      - ./data:/data:Z
  console:
    image: governance-console:latest
    ports: ["3000:3000"]
```

## Testing and release controls

Phase 1 adds product-level correctness requirements on top of protocol tests.[file:1]

### Required tests

- Tier enforcement integration tests
- Audit append-only tests
- CLI round-trip tests (`upload` → `register` → `set-policy`)
- Python client e2e tests
- Console tests for core workflows in Deno

### Metering check

Billing summaries should be derived from audit projections and validated against raw audit events to avoid drift between accounting and access logs.[file:1]

## Development workflow embedding

A Phase 1 release checklist should be encoded in tasks and scripts.

```bash
cargo test --workspace
cargo run -p solid-zarr-cli -- audit --dataset org-a/bert-v2 --since 7d
cd governance-console && deno task lint && deno task test && deno task build
podman compose up --build
```

## Exit criteria

Phase 1 is complete when a user can upload and register a dataset, issue governed access to a partner, stream the array from Python, and inspect a standards-grounded audit log through the console.[file:1][file:2]
