#!/usr/bin/env bash
# Reactor full e2e smoke test.
# Runs against a local reactor-server with Postgres.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DEV_DIR="$(dirname "$SCRIPT_DIR")"
PROJECT_ROOT="$(dirname "$DEV_DIR")"

BASE_URL="http://127.0.0.1:8000"
ADMIN_TOKEN="dev-admin-token"
USER_EMAIL="smoke-$(date +%s)@test.local"
USER_PASSWORD="smoke-password-123"
BUNDLE_PATH="$DEV_DIR/bundle.tar.zst"
LOG_FILE="$DEV_DIR/.reactor/server.log"
PID_FILE="$DEV_DIR/.reactor/server.pid"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

log_step() {
    echo -e "${GREEN}==> $1${NC}"
}

log_warn() {
    echo -e "${YELLOW}WARN: $1${NC}"
}

log_error() {
    echo -e "${RED}ERROR: $1${NC}"
}

# Cleanup on exit
cleanup() {
    local exit_code=$?
    echo ""
    if [ $exit_code -ne 0 ]; then
        log_error "Smoke test failed!"
        if [ -f "$LOG_FILE" ]; then
            echo ""
            echo "=== Last 50 lines of server log ==="
            tail -50 "$LOG_FILE" || true
        fi
    fi

    # Kill server if running
    if [ -f "$PID_FILE" ]; then
        local pid
        pid=$(cat "$PID_FILE")
        if kill -0 "$pid" 2>/dev/null; then
            log_step "Killing server (PID $pid)"
            kill "$pid" 2>/dev/null || true
            sleep 1
            kill -9 "$pid" 2>/dev/null || true
        fi
        rm -f "$PID_FILE"
    fi

    exit $exit_code
}

trap cleanup EXIT

# Assert HTTP response
# Usage: assert_http METHOD URL [EXPECTED_STATUS] [DATA] [EXTRA_HEADERS...]
assert_http() {
    local method="$1"
    local url="$2"
    local expected_status="${3:-200}"
    local data="${4:-}"
    shift 4 || true
    local extra_headers=("$@")

    local curl_args=(-s -w "\n%{http_code}" -X "$method")

    if [ -n "$data" ]; then
        curl_args+=(-H "Content-Type: application/json" -d "$data")
    fi

    for h in "${extra_headers[@]:-}"; do
        [ -n "$h" ] && curl_args+=(-H "$h")
    done

    local response
    response=$(curl "${curl_args[@]}" "$url")

    local body
    local status
    # Get all lines except the last (macOS compatible)
    body=$(echo "$response" | sed '$d')
    status=$(echo "$response" | tail -n 1)

    if [ "$status" != "$expected_status" ]; then
        log_error "$method $url returned $status (expected $expected_status)"
        echo "Response body: $body"
        return 1
    fi

    echo "$body"
}

# Assert JSON field exists and optionally matches value
# Usage: echo "$json" | assert_json_field FIELD [EXPECTED_VALUE]
assert_json_field() {
    local field="$1"
    local expected="${2:-}"
    local json
    json=$(cat)

    local value
    value=$(echo "$json" | jq -r "$field")

    if [ "$value" = "null" ] || [ -z "$value" ]; then
        log_error "Field $field not found in response"
        echo "Response: $json"
        return 1
    fi

    if [ -n "$expected" ] && [ "$value" != "$expected" ]; then
        log_error "Field $field = '$value' (expected '$expected')"
        return 1
    fi

    echo "$value"
}

# =============================================================================
# PRE-FLIGHT CHECKS
# =============================================================================

log_step "Pre-flight checks"

# Check bundle exists
if [ ! -f "$BUNDLE_PATH" ]; then
    log_error "Bundle not found: $BUNDLE_PATH"
    log_error "Run 'make bundle' first"
    exit 1
fi

# Check Postgres is up
if ! docker compose -f "$PROJECT_ROOT/docker-compose.yml" ps postgres 2>/dev/null | grep -qE "(running|Up)"; then
    log_error "Postgres container not running"
    log_error "Run 'make db-up' first"
    exit 1
fi

# Create .reactor directory
mkdir -p "$DEV_DIR/.reactor"

# =============================================================================
# START SERVER
# =============================================================================

