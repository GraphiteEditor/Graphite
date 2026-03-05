#!/usr/bin/env bash
set -euo pipefail

IMAGE_NAME="graphite-github-actions-runner"
GITHUB_URL="https://github.com/GraphiteEditor/Graphite"

usage() {
    cat <<EOF
Usage: $0 [options]

Spawn self-hosted GitHub Actions runners for Graphite.

Required:
  -t TOKEN        GitHub runner registration token
  -p PREFIX       Runner name prefix

Optional:
  -n COUNT        Number of target/native runners to create (default: 1)
  -w COUNT        Number of target/wasm   runners to create (default: 1)
  -u URL          GitHub repository URL    (default: $GITHUB_URL)
  -i IMAGE        Container image name     (default: $IMAGE_NAME)
  -b              Build the image before starting runners
  -d              Tear down existing runners matching the prefix first
  -h              Show this help message

EOF
    exit 1
}

NATIVE_COUNT=1
WASM_COUNT=1
TOKEN=""
PREFIX=""
BUILD=false
TEARDOWN=false

while getopts "t:p:n:w:u:i:bdh" opt; do
    case $opt in
        t) TOKEN="$OPTARG" ;;
        p) PREFIX="$OPTARG" ;;
        n) NATIVE_COUNT="$OPTARG" ;;
        w) WASM_COUNT="$OPTARG" ;;
        u) GITHUB_URL="$OPTARG" ;;
        i) IMAGE_NAME="$OPTARG" ;;
        b) BUILD=true ;;
        d) TEARDOWN=true ;;
        h) usage ;;
        *) usage ;;
    esac
done

if [ -z "$PREFIX" ]; then
    echo "Error: -p PREFIX is required"
    usage
fi

teardown() {
    local pattern="$IMAGE_NAME-${PREFIX}-"
    echo "Tearing down containers matching: ${pattern}*"
    for cid in $(podman ps -a --filter "name=${pattern}" --format '{{.ID}}' 2>/dev/null); do
        name=$(podman inspect --format '{{.Name}}' "$cid")
        echo "  Stopping and removing $name"
        podman rm -f "$cid"
    done
}

if [ "$TEARDOWN" = true ]; then
    teardown
    if [ -z "$TOKEN" ]; then
        echo "Teardown complete."
        exit 0
    fi
fi

if [ -z "$TOKEN" ]; then
    echo "Error: -t TOKEN is required to create runners"
    usage
fi

if [ "$BUILD" = true ]; then
    echo "Building image: $IMAGE_NAME"
    SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
    podman build -t "$IMAGE_NAME" -f "$SCRIPT_DIR/containerfile" "$SCRIPT_DIR/.."
    echo ""
fi

start_runner() {
    local target="$1"   # native or wasm
    local index="$2"

    local name="${IMAGE_NAME}-${PREFIX}-${target}-${index}"
    local runner_name="${PREFIX}-${target}-${index}"
    local label="target/${target}"

    # Each runner gets its own sccache volume for isolation
    local cache_vol="${name}-cache"

    echo "Starting runner: $name (label: $label)"
    podman run -d \
        --name "$name" \
        --restart unless-stopped \
        -v "${cache_vol}:/var/lib/github-actions/.cache" \
        -e RUNNER_NAME="$runner_name" \
        -e RUNNER_LABELS="$label" \
        -e GITHUB_URL="$GITHUB_URL" \
        -e RUNNER_TOKEN="$TOKEN" \
        "$IMAGE_NAME"

    echo "  -> $name started (volume: $cache_vol)"
}

echo "Creating $NATIVE_COUNT native runner(s) and $WASM_COUNT wasm runner(s)"
echo ""

for i in $(seq 0 $((NATIVE_COUNT - 1))); do
    start_runner "native" "$i"
done

for i in $(seq 0 $((WASM_COUNT - 1))); do
    start_runner "wasm" "$i"
done

echo ""
echo "All runners started. Verify with: podman ps --filter name=$IMAGE_NAME"
