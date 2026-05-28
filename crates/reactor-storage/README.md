# reactor-storage

S3-shaped HTTP surface with dual FS/S3 backends for blob storage.

## Overview

`reactor-storage` is the storage capability for Reactor.cloud, providing:

- **S3-shaped HTTP API** for blob storage operations
- **Dual backend support**: Local filesystem (FS) and S3-compatible storage
- **Named buckets** per organization with public/private visibility
- **Policy-based access control** using the shared `reactor-policy` engine
- **Multipart uploads** for large files
- **Signed URLs** for temporary authenticated access

## Features

### Cargo Features

- `fs` (default): Enable local filesystem storage backend
- `s3`: Enable S3-compatible storage backend (requires AWS SDK)

### HTTP Endpoints

All endpoints are prefixed with `/storage/v1`.

#### Health & Metrics
- `GET /health` - Health check
- `GET /metrics` - Prometheus metrics (when enabled)

#### Buckets
- `POST /buckets` - Create bucket
- `GET /buckets` - List buckets
- `GET /buckets/:bucket` - Get bucket details
- `PATCH /buckets/:bucket` - Update bucket
- `DELETE /buckets/:bucket` - Delete bucket

#### Objects
- `PUT /object/:bucket/:key` - Upload object
- `GET /object/:bucket/:key` - Download object (supports Range header)
- `HEAD /object/:bucket/:key` - Get object metadata
- `DELETE /object/:bucket/:key` - Delete object

#### Multipart Uploads
- `POST /upload/:bucket/:key` - Initiate multipart upload
- `PUT /upload/:bucket/:key/part?uploadId=X&partNumber=N` - Upload part
- `POST /upload/:bucket/:key/complete?uploadId=X` - Complete upload
- `DELETE /upload/:bucket/:key/abort?uploadId=X` - Abort upload

#### Signed URLs
- `POST /sign/:bucket/:key` - Generate signed URL

## Configuration

Environment variables:

| Variable | Description | Required |
|----------|-------------|----------|
| `DATABASE_URL` | PostgreSQL connection URL for metadata | Yes |
| `STORAGE_BIND` | HTTP server bind address (default: `0.0.0.0:8082`) | No |
| `DEPLOYMENT_MODE` | `monolith` or `microservices` | No |
| `AUTH_DATABASE_URL` | Auth DB URL (monolith mode) | When monolith |
| `AUTH_URL` | Auth service URL (microservices mode) | When microservices |
| `FS_BASE_PATH` | Base path for FS storage | When using FS |
| `S3_BUCKET` | S3 bucket name | When using S3 |
| `S3_REGION` | S3 region | When using S3 |
| `S3_ENDPOINT` | S3 endpoint (for MinIO, LocalStack) | Optional |
| `SIGNING_SECRET` | Secret for HMAC-based signed URLs | Recommended |
| `MAX_UPLOAD_SIZE` | Maximum upload size in bytes | No (default: 50MB) |
| `METRICS` | Enable metrics endpoint (`true`/`false`) | No |
| `LOG_LEVEL` | Log level (`trace`/`debug`/`info`/`warn`/`error`) | No |

## Database Schema

The storage metadata is stored in the `_reactor_storage` schema:

- `buckets` - Bucket definitions
- `objects` - Object metadata
- `policies` - Bucket-level access policies
- `multipart_uploads` - In-progress multipart uploads
- `multipart_parts` - Parts for multipart uploads
- `audit_events` - Audit log

## Integration

### With reactor-auth

`reactor-storage` integrates with `reactor-auth` for authentication:

- **Monolith mode**: Direct database access to auth tables
- **Microservices mode**: HTTP calls to auth service for token verification

### With reactor-policy

Uses the shared `reactor-policy` crate for:
- Policy expression parsing
- Auth context evaluation
- Storage-specific domain builtins (`object.key`, `bucket.name`, etc.)

## Example Usage

```bash
# Create a bucket
curl -X POST http://localhost:8082/storage/v1/buckets \
  -H "Authorization: Bearer $TOKEN" \
  -H "X-Reactor-Org: myorg" \
  -H "Content-Type: application/json" \
  -d '{"slug": "uploads", "is_public": false}'

# Upload an object
curl -X PUT http://localhost:8082/storage/v1/object/uploads/hello.txt \
  -H "Authorization: Bearer $TOKEN" \
  -H "X-Reactor-Org: myorg" \
  -H "Content-Type: text/plain" \
  -d "Hello, World!"

# Download an object
curl http://localhost:8082/storage/v1/object/uploads/hello.txt \
  -H "Authorization: Bearer $TOKEN" \
  -H "X-Reactor-Org: myorg"

# Generate signed URL
curl -X POST http://localhost:8082/storage/v1/sign/uploads/hello.txt \
  -H "Authorization: Bearer $TOKEN" \
  -H "X-Reactor-Org: myorg" \
  -H "Content-Type: application/json" \
  -d '{"expires_in": 3600}'
```

## License

See repository root for license information.
