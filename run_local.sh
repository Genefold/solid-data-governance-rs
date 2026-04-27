#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# run_local.sh  —  Run the Solid conformance-test-harness against a server
#                  already running on this machine (no Docker required).
#
# Mirrors the structure of:
#   https://github.com/solid-contrib/specification-tests/blob/main/run.sh
# but replaces every `docker run …` with a direct Java invocation of the
# CTH uber-jar, and replaces the CSS Docker setup with a readiness probe
# against a server you manage yourself.
#
# Usage:
#   ./run_local.sh [OPTIONS]
#
# Options:
#   -u <base-url>    Base URL of the server under test
#                    (default: http://localhost:3000/)
#   -d <testdir>     Path to a local checkout of specification-tests
#                    (default: auto-cloned into ./specification-tests)
#   -j <jar>         Path to the CTH uber-jar
#                    (default: auto-downloaded into ./bin/)
#   -o <outdir>      Directory for HTML/TTL reports  (default: ./reports)
#   -t <file>        Tolerable-failures file (passed to --tolerable-failures)
#   -r               Re-download the CTH jar even if it already exists
#   -v               Verbose: print every curl probe and harness command
#   -h               Show this help and exit
#
# Requirements:
#   bash 4+, curl, java 11+, git
#   The server under test must already be running before you call this script.
#
# Quick start:
#   cargo run -p cli &
#   sleep 2
#   ./run_local.sh
# ---------------------------------------------------------------------------

set -euo pipefail

# ── defaults ────────────────────────────────────────────────────────────────
BASE_URL="http://localhost:3000/"
TEST_DIR=""
JAR_PATH=""
OUT_DIR="$(pwd)/reports"
TOLERABLE_FILE=""
FORCE_DOWNLOAD=false
VERBOSE=false

# CTH release to fetch when no -j is given.
# Check https://github.com/solid/conformance-test-harness/releases for updates.
CTH_VERSION="2.0.1"
CTH_JAR_URL="https://github.com/solid/conformance-test-harness/releases/download/${CTH_VERSION}/solid-conformance-test-harness-${CTH_VERSION}.jar"

# The specification-tests repo to clone when no -d is given.
SPEC_TESTS_REPO="https://github.com/solid-contrib/specification-tests.git"

# ── helpers ─────────────────────────────────────────────────────────────────
log()  { echo "[run_local] $*"; }
vlog() { $VERBOSE && echo "[run_local:verbose] $*" || true; }
die()  { echo "[run_local] ERROR: $*" >&2; exit 1; }

usage() {
  grep '^#' "$0" | grep -v '^#!/' | sed 's/^# \?//'
  exit 0
}

# ── argument parsing ────────────────────────────────────────────────────────
while getopts ":u:d:j:o:t:rvh" opt; do
  case $opt in
    u) BASE_URL="$OPTARG" ;;
    d) TEST_DIR="$(cd "$OPTARG" && pwd)" ;;
    j) JAR_PATH="$(cd "$(dirname "$OPTARG")" && pwd)/$(basename "$OPTARG")" ;;
    o) OUT_DIR="$(mkdir -p "$OPTARG" && cd "$OPTARG" && pwd)" ;;
    t) TOLERABLE_FILE="$(cd "$(dirname "$OPTARG")" && pwd)/$(basename "$OPTARG")" ;;
    r) FORCE_DOWNLOAD=true ;;
    v) VERBOSE=true ;;
    h) usage ;;
    :) die "Option -$OPTARG requires an argument." ;;
    \?) die "Unknown option: -$OPTARG" ;;
  esac
done
shift $((OPTIND - 1))

# ── pre-flight checks ───────────────────────────────────────────────────────
for cmd in curl java git; do
  command -v "$cmd" &>/dev/null || die "'$cmd' not found on PATH. Please install it."
done

java_version=$(java -version 2>&1 | awk -F '"' '/version/ {print $2}' | cut -d. -f1)
if [[ "$java_version" -lt 11 ]]; then
  die "Java 11 or higher is required (found: $java_version)"
fi

# Strip trailing slash then add one — ensures BASE_URL always ends with /
BASE_URL="${BASE_URL%/}/"
log "Target server: $BASE_URL"

# ── wait for server ──────────────────────────────────────────────────────────
log "Checking server availability…"
max_wait=30
elapsed=0
until curl --silent --fail --output /dev/null "${BASE_URL}"; do
  if [[ $elapsed -ge $max_wait ]]; then
    die "Server at ${BASE_URL} did not respond within ${max_wait}s. Is it running?"
  fi
  vlog "  waiting… (${elapsed}s)"
  sleep 1
  (( elapsed++ )) || true
done
log "Server is up."

