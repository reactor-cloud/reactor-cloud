#!/bin/bash
# Generate Swift types from OpenAPI spec using swift-openapi-generator
#
# Prerequisites:
#   brew install swift-openapi-generator
#   # or: swift package plugin install swift-openapi-generator
#
# Usage:
#   ./scripts/generate-openapi.sh
#
# The generated types go into Sources/ReactorOpenAPI/Generated/
# Run `git diff --exit-code Sources/ReactorOpenAPI/Generated/` in CI to detect drift.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SDK_DIR="$(dirname "$SCRIPT_DIR")"
REPO_ROOT="$(dirname "$(dirname "$SDK_DIR")")"
SPEC_FILE="$REPO_ROOT/sdks/js/openapi/spec.json"
OUTPUT_DIR="$SDK_DIR/Sources/ReactorOpenAPI/Generated"

# Check if spec exists
if [ ! -f "$SPEC_FILE" ]; then
    echo "Error: OpenAPI spec not found at $SPEC_FILE"
    exit 1
fi

# Create output directory
mkdir -p "$OUTPUT_DIR"

# Check if swift-openapi-generator is available
if command -v swift-openapi-generator &> /dev/null; then
    echo "Generating Swift types from $SPEC_FILE..."
    swift-openapi-generator generate \
        --mode types \
        --output-directory "$OUTPUT_DIR" \
        "$SPEC_FILE"
    echo "Generated types in $OUTPUT_DIR"
else
    echo "swift-openapi-generator not found."
    echo "Install with: brew install swift-openapi-generator"
    echo ""
    echo "For now, types are hand-written in Sources/ReactorShared/Types.swift"
    echo "This script will be used when the OpenAPI spec expands."
    exit 0
fi

# Format generated code
if command -v swift-format &> /dev/null; then
    swift-format --in-place --recursive "$OUTPUT_DIR"
fi

echo "Done."
