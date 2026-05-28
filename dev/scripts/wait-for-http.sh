#!/usr/bin/env bash
# Wait for an HTTP endpoint to return 200 OK.
# Usage: wait-for-http.sh URL [MAX_ATTEMPTS] [SLEEP_SECS]

set -euo pipefail

URL="${1:?Usage: wait-for-http.sh URL [MAX_ATTEMPTS] [SLEEP_SECS]}"
MAX_ATTEMPTS="${2:-60}"
SLEEP_SECS="${3:-0.25}"

echo "Waiting for $URL ..."

for i in $(seq 1 "$MAX_ATTEMPTS"); do
    if curl -sf "$URL" > /dev/null 2>&1; then
        echo "OK (attempt $i)"
        exit 0
    fi
    sleep "$SLEEP_SECS"
done

echo "FAIL: $URL not available after $MAX_ATTEMPTS attempts"
exit 1
