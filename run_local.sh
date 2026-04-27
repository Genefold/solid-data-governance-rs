#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# run_local.sh
#
# Run solid-contrib/specification-tests against a Solid server already
# listening on this machine, using Podman to execute the official
# solidproject/conformance-test-harness container image.
#
# Mirrors the approach of:
#   https://github.com/solid-contrib/specification-tests/blob/main/run.sh
# but replaces `docker` with `podman` and removes the CSS Docker setup
# (your server is already running).
#
# Prerequisites: podman, curl, git
#
# Usage:
#   ./run_local.sh [OPTIONS]
#
# Options:
#   -u <url>    Base URL of your server  (default: http://host.containers.internal:3000/)
#   -d <dir>    Path to an existing specification-tests checkout
#                 (default: auto-cloned into ./specification-tests/)
#   -t <file>   Tolerable-failures file
#   -h          Show this help
#
# Quick start (server already running on port 3000):
#   chmod +x run_local.sh && ./run_local.sh
# ---------------------------------------------------------------------------
set -euo pipefail

# ── defaults ───────────────────────────────────────────────────────────────
# host.containers.internal resolves to the host from inside a Podman container.
# If your server binds to 127.0.0.1 you may need --network=host instead;
# see the note at the bottom of this file.
BASE_URL="http://host.containers.internal:3000/"
SPEC_DIR=""
TOLERABLE_FILE=""

CTH_IMAGE="solidproject/conformance-test-harness"
CWD="$(pwd)"

log() { echo "==> $*"; }

usage() {
  sed -n '/^# Usage/,/^# Quick/p' "$0" | sed 's/^# \?//'
  exit 0
}

while getopts ":u:d:t:h" opt; do
  case $opt in
    u) BASE_URL="${OPTARG%/}/" ;;
    d) SPEC_DIR="$(cd "$OPTARG" && pwd)" ;;
    t) TOLERABLE_FILE="$(cd "$(dirname "$OPTARG")" && pwd)/$(basename "$OPTARG")" ;;
    h) usage ;;
    :) echo "ERROR: -$OPTARG requires an argument"; exit 1 ;;
   \?) echo "ERROR: unknown option -$OPTARG"; exit 1 ;;
  esac
done
shift $((OPTIND - 1))

# ── preflight ─────────────────────────────────────────────────────────────
for cmd in podman curl git; do
  command -v "$cmd" &>/dev/null || { echo "ERROR: '$cmd' not found on PATH"; exit 1; }
done

# ── server readiness probe ────────────────────────────────────────────────
# Probe via localhost (from the host), not via host.containers.internal.
PROBE_URL="${BASE_URL/host.containers.internal/localhost}"
log "Probing server at $PROBE_URL ..."
for i in $(seq 1 15); do
  curl --silent --fail --output /dev/null "$PROBE_URL" && break
  [[ $i -eq 15 ]] && { echo "ERROR: server not reachable at $PROBE_URL after 15 s"; exit 1; }
  sleep 1
done
log "Server is up."

# ── specification-tests checkout ──────────────────────────────────────────────
if [[ -z "$SPEC_DIR" ]]; then
  SPEC_DIR="$CWD/specification-tests"
  if [[ -d "$SPEC_DIR/.git" ]]; then
    log "Updating specification-tests ..."
    git -C "$SPEC_DIR" pull --quiet
  else
    log "Cloning specification-tests ..."
    git clone --depth=1 --quiet \
      https://github.com/solid-contrib/specification-tests.git \
      "$SPEC_DIR"
  fi
fi

# ── config directory ────────────────────────────────────────────────────────────
# application.yaml is read by CTH from /app/config inside the container.
CONFIG_DIR="$CWD/config"
mkdir -p "$CONFIG_DIR"

cat > "$CONFIG_DIR/application.yaml" <<YAML
subjects: /data/test-subjects.ttl
sources:
  - https://solidproject.org/TR/protocol
  - https://github.com/solid-contrib/specification-tests/blob/main/protocol/solid-protocol-test-manifest.ttl
  - https://github.com/solid-contrib/specification-tests/blob/main/protocol/requirement-comments.ttl
