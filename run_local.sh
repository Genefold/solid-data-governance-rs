#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# run_local.sh
#
# Glue script: run solid-contrib/specification-tests against a Solid server
# already listening on this machine.
#
# The specification-tests use Karate .feature files that must be executed
# by the Conformance Test Harness (CTH) Java runner — this script downloads
# both automatically on first run, then re-uses cached copies.
#
# Prerequisites: java 11+, curl, git
#
# Usage:
#   ./run_local.sh [BASE_URL]
#
# Examples:
#   ./run_local.sh                        # default: http://localhost:3000/
#   ./run_local.sh http://localhost:8080/
#
# First run (downloads ~60 MB once, cached in ./bin/ and ./specification-tests/):
#   cargo run -p cli &
#   sleep 2 && ./run_local.sh
# ---------------------------------------------------------------------------
set -euo pipefail

BASE_URL="${1:-http://localhost:3000/}"
BASE_URL="${BASE_URL%/}/"   # ensure trailing slash

CTH_VERSION="2.0.1"
CTH_JAR="$(pwd)/bin/solid-conformance-test-harness-${CTH_VERSION}.jar"
CTH_URL="https://github.com/solid/conformance-test-harness/releases/download/${CTH_VERSION}/solid-conformance-test-harness-${CTH_VERSION}.jar"

SPEC_DIR="$(pwd)/specification-tests"
REPORTS_DIR="$(pwd)/reports"
CONFIG_DIR="$(pwd)/config"

log() { echo "==> $*"; }

# ─── 1. preflight ──────────────────────────────────────────────────────────
for cmd in java curl git; do
  command -v "$cmd" &>/dev/null || { echo "ERROR: '$cmd' not found on PATH"; exit 1; }
done

# ─── 2. server readiness probe ────────────────────────────────────────────────
log "Probing server at $BASE_URL ..."
for i in $(seq 1 15); do
  curl --silent --fail --output /dev/null "${BASE_URL}" && break
  [[ $i -eq 15 ]] && { echo "ERROR: server not reachable after 15 s"; exit 1; }
  sleep 1
done
log "Server is up."

# ─── 3. specification-tests checkout ───────────────────────────────────────────
if [[ -d "$SPEC_DIR/.git" ]]; then
  log "Updating specification-tests ..."
  git -C "$SPEC_DIR" pull --quiet
else
  log "Cloning specification-tests ..."
  git clone --depth=1 --quiet \
    https://github.com/solid-contrib/specification-tests.git \
    "$SPEC_DIR"
fi

# ─── 4. CTH jar ─────────────────────────────────────────────────────────────────
if [[ ! -f "$CTH_JAR" ]]; then
  log "Downloading CTH ${CTH_VERSION} ..."
  mkdir -p "$(dirname "$CTH_JAR")"
  curl --location --progress-bar "$CTH_URL" --output "$CTH_JAR"
else
  log "Using cached CTH jar: $CTH_JAR"
fi

# ─── 5. config: application.yaml + test-subjects.ttl ────────────────────────
mkdir -p "$CONFIG_DIR" "$REPORTS_DIR"

# application.yaml: tells CTH which specs and local test data to load.
# The mappings section rewrites GitHub blob URLs -> local checkout paths.
cat > "$CONFIG_DIR/application.yaml" <<YAML
subjects: ${CONFIG_DIR}/test-subjects.ttl
sources:
  - https://solidproject.org/TR/protocol
  - https://github.com/solid-contrib/specification-tests/blob/main/protocol/solid-protocol-test-manifest.ttl
  - https://github.com/solid-contrib/specification-tests/blob/main/protocol/requirement-comments.ttl
mappings:
  - prefix: https://github.com/solid-contrib/specification-tests/blob/main
    path: ${SPEC_DIR}
YAML

# test-subjects.ttl: declares this server as the test subject.
# No credentials — our server is currently unauthenticated.
cat > "$CONFIG_DIR/test-subjects.ttl" <<TTL
@prefix solid-test: <https://github.com/solid/conformance-test-harness/> .
@prefix earl:        <http://www.w3.org/ns/earl#> .
@prefix rdfs:        <http://www.w3.org/2000/01/rdf-schema#> .

<${BASE_URL}>
  a earl:TestSubject ;
  rdfs:label "solid-community-rs" ;
  solid-test:serverRoot <${BASE_URL}> ;
  solid-test:podOwner [
    solid-test:podRoot <${BASE_URL}>
  ] .
TTL

# ─── 6. run ────────────────────────────────────────────────────────────────────────
log "Starting CTH ..."
log "  server:  $BASE_URL"
log "  tests:   $SPEC_DIR"
log "  reports: $REPORTS_DIR"
echo ""

set +e
java -jar "$CTH_JAR" \
  --output="$REPORTS_DIR" \
  --config="$CONFIG_DIR/application.yaml" \
  --target="${BASE_URL}" \
  --skip-teardown
exit_code=$?
set -e

echo ""
if [[ $exit_code -eq 0 ]]; then
  log "All tests passed."
else
  log "Tests finished with failures (exit $exit_code)."
fi
log "Reports: $REPORTS_DIR"
exit $exit_code
