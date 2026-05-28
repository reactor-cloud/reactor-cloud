import { pathToFileURL } from 'node:url';
import { Client } from './client.js';
import { runTestDefinition } from './harness.js';
import type { TestDefinition, TestResult } from './types.js';

export async function runTest(
  testPath: string,
  opts?: {
    workspacePath?: string;
    outputDir?: string;
  }
): Promise<TestResult> {
  // Import the test definition
  const testModule = await import(pathToFileURL(testPath).href);
  const definition: TestDefinition = testModule.default;

  if (!definition || typeof definition.run !== 'function') {
    throw new Error(`Invalid test definition in ${testPath}. Must export a defineTest() result.`);
  }

  // Create client
  const client = await Client.create({
    workspacePath: opts?.workspacePath,
  });

  // Run the test
  return runTestDefinition(definition, client, opts?.outputDir);
}
