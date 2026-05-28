# ReactorOpenAPI

This directory is prepared for swift-openapi-generator output.

## Current Status

The Reactor API types are currently hand-written in `ReactorShared/Types.swift` because:

1. The OpenAPI spec at `sdks/js/openapi/spec.json` is minimal (~290 lines, auth-only)
2. Hand-written types provide better Swift ergonomics (Identifiable, Equatable, etc.)
3. The PostgREST data layer uses generic `Codable` types rather than fixed schemas

## Future Use

When the OpenAPI spec expands to cover more endpoints, run:

```bash
./scripts/generate-openapi.sh
```

This will generate types into `Sources/ReactorOpenAPI/Generated/`.

To check for drift in CI:

```bash
./scripts/generate-openapi.sh
git diff --exit-code Sources/ReactorOpenAPI/Generated/
```

## Dependencies

Install swift-openapi-generator:

```bash
brew install swift-openapi-generator
```

Or add as a Swift Package plugin.
