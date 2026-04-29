//! Phase 0 governance plane.
//!
//! Owns three concerns that the strategic plan keeps decoupled from
//! the LDP/WAC core:
//!
//! 1. **Catalog** — registration of governed datasets as Solid-addressable
//!    resources (`/pods/{org}/datasets/{dataset_id}/`).
//! 2. **Capability tokens** — DPoP-bound JWT-shaped tokens whose claims
//!    bind a request to a dataset, access tier, byte cap, expiry, and
//!    requester WebID.
//! 3. **Audit** — append-only NDJSON event ledger per dataset, kept on
//!    disk under the catalog root so it can be served as a Solid
//!    resource.
//!
//! Phase 1 will add tier enforcement middleware and projections; this
//! crate is structured so those additions are downstream of the
//! existing types.

pub mod audit;
pub mod catalog;
pub mod policy;
pub mod tokens;

pub use audit::{AuditEvent, AuditLog};
pub use catalog::{Catalog, CatalogEntry, CatalogError};
pub use policy::{AccessPolicy, AccessRule, Tier};
pub use tokens::{CapabilityClaims, TokenError, TokenIssuer};