mappings:
  # Rewrite GitHub blob URLs to the local checkout mounted at /data
  - prefix: https://github.com/solid-contrib/specification-tests/blob/main
    path: /data
YAML

# test-subjects.ttl declares solid-community-rs as the test subject.
# No credentials — our server is currently unauthenticated.
cat > "$CONFIG_DIR/test-subjects.ttl" <<TTL
@prefix solid-test: <https://github.com/solid/conformance-test-harness/vocab#> .
@prefix earl:  <http://www.w3.org/ns/earl#> .
@prefix doap:  <http://usefulinc.com/ns/doap#> .
@prefix rdfs:  <http://www.w3.org/2000/01/rdf-schema#> .

<solid-community-rs>
    a earl:Software, earl:TestSubject ;
    doap:name "solid-community-rs" ;
    doap:description "A Rust implementation of the Solid protocol." ;
    doap:programming-language "Rust" ;
    doap:homepage <https://github.com/tuned-org-uk/solid-community-rs> ;
    solid-test:skip "acp", "wac", "authentication" ;
    solid-test:serverRoot <${BASE_URL}> .
TTL

# ── reports directory ──────────────────────────────────────────────────────────
REPORTS_DIR="$CWD/reports"
mkdir -p "$REPORTS_DIR"

# ── pull image ─────────────────────────────────────────────────────────────────
log "Pulling $CTH_IMAGE ..."
podman pull "$CTH_IMAGE"

# ── build podman args ──────────────────────────────────────────────────────────
PODMAN_ARGS=(
  --rm
  --interactive
  # Run as current user so report files are owned by you, not root.
  --user "$(id -u):$(id -g)"
  # specification-tests checkout mounted at /data (matches the mappings above).
  --volume "${SPEC_DIR}:/data:ro,z"
  # Config (application.yaml + test-subjects.ttl) mounted at /app/config.
  --volume "${CONFIG_DIR}:/app/config:ro,z"
  # Reports output.
  --volume "${REPORTS_DIR}:/reports:z"
  # Karate writes its own artefacts here.
  --volume "${CWD}/target:/app/target:z"
)

# host.containers.internal is available by default in Podman 4+.
# If your server binds only to 127.0.0.1 (not 0.0.0.0), add:
#   --network=host
# and change BASE_URL back to http://localhost:3000/

HARNESS_ARGS=(
  --output=/reports
  --target="${BASE_URL}"
  --skip-teardown
)

if [[ -n "$TOLERABLE_FILE" ]]; then
  mkdir -p "$CWD/target"
  cp "$TOLERABLE_FILE" "$CWD/target/tolerable-failures.txt"
  HARNESS_ARGS+=("--tolerable-failures=/app/target/tolerable-failures.txt")
fi

mkdir -p "$CWD/target"

# ── run ───────────────────────────────────────────────────────────────────────
log "Launching CTH container ..."
log "  image:   $CTH_IMAGE"
log "  server:  $BASE_URL"
log "  tests:   $SPEC_DIR"
log "  reports: $REPORTS_DIR"
echo ""
echo "Running: podman run ${PODMAN_ARGS[*]} $CTH_IMAGE ${HARNESS_ARGS[*]}"
echo ""

set +e
podman run "${PODMAN_ARGS[@]}" "$CTH_IMAGE" "${HARNESS_ARGS[@]}"
exit_code=$?
set -e

echo ""
if [[ $exit_code -eq 0 ]]; then
  log "All tests passed."
else
  log "Tests finished with failures (exit $exit_code) — see: $REPORTS_DIR"
fi

exit $exit_code
# ---------------------------------------------------------------------------
# NOTE — if the container cannot reach your server:
#
# Podman rootless uses a pasta/slirp4netns network where
# host.containers.internal resolves to the host IP.
# If that doesn’t work on your distro try:
#
#   ./run_local.sh --network=host -u http://localhost:3000/
#
# or add “--add-host=host.containers.internal:host-gateway” to PODMAN_ARGS.
# ---------------------------------------------------------------------------
