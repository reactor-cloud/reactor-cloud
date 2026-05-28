#!/usr/bin/env tsx
/**
 * OpenAPI Type Generation Script
 *
 * Fetches the OpenAPI spec from reactor-server and generates TypeScript types.
 *
 * Usage:
 *   pnpm openapi:generate                    # Uses default URL or env var
 *   REACTOR_OPENAPI_URL=http://... pnpm openapi:generate
 *
 * The script will:
 * 1. Fetch the merged OpenAPI spec from /_api/openapi.json
 * 2. Run openapi-typescript to generate TypeScript types
 * 3. Write the output to packages/shared/src/generated/api.d.ts
 * 4. Also save a copy of the spec to openapi/spec.json for CI drift detection
 */

import { execSync } from 'node:child_process';
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = join(__dirname, '..');

const OPENAPI_URL = process.env.REACTOR_OPENAPI_URL ?? 'http://localhost:8000/_api/openapi.json';
const OUTPUT_DIR = join(ROOT, 'packages/shared/src/generated');
const OUTPUT_FILE = join(OUTPUT_DIR, 'api.d.ts');
const SPEC_FILE = join(ROOT, 'openapi/spec.json');

async function main() {
  console.log(`Fetching OpenAPI spec from ${OPENAPI_URL}...`);

  let spec: string;

  try {
    const response = await fetch(OPENAPI_URL);
    if (!response.ok) {
      throw new Error(`HTTP ${response.status}: ${response.statusText}`);
    }
    spec = await response.text();
  } catch (error) {
    // If fetch fails, try to use the cached spec file
    if (existsSync(SPEC_FILE)) {
      console.log('Fetch failed, using cached spec file...');
      spec = readFileSync(SPEC_FILE, 'utf-8');
    } else {
      console.error('Failed to fetch OpenAPI spec and no cached spec available.');
      console.error('Make sure reactor-server is running or provide a cached spec.');
      console.error(`Error: ${error}`);
      process.exit(1);
    }
  }

  // Validate it's valid JSON
  try {
    JSON.parse(spec);
  } catch {
    console.error('Invalid JSON in OpenAPI spec');
    process.exit(1);
  }

  // Ensure directories exist
  if (!existsSync(OUTPUT_DIR)) {
    mkdirSync(OUTPUT_DIR, { recursive: true });
  }

  if (!existsSync(dirname(SPEC_FILE))) {
    mkdirSync(dirname(SPEC_FILE), { recursive: true });
  }

  // Save the spec for drift detection
  writeFileSync(SPEC_FILE, spec + '\n');
  console.log(`Saved spec to ${SPEC_FILE}`);

  // Write spec to temp file for openapi-typescript
  const tempSpec = join(ROOT, 'openapi/.temp-spec.json');
  writeFileSync(tempSpec, spec);

  // Run openapi-typescript
  console.log('Generating TypeScript types...');
  try {
    execSync(`npx openapi-typescript ${tempSpec} -o ${OUTPUT_FILE}`, {
      cwd: ROOT,
      stdio: 'inherit',
    });
  } catch {
    console.error('Failed to generate TypeScript types');
    process.exit(1);
  }

  // Clean up temp file
  try {
    const { unlinkSync } = await import('node:fs');
    unlinkSync(tempSpec);
  } catch {
    // Ignore cleanup errors
  }

  console.log(`Generated types at ${OUTPUT_FILE}`);
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
