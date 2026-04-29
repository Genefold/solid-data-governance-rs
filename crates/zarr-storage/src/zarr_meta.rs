//! Minimal Zarr v3 array-metadata serialization.
//!
//! Generates a `zarr.json` document that `zarr-python` 3.x recognises as a
//! valid v3 array. We deliberately keep this minimal — no codecs beyond
//! the default `bytes` codec, no shards — because the local-mmap blob
//! stores raw chunk bytes one-to-one.
//!
//! # Architectural decisions
//!
//! **Why generate `zarr.json` from the manifest at request time,
//! rather than persist it on disk?** The [`ArrayManifest`] is the
//! single source of truth for shape/dtype/chunk_shape; deriving
//! `zarr.json` on the fly avoids a two-place update problem and lets
//! us evolve the v3 rendering (codec choices, attributes) without
//! migrating stored datasets. The cost is a small amount of work per
//! `GET /datasets/{org}/{dataset}/zarr.json`, which is dwarfed by
//! the surrounding HTTP overhead and is anyway requested once per
//! `zarr.open()`.
//!
//! **Why expose dedicated structs (`ChunkGrid`, `ChunkKeyEncoding`,
//! `Codec`) instead of a freeform `serde_json::Value`?** Typed structs
//! catch typos at compile time, document the v3 shape inline, and let
//! the IDE help when adding fields in Phase 2 (sharding, additional
//! codecs). The trade-off — needing a new struct for each codec we
//! support — is acceptable while the codec set is small.
//!
//! **Why `bytes` as the only codec?** Phase 0's mmap blob stores
//! chunks verbatim with no compression, so claiming a codec like
//! `gzip` or `blosc` in `zarr.json` would mis-describe the bytes on
//! the wire. The `bytes` codec is the v3 spec's identity codec and
//! lets `zarr-python` decode our chunks directly into a NumPy array.
//! Compression will be added when ingestion grows a codec pipeline.

use serde::{Deserialize, Serialize};

use crate::manifest::ArrayManifest;

/// A v3 array `zarr.json` document.
///
/// Field names mirror the [Zarr v3 specification][spec] and are
/// serialized as-is so the document is consumable by any compliant
/// v3 reader. We only model the subset needed for Phase 0 — future
/// fields (e.g. `dimension_names`, `storage_transformers`) can be
/// added with `#[serde(default)]` without breaking existing
/// manifests.
///
/// [spec]: https://zarr-specs.readthedocs.io/en/latest/v3/core/v3.0.html
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ZarrV3ArrayMeta {
    pub zarr_format: u32,
    pub node_type: String,
    pub shape: Vec<u64>,
    pub data_type: String,
    pub chunk_grid: ChunkGrid,
    pub chunk_key_encoding: ChunkKeyEncoding,
    pub fill_value: serde_json::Value,
    pub codecs: Vec<Codec>,
    #[serde(default)]
    pub attributes: serde_json::Map<String, serde_json::Value>,
}

/// Chunk-grid descriptor.
///
/// We only emit `name = "regular"` (uniform chunk shape across the
/// array). The v3 spec allows pluggable grids, but Phase 0's
/// `MmapChunkStore` assumes a regular grid when computing chunk
/// counts, so we keep the rendering aligned with the implementation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChunkGrid {
    pub name: String,
    pub configuration: ChunkGridConfig,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChunkGridConfig {
    pub chunk_shape: Vec<u64>,
}

/// How chunk indices are encoded in the URL path.
///
/// Phase 0 uses the v3 default (`name = "default"`, `separator = "/"`),
/// which produces keys like `c/0/0`. The HTTP route
/// `GET /datasets/{org}/{dataset}/c/{*chunk_path}` mirrors this
/// shape: the captured `chunk_path` is appended to the literal `c/`
/// prefix and looked up in the manifest verbatim. Choosing any other
/// encoding here would also require updating that route.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChunkKeyEncoding {
    pub name: String,
    pub configuration: ChunkKeyEncodingConfig,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChunkKeyEncodingConfig {
    pub separator: String,
}

/// A single codec in the v3 codec pipeline.
///
/// `configuration` is a freeform JSON object because each codec has
/// its own schema (the `bytes` codec needs `endian`, `gzip` needs
/// `level`, etc.). Carrying it as `serde_json::Value` keeps this
/// crate codec-agnostic; the renderer in [`ZarrV3ArrayMeta::from_manifest`]
/// is the only place that knows which codec the local store actually
/// implements. When more codecs land, prefer adding a typed builder
/// to that renderer rather than leaking codec types here.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Codec {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub configuration: Option<serde_json::Value>,
}

impl ZarrV3ArrayMeta {
    /// Produce a v3 array document from a manifest.
    ///
    /// The dtype string is forwarded directly; callers should supply a
    /// Zarr v3 dtype identifier (`"float32"`, `"float64"`, `"int32"`, …).
    ///
    /// # Defaults baked in
    ///
    /// - `fill_value = 0` — Phase 0 datasets are dense (every chunk
    ///   in range is written before being read), so the fill value
    ///   is effectively unused. Numeric `0` is valid for every
    ///   numeric dtype `zarr-python` ships with.
    /// - `codecs = [{ name: "bytes", endian: "little" }]` — raw
    ///   little-endian bytes, no compression. Matches what the
    ///   `MmapChunkStore` actually serves.
    /// - `chunk_key_encoding = { name: "default", separator: "/" }`
    ///   — see the [`ChunkKeyEncoding`] doc; tied to the HTTP route
    ///   shape.
    /// - `attributes = {}` — user attributes will be persisted via a
    ///   future governance-side `attributes` map; the manifest does
    ///   not yet carry them.
    pub fn from_manifest(manifest: &ArrayManifest) -> Self {
        Self {
            zarr_format: 3,
            node_type: "array".to_owned(),
            shape: manifest.shape.clone(),
            data_type: manifest.dtype.clone(),
            chunk_grid: ChunkGrid {
                name: "regular".to_owned(),
                configuration: ChunkGridConfig {
                    chunk_shape: manifest.chunk_shape.clone(),
                },
            },
            chunk_key_encoding: ChunkKeyEncoding {
                name: "default".to_owned(),
                configuration: ChunkKeyEncodingConfig {
                    separator: "/".to_owned(),
                },
            },
            fill_value: serde_json::Value::Number(0.into()),
            codecs: vec![Codec {
                name: "bytes".to_owned(),
                configuration: Some(serde_json::json!({ "endian": "little" })),
            }],
            attributes: serde_json::Map::new(),
        }
    }
}
