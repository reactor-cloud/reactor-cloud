import * as fs from 'node:fs/promises';
import * as path from 'node:path';
import { Client } from './client.js';
import type {
  TestDefinition,
  TestSession,
  TestAssertion,
  TestLog,
  TestResult,
  WaitResponseResult,
} from './types.js';

export function defineTest(definition: TestDefinition): TestDefinition {
  return definition;
}

export class TestSessionImpl implements TestSession {
  conversationId: string;
  agentId: string;
  model: { provider: string; modelId: string } | null;
  workspacePath: string;
  client: Client;

  private logs: TestLog[] = [];
  private assertions: TestAssertion[] = [];
  private screenshots: string[] = [];

  constructor(
    client: Client,
    workspacePath: string,
    conversationId: string,
    agentId: string,
    model: { provider: string; modelId: string } | null = null
  ) {
    this.client = client;
    this.workspacePath = workspacePath;
    this.conversationId = conversationId;
    this.agentId = agentId;
    this.model = model;
  }

  log(msg: string, data?: Record<string, unknown>): void {
    const entry: TestLog = {
      timestamp: new Date().toISOString(),
      level: 'info',
      message: msg,
      data,
    };
    this.logs.push(entry);
    console.log(`[${entry.timestamp}] ${msg}`, data ? JSON.stringify(data) : '');
  }

  async screenshot(name?: string): Promise<string | null> {
    const result = await this.client.screenshot();
    if (result.success && result.path) {
      this.screenshots.push(result.path);
      this.log(`Screenshot taken: ${name ?? 'unnamed'}`, { path: result.path });
      return result.path;
    }
    this.log(`Screenshot failed: ${result.error}`);
    return null;
  }

  async sendAndWait(
    prompt: string,
    opts?: {
      timeoutMs?: number;
      onUpdate?: (messageCount: number) => void;
    }
  ): Promise<WaitResponseResult> {
    this.log(`Sending message: ${prompt.slice(0, 100)}...`);

    const sendResult = await this.client.sendMessage(this.conversationId, prompt);
    if (!sendResult.success) {
      return {
        success: false,
        error: sendResult.error ?? 'Failed to send message',
        toolSequence: [],
        durationMs: 0,
      };
    }

    const result = await this.client.waitResponse({
      timeoutMs: opts?.timeoutMs ?? 120_000,
      onUpdate: opts?.onUpdate ? (data) => opts.onUpdate!(data as number) : undefined,
    });

    this.log(`Response received`, {
      success: result.success,
      toolCount: result.toolSequence.length,
      durationMs: result.durationMs,
    });

    return result;
  }

  assert(name: string, passed: boolean, expected?: unknown, actual?: unknown): void {
    const assertion: TestAssertion = {
      name,
      passed,
      expected,
      actual,
    };
    this.assertions.push(assertion);

    if (passed) {
      console.log(`  ✓ ${name}`);
    } else {
      console.log(`  ✗ ${name}`);
      if (expected !== undefined) {
        console.log(`    Expected: ${JSON.stringify(expected)}`);
      }
      if (actual !== undefined) {
        console.log(`    Actual: ${JSON.stringify(actual)}`);
      }
    }
  }

  getLogs(): TestLog[] {
    return this.logs;
  }

  getAssertions(): TestAssertion[] {
    return this.assertions;
  }

  getScreenshots(): string[] {
    return this.screenshots;
  }

  allAssertionsPassed(): boolean {
    return this.assertions.every((a) => a.passed);
  }
}

export async function bootstrapTest(
  definition: TestDefinition,
  client: Client
): Promise<TestSessionImpl> {
  const setup = definition.setup ?? {};

  // Get or use default workspace
  const workspacePath = setup.workspace ?? client.getWorkspacePath() ?? process.cwd();

  // Open workspace if needed
  const currentState = await client.getState();
  if (!currentState.workspace || currentState.workspace.path !== workspacePath) {
    console.log(`Opening workspace: ${workspacePath}`);
    const result = await client.openWorkspace(workspacePath);
    if (!result.success) {
      throw new Error(`Failed to open workspace: ${result.error}`);
    }
  }

  // Select agent
  const agentId = setup.agent ?? 'coder';
  console.log(`Selecting agent: ${agentId}`);
  await client.selectAgent(agentId);

  // Create new conversation
  console.log(`Creating new conversation...`);
  const convResult = await client.newConversation(agentId);
  if (!convResult.success || !convResult.conversationId) {
    throw new Error(`Failed to create conversation: ${convResult.error}`);
  }

  const session = new TestSessionImpl(
    client,
    workspacePath,
    convResult.conversationId,
    agentId,
    setup.model ?? null
  );

  // Take bootstrap screenshot
  await session.screenshot('bootstrap');

  return session;
}

export async function runTestDefinition(
  definition: TestDefinition,
  client: Client,
  outputDir: string = 'test-results'
): Promise<TestResult> {
  const testRunId = `${definition.name}-${Date.now()}`;
  const startTime = Date.now();

  console.log(`\n${'='.repeat(60)}`);
  console.log(`Running test: ${definition.name}`);
  if (definition.description) {
    console.log(`Description: ${definition.description}`);
  }
  console.log(`${'='.repeat(60)}\n`);

  let session: TestSessionImpl | null = null;
  let userData: Record<string, unknown> | undefined;
  let error: string | undefined;

  try {
    session = await bootstrapTest(definition, client);
    const result = await definition.run(session);
    userData = result.data;
  } catch (e) {
    error = e instanceof Error ? e.message : String(e);
    console.error(`\nTest error: ${error}`);
  }

  const duration = Date.now() - startTime;
  const success = session?.allAssertionsPassed() ?? false;

  const result: TestResult = {
    testRunId,
    testName: definition.name,
    success: success && !error,
    duration,
    assertions: session?.getAssertions() ?? [],
    logs: session?.getLogs() ?? [],
    screenshots: session?.getScreenshots() ?? [],
    data: userData,
    error,
  };

  // Write result to file
  await fs.mkdir(outputDir, { recursive: true });
  const resultPath = path.join(outputDir, `${testRunId}-result.json`);
  await fs.writeFile(resultPath, JSON.stringify(result, null, 2));

  // Print summary
  console.log(`\n${'='.repeat(60)}`);
  console.log(`Test ${result.success ? 'PASSED' : 'FAILED'}: ${definition.name}`);
  console.log(`Duration: ${duration}ms`);
  console.log(`Assertions: ${result.assertions.filter((a) => a.passed).length}/${result.assertions.length} passed`);
  console.log(`Result saved to: ${resultPath}`);
  console.log(`${'='.repeat(60)}\n`);

  return result;
}