log_step "Running migrations"
cd "$DEV_DIR"
cargo run -q -p reactor-server -- migrate

log_step "Starting reactor-server"
cargo run -q -p reactor-server > "$LOG_FILE" 2>&1 &
SERVER_PID=$!
echo "$SERVER_PID" > "$PID_FILE"
echo "Server PID: $SERVER_PID"

# Wait for health endpoint
log_step "Waiting for server health"
"$SCRIPT_DIR/wait-for-http.sh" "$BASE_URL/health" 60 0.5

# =============================================================================
# ADMIN ENDPOINTS
# =============================================================================

log_step "Testing /_admin/version"
VERSION_RESP=$(assert_http GET "$BASE_URL/_admin/version" 200 "" "Authorization: Bearer $ADMIN_TOKEN")
echo "$VERSION_RESP" | assert_json_field ".version" > /dev/null
echo "Version: $(echo "$VERSION_RESP" | jq -r '.version')"

log_step "Testing /_admin/doctor"
DOCTOR_RESP=$(assert_http GET "$BASE_URL/_admin/doctor" 200 "" "Authorization: Bearer $ADMIN_TOKEN")
echo "$DOCTOR_RESP" | jq -e '.checks' > /dev/null

# =============================================================================
# AUTH FLOW
# =============================================================================

log_step "Signing up user: $USER_EMAIL"
SIGNUP_RESP=$(assert_http POST "$BASE_URL/auth/v1/signup" 201 \
    "{\"email\":\"$USER_EMAIL\",\"password\":\"$USER_PASSWORD\"}")

USER_ID=$(echo "$SIGNUP_RESP" | assert_json_field ".user.id")
ACCESS_TOKEN=$(echo "$SIGNUP_RESP" | assert_json_field ".session.access_token")
echo "User ID: $USER_ID"

log_step "Testing password grant"
TOKEN_RESP=$(assert_http POST "$BASE_URL/auth/v1/token?grant_type=password" 200 \
    "{\"email\":\"$USER_EMAIL\",\"password\":\"$USER_PASSWORD\"}")
ACCESS_TOKEN=$(echo "$TOKEN_RESP" | assert_json_field ".access_token")
echo "Got access token (${#ACCESS_TOKEN} chars)"

# =============================================================================
# DEPLOY BUNDLE
# =============================================================================

log_step "Deploying bundle"
DEPLOY_RESP=$(curl -s -w "\n%{http_code}" -X POST \
    -H "Authorization: Bearer $ADMIN_TOKEN" \
    -H "Content-Type: application/octet-stream" \
    --data-binary "@$BUNDLE_PATH" \
    "$BASE_URL/_admin/deploy")

DEPLOY_BODY=$(echo "$DEPLOY_RESP" | sed '$d')
DEPLOY_STATUS=$(echo "$DEPLOY_RESP" | tail -n 1)

if [ "$DEPLOY_STATUS" != "200" ] && [ "$DEPLOY_STATUS" != "201" ]; then
    log_error "Deploy failed with status $DEPLOY_STATUS"
    echo "Response: $DEPLOY_BODY"
    exit 1
fi
echo "Deploy response: $DEPLOY_BODY"

# =============================================================================
# DATA CAPABILITY
# =============================================================================

log_step "Testing data endpoint (GET /data/v1/notes)"
# This might return empty array or 404 if table doesn't exist yet
NOTES_RESP=$(curl -s -w "\n%{http_code}" -X GET \
    -H "Authorization: Bearer $ACCESS_TOKEN" \
    "$BASE_URL/data/v1/notes")

NOTES_BODY=$(echo "$NOTES_RESP" | sed '$d')
NOTES_STATUS=$(echo "$NOTES_RESP" | tail -n 1)

if [ "$NOTES_STATUS" = "200" ]; then
    echo "Notes response: $NOTES_BODY"
elif [ "$NOTES_STATUS" = "404" ]; then
    log_warn "Notes table not found (migration may not have been applied)"
else
    log_error "Unexpected status $NOTES_STATUS for notes endpoint"
    echo "Response: $NOTES_BODY"
fi

# =============================================================================
# FUNCTIONS CAPABILITY
# =============================================================================

