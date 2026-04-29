//! Minimal Zarr v3 array-metadata serialization.
//!
//! Generates a `zarr.json` document that `zarr-python` 3.x recognises as a
//! valid v3 array. We deliberately keep this minimal — no codecs beyond
//! the default `bytes` codec, no shards — because the local-mmap blob
//! stores raw chunk bytes one-to-one.

use serde::{Deserialize, Serialize};

use crate::manifest::ArrayManifest;

/// A v3 array `zarr.json` document.
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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChunkGrid {
    pub name: String,
    pub configuration: ChunkGridConfig,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChunkGridConfig {
    pub chunk_shape: Vec<u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChunkKeyEncoding {
    pub name: String,
    pub configuration: ChunkKeyEncodingConfig,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChunkKeyEncodingConfig {
    pub separator: String,
}

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
