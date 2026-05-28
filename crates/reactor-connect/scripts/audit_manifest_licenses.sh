#!/bin/bash
# License Audit Script for Reactor Connect Manifests
# Verifies all YAML manifests have appropriate license headers

set -e

MANIFESTS_DIR="$(dirname "$0")/../manifests"
REQUIRED_LICENSE="License: MIT"
EXIT_CODE=0

echo "=== Reactor Connect Manifest License Audit ==="
echo ""

# Count manifests
MANIFEST_COUNT=$(find "$MANIFESTS_DIR" -name "*.yaml" | wc -l | tr -d ' ')
echo "Found $MANIFEST_COUNT YAML manifests"
echo ""

# Check each manifest
for manifest in "$MANIFESTS_DIR"/*.yaml; do
    if [ ! -f "$manifest" ]; then
        continue
    fi
    
    filename=$(basename "$manifest")
    
    # Check for license header in first 5 lines
    if head -5 "$manifest" | grep -q "$REQUIRED_LICENSE"; then
        echo "✓ $filename - License OK"
    else
        echo "✗ $filename - MISSING LICENSE HEADER"
        echo "  Expected: # $REQUIRED_LICENSE"
        EXIT_CODE=1
    fi
done

echo ""

# Summary
if [ $EXIT_CODE -eq 0 ]; then
    echo "=== All manifests have valid license headers ==="
else
    echo "=== Some manifests are missing license headers ==="
    echo ""
    echo "Please add the following header to each manifest:"
    echo "  # License: MIT (Airbyte Low-Code CDK compatible)"
fi

exit $EXIT_CODE