# ── ensure specification-tests checkout ─────────────────────────────────────
if [[ -z "$TEST_DIR" ]]; then
  TEST_DIR="$(pwd)/specification-tests"
  if [[ -d "$TEST_DIR/.git" ]]; then
    log "Updating existing specification-tests checkout…"
    git -C "$TEST_DIR" pull --quiet
  else
    log "Cloning specification-tests…"
    git clone --depth=1 --quiet "$SPEC_TESTS_REPO" "$TEST_DIR"
  fi
fi
[[ -d "$TEST_DIR" ]] || die "TEST_DIR does not exist: $TEST_DIR"
log "Test data:  $TEST_DIR"

# ── ensure CTH jar ──────────────────────────────────────────────────────────
if [[ -z "$JAR_PATH" ]]; then
  mkdir -p "$(pwd)/bin"
  JAR_PATH="$(pwd)/bin/solid-conformance-test-harness-${CTH_VERSION}.jar"
fi

if [[ ! -f "$JAR_PATH" ]] || $FORCE_DOWNLOAD; then
  log "Downloading CTH ${CTH_VERSION} → $JAR_PATH"
  curl --location --progress-bar "$CTH_JAR_URL" --output "$JAR_PATH" \
    || die "Failed to download CTH jar from $CTH_JAR_URL"
else
  log "Using cached CTH jar: $JAR_PATH"
fi

# ── build config/application.yaml ───────────────────────────────────────────
# The CTH reads application.yaml from its working directory.
# We generate one that points at the local test data and the live server.
CONFIG_DIR="$(pwd)/config"
mkdir -p "$CONFIG_DIR"

log "Writing config/application.yaml…"
cat > "$CONFIG_DIR/application.yaml" <<YAML
subjects: /data/test-subjects.ttl
sources:
  # Protocol spec & manifest
  - https://solidproject.org/TR/protocol
  - https://github.com/solid-contrib/specification-tests/blob/main/protocol/solid-protocol-test-manifest.ttl
  - https://github.com/solid-contrib/specification-tests/blob/main/protocol/requirement-comments.ttl

  # WAC spec & manifest
  - https://solidproject.org/TR/wac
  - https://github.com/solid-contrib/specification-tests/blob/main/web-access-control/web-access-control-test-manifest.ttl
  - https://github.com/solid-contrib/specification-tests/blob/main/web-access-control/requirement-comments.ttl

  - https://github.com/solid-contrib/specification-tests/blob/main/web-access-control/wac-spec-additions.ttl

mappings:
  # Map the GitHub blob URLs to the local checkout so no network fetch is needed.
  - prefix: https://github.com/solid-contrib/specification-tests/blob/main
    path: /data
YAML

# ── build test-subjects.ttl ─────────────────────────────────────────────────
# Defines this server as a test subject so CTH knows where to send requests.
# No authentication is configured — mirrors the unauthenticated profile used
# by our integration tests.  Add credentials here when auth is implemented.
cat > "$CONFIG_DIR/test-subjects.ttl" <<TTL
@prefix solid-test: <https://github.com/solid/conformance-test-harness/> .
@prefix earl:        <http://www.w3.org/ns/earl#> .
@prefix rdfs:        <http://www.w3.org/2000/01/rdf-schema#> .

<${BASE_URL}>
  a earl:TestSubject ;
  rdfs:label "solid-community-rs" ;
  solid-test:serverRoot <${BASE_URL}> ;
  solid-test:podOwner [
    solid-test:username  "" ;
    solid-test:password  "" ;
    solid-test:webid     "" ;
    solid-test:podRoot   <${BASE_URL}>
  ] .
TTL

# ── prepare output directory ────────────────────────────────────────────────
mkdir -p "$OUT_DIR"
log "Reports:    $OUT_DIR"

# ── build harness arguments ─────────────────────────────────────────────────
HARNESS_ARGS=(
  --output="$OUT_DIR"
  --subjects="$CONFIG_DIR/test-subjects.ttl"
  --source="$TEST_DIR"
  --target="${BASE_URL}"
  --skip-teardown
)

if [[ -n "$TOLERABLE_FILE" ]]; then
  HARNESS_ARGS+=("--tolerable-failures=$TOLERABLE_FILE")
fi

# Pass any extra arguments the user appended after the flags straight through.
if [[ $# -gt 0 ]]; then
  HARNESS_ARGS+=("$@")
fi

# ── run the harness ──────────────────────────────────────────────────────────
log "Launching CTH…"
vlog "  java -jar $JAR_PATH ${HARNESS_ARGS[*]}"
echo ""

set +e
java -jar "$JAR_PATH" "${HARNESS_ARGS[@]}"
exit_code=$?
set -e

echo ""
log "CTH finished with exit code: $exit_code"
if [[ "$exit_code" -eq 0 ]]; then
  log "All tests passed."
else
  log "Some tests failed or were skipped — see reports in: $OUT_DIR"
fi

exit "$exit_code"
