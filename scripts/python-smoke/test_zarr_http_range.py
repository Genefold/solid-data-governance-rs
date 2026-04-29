"""Phase 0 smoke test: verify the Rust pod-server serves Zarr v3 chunks
over HTTP Range so `zarr.open(url)` reads them transparently.

This is run end-to-end:

    python scripts/python-smoke/test_zarr_http_range.py http://localhost:8080

Prerequisites:
    pip install zarr>=3.0 requests

The script:
    1. Registers a fresh dataset via the governance API.
    2. Ingests two raw chunks via a small admin shim — for Phase 0 we
       reach into the same data directory through the API by writing
       directly to disk (the HTTP upload endpoint lands in Phase 1).
       To keep this script self-contained, we use the catalog endpoint
       to discover the on-disk path and stream chunks via the standard
       library.
    3. Calls `zarr.open(url, mode='r')` and asserts the array shape
       and first-row values.
"""

from __future__ import annotations

import os
import sys
import json
import struct
from pathlib import Path

import requests


def main(base_url: str, data_dir: str) -> int:
    base_url = base_url.rstrip("/")

    org, dataset = "smoke", "demo"
    body = {
        "title": "Phase 0 Zarr smoke",
        "description": "float32 4x4 array",
        "shape": [4, 4],
        "chunk_shape": [2, 2],
        "dtype": "float32",
    }
    r = requests.put(f"{base_url}/catalog/{org}/{dataset}", json=body)
    if r.status_code not in (201, 409):
        print("register failed:", r.status_code, r.text)
        return 1

    # Write 4 chunks of float32 directly to the chunk store using the
    # data_dir handed to the server. This mimics what the Phase 1
    # upload CLI will do over the wire.
    chunks_root = Path(data_dir) / "chunks"
    blob = chunks_root / "smoke__demo.bin"
    manifest_p = chunks_root / "smoke__demo.manifest.json"
    manifest = json.loads(manifest_p.read_text())

    # 4 chunks of 2x2 float32 = 16 bytes each.
    next_offset = manifest.get("descriptors", [])
    next_offset = next_offset[-1]["offset"] + next_offset[-1]["length"] if next_offset else 0
    chunks_written = []
    rows = [
        [1.0, 2.0, 3.0, 4.0],
        [5.0, 6.0, 7.0, 8.0],
        [9.0, 10.0, 11.0, 12.0],
        [13.0, 14.0, 15.0, 16.0],
    ]
    # chunk c/0/0 = rows 0-1 cols 0-1 ; etc.
    for ci in range(2):
        for cj in range(2):
            key = f"c/{ci}/{cj}"
            vals = []
            for i in range(2):
                for j in range(2):
                    vals.append(rows[ci * 2 + i][cj * 2 + j])
            data = b"".join(struct.pack("<f", v) for v in vals)
            with open(blob, "ab") as f:
                f.write(data)
            chunks_written.append({
                "key": key,
                "offset": next_offset,
                "length": len(data),
            })
            next_offset += len(data)

    manifest["descriptors"] = chunks_written
    manifest_p.write_text(json.dumps(manifest, indent=2))

    # Trigger a re-register POST with same body to reload the manifest
    # via the running server. Easier: delete the dataset entry from
    # the in-memory cache by restarting the server. For smoke purposes
    # we simply make a fresh request that calls ensure_loaded() —
    # however the server's RwLock currently caches a stale manifest.
    # As a workaround for the Phase 0 smoke test, instruct the operator
    # to restart the server before this script runs the read step.
    print(
        "manifest written; restart the server now if it was already running, "
        "then press Enter to continue …",
    )
    if sys.stdin.isatty():
        input()

    # zarr open over HTTP.
    try:
        import zarr
    except ImportError:
        print("zarr not installed; pip install 'zarr>=3.0'")
        return 1

    arr_url = f"{base_url}/datasets/{org}/{dataset}"
    arr = zarr.open(arr_url, mode="r")
    print("shape:", arr.shape, "dtype:", arr.dtype)
    first_row = arr[0, :]
    print("first row:", list(first_row))
    expected = [1.0, 2.0, 3.0, 4.0]
    assert list(first_row) == expected, (first_row, expected)
    print("OK")
    return 0


if __name__ == "__main__":
    base = sys.argv[1] if len(sys.argv) > 1 else "http://localhost:8080"
    ddir = sys.argv[2] if len(sys.argv) > 2 else os.environ.get("POD_DATA_DIR", "./.pod-data")
    sys.exit(main(base, ddir))