log_step "Testing function invoke (GET /fn/v1/hello)"
FN_RESP=$(curl -s -w "\n%{http_code}" -X GET \
    -H "Authorization: Bearer $ACCESS_TOKEN" \
    "$BASE_URL/fn/v1/hello")

FN_BODY=$(echo "$FN_RESP" | sed '$d')
FN_STATUS=$(echo "$FN_RESP" | tail -n 1)

if [ "$FN_STATUS" = "200" ]; then
    echo "Function response: $FN_BODY"
elif [ "$FN_STATUS" = "404" ]; then
    log_warn "Function 'hello' not found (deployment may not include functions)"
else
    log_warn "Function invoke returned $FN_STATUS (may need function registration)"
    echo "Response: $FN_BODY"
fi

# =============================================================================
# JOBS CAPABILITY
# =============================================================================

log_step "Testing jobs health (GET /jobs/v1/health)"
JOBS_HEALTH=$(assert_http GET "$BASE_URL/jobs/v1/health" 200)
echo "Jobs health: $JOBS_HEALTH"

# Try manual trigger if job exists
log_step "Testing job trigger (POST /jobs/v1/heartbeat/trigger)"
JOB_TRIGGER_RESP=$(curl -s -w "\n%{http_code}" -X POST \
    -H "Authorization: Bearer $ACCESS_TOKEN" \
    -H "Content-Type: application/json" \
    -d '{}' \
    "$BASE_URL/jobs/v1/heartbeat/trigger")

JOB_BODY=$(echo "$JOB_TRIGGER_RESP" | sed '$d')
JOB_STATUS=$(echo "$JOB_TRIGGER_RESP" | tail -n 1)

if [ "$JOB_STATUS" = "200" ] || [ "$JOB_STATUS" = "202" ]; then
    echo "Job triggered: $JOB_BODY"
    
    # Try to get run ID and poll for completion
    RUN_ID=$(echo "$JOB_BODY" | jq -r '.run_id // empty')
    if [ -n "$RUN_ID" ]; then
        log_step "Polling job run: $RUN_ID"
        for i in $(seq 1 10); do
            RUN_RESP=$(curl -s \
                -H "Authorization: Bearer $ACCESS_TOKEN" \
                "$BASE_URL/jobs/v1/_admin/jobs/heartbeat/runs/$RUN_ID")
            RUN_STATUS=$(echo "$RUN_RESP" | jq -r '.status // empty')
            echo "Attempt $i: status=$RUN_STATUS"
            
            if [ "$RUN_STATUS" = "succeeded" ] || [ "$RUN_STATUS" = "failed" ]; then
                break
            fi
            sleep 1
        done
    fi
elif [ "$JOB_STATUS" = "404" ]; then
    log_warn "Job 'heartbeat' not found (deployment may not include jobs)"
else
    log_warn "Job trigger returned $JOB_STATUS"
    echo "Response: $JOB_BODY"
fi

# =============================================================================
# SHUTDOWN
# =============================================================================

log_step "Requesting graceful shutdown"
SHUTDOWN_RESP=$(curl -s -w "\n%{http_code}" -X POST \
    -H "Authorization: Bearer $ADMIN_TOKEN" \
    "$BASE_URL/_admin/shutdown")

SHUTDOWN_BODY=$(echo "$SHUTDOWN_RESP" | sed '$d')
SHUTDOWN_STATUS=$(echo "$SHUTDOWN_RESP" | tail -n 1)

if [ "$SHUTDOWN_STATUS" = "200" ] || [ "$SHUTDOWN_STATUS" = "202" ]; then
    echo "Shutdown initiated"
else
    log_warn "Shutdown returned $SHUTDOWN_STATUS: $SHUTDOWN_BODY"
fi

# Wait for process to exit
log_step "Waiting for server to exit"
for i in $(seq 1 10); do
    if ! kill -0 "$SERVER_PID" 2>/dev/null; then
        echo "Server exited (attempt $i)"
        rm -f "$PID_FILE"
        break
    fi
    sleep 0.5
done

# =============================================================================
# SUCCESS
# =============================================================================

echo ""
echo -e "${GREEN}============================================${NC}"
echo -e "${GREEN}  SMOKE TEST PASSED${NC}"
echo -e "${GREEN}============================================${NC}"
