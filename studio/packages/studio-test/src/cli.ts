#!/usr/bin/env node

import { program } from 'commander';
import { spawn, ChildProcess } from 'node:child_process';
import * as fs from 'node:fs/promises';
import * as path from 'node:path';
import { Client } from './client.js';
import { runTest } from './test-runner.js';

program
  .name('studio-test')
  .description('Testing CLI for Reactor Studio')
  .version('0.1.0');

// start command
program
  .command('start')
  .description('Start Reactor Studio with devserver enabled')
  .option('-w, --workspace <path>', 'Workspace path')
  .option('-t, --timeout <seconds>', 'Timeout in seconds', '60')
  .action(async (opts) => {
    const workspacePath = opts.workspace ?? process.cwd();
    console.log(`Starting Reactor Studio with devserver...`);

    const studioRoot = path.resolve(workspacePath, '..', '..'); // Adjust based on where we are

    const env = {
      ...process.env,
      REACTOR_STUDIO_DEVSERVER: '1',
    };

    // Try to find the studio directory
    let studioDir = process.cwd();
    while (studioDir !== '/' && !await exists(path.join(studioDir, 'studio', 'apps', 'studio'))) {
      studioDir = path.dirname(studioDir);
    }
    studioDir = path.join(studioDir, 'studio');

    if (!await exists(studioDir)) {
      console.error('Could not find studio directory. Please run from within the Reactor repo.');
      process.exit(1);
    }

    console.log(`Studio directory: ${studioDir}`);
    console.log(`Starting with devserver enabled...`);

    const child = spawn('pnpm', ['tauri', 'dev'], {
      cwd: path.join(studioDir, 'apps', 'studio'),
      env,
      stdio: 'inherit',
      detached: true,
    });

    child.unref();

    console.log(`Started with PID ${child.pid}`);
    console.log(`Waiting for devserver to be ready...`);

    const timeoutMs = parseInt(opts.timeout) * 1000;
    const startTime = Date.now();

    while (Date.now() - startTime < timeoutMs) {
      try {
        const client = await Client.create({ workspacePath });
        const health = await client.health();
        console.log(`DevServer is ready: ${health.status}`);
        return;
      } catch {
        await sleep(1000);
      }
    }

    console.error('Timeout waiting for devserver to start');
    process.exit(1);
  });

// stop command
program
  .command('stop')
  .description('Stop Reactor Studio')
  .option('-f, --force', 'Force kill')
  .action(async (opts) => {
    console.log('Stopping Reactor Studio...');

    try {
      const discoveryPath = path.join(process.cwd(), '.reactor', 'dev-server.json');
      const content = await fs.readFile(discoveryPath, 'utf-8');
      const discovery = JSON.parse(content);

      if (discovery.pid) {
        process.kill(discovery.pid, opts.force ? 'SIGKILL' : 'SIGTERM');
        console.log(`Sent ${opts.force ? 'SIGKILL' : 'SIGTERM'} to PID ${discovery.pid}`);
      }
    } catch (e) {
      console.error('Could not stop: ', e);
      process.exit(1);
    }
  });

// restart command
program
  .command('restart')
  .description('Restart Reactor Studio')
  .option('-t, --timeout <seconds>', 'Timeout in seconds', '60')
  .action(async (opts) => {
    // First stop
    try {
      const discoveryPath = path.join(process.cwd(), '.reactor', 'dev-server.json');
      const content = await fs.readFile(discoveryPath, 'utf-8');
      const discovery = JSON.parse(content);
      if (discovery.pid) {
        process.kill(discovery.pid, 'SIGTERM');
        await sleep(2000);
      }
    } catch {
      // Ignore if not running
    }

    // Then start (reuse start logic)
    console.log('Restarting...');
    const child = spawn('studio-test', ['start', '--timeout', opts.timeout], {
      stdio: 'inherit',
    });
    child.on('exit', (code) => process.exit(code ?? 0));
  });

// health command
program
  .command('health')
  .description('Check devserver health')
  .option('-w, --workspace <path>', 'Workspace path')
  .action(async (opts) => {
    try {
      const client = await Client.create({ workspacePath: opts.workspace });
      const health = await client.health();
      console.log(JSON.stringify(health, null, 2));
    } catch (e) {
      console.error('Health check failed:', e);
      process.exit(1);
    }
  });

// screenshot command
program
  .command('screenshot')
  .description('Take a screenshot')
  .option('-o, --out <path>', 'Output path')
  .option('-w, --workspace <path>', 'Workspace path')
  .action(async (opts) => {
    try {
      const client = await Client.create({ workspacePath: opts.workspace });
      const result = await client.screenshot(opts.out);
      if (result.success) {
        console.log(`Screenshot saved to: ${result.path}`);
      } else {
        console.error(`Screenshot failed: ${result.error}`);
        process.exit(1);
      }
    } catch (e) {
      console.error('Screenshot failed:', e);
      process.exit(1);
    }
  });

// logs command
program
  .command('logs')
  .description('Get logs from devserver')
  .option('--cat <category>', 'Log category (agent|app)', 'agent')
  .option('--level <level>', 'Log level filter')
  .option('--limit <n>', 'Limit number of entries', '50')
  .option('--since <timestamp>', 'Only show logs since timestamp')
  .option('-w, --workspace <path>', 'Workspace path')
  .action(async (opts) => {
    try {
      const client = await Client.create({ workspacePath: opts.workspace });
      const logs = await client.getLogs({
        cat: opts.cat as 'agent' | 'app',
        level: opts.level,
        limit: parseInt(opts.limit),
        since: opts.since,
      });

      for (const entry of logs.entries) {
        console.log(
          `[${entry.timestamp}] [${entry.level}] [${entry.category}] ${entry.event}`,
          entry.data ? JSON.stringify(entry.data) : ''
        );
      }

      if (logs.truncated) {
        console.log(`... (${logs.total} total entries, truncated)`);
      }
    } catch (e) {
      console.error('Failed to get logs:', e);
      process.exit(1);
    }
  });

// run command
program
  .command('run <script>')
  .description('Run a test script')
  .option('-w, --workspace <path>', 'Workspace path')
  .option('-o, --output <dir>', 'Output directory', 'test-results')
  .action(async (script, opts) => {
    const scriptPath = path.resolve(script);

    if (!await exists(scriptPath)) {
      console.error(`Test script not found: ${scriptPath}`);
      process.exit(1);
    }

    try {
      const result = await runTest(scriptPath, {
        workspacePath: opts.workspace,
        outputDir: opts.output,
      });

      process.exit(result.success ? 0 : 1);
    } catch (e) {
      console.error('Test failed:', e);
      process.exit(1);
    }
  });

// scaffold command
program
  .command('scaffold <name>')
  .description('Create a new test script')
  .option('-d, --dir <dir>', 'Directory to create test in', 'src/scripts')
  .action(async (name, opts) => {
    const fileName = name.endsWith('.ts') ? name : `${name}.ts`;
    const filePath = path.join(opts.dir, fileName);

    await fs.mkdir(opts.dir, { recursive: true });

    const template = `import { defineTest } from '@reactor-studio/test';

export default defineTest({
  name: '${name.replace('.ts', '')}',
  description: 'TODO: Add description',

  setup: {
    agent: 'coder',
    // workspace: '/path/to/workspace',
    // model: { provider: 'openrouter', modelId: 'anthropic/claude-sonnet-4' },
  },

  async run(s) {
    // Send a message and wait for response
    const result = await s.sendAndWait('Hello!', { timeoutMs: 60_000 });

    // Make assertions
    s.assert('got_response', result.success, true, result.success);
    s.assert('has_text', !!result.finalText, true, !!result.finalText);

    // Take screenshot
    await s.screenshot('final');

    return {
      data: {
        toolCount: result.toolSequence.length,
        durationMs: result.durationMs,
      },
    };
  },
});
`;

    await fs.writeFile(filePath, template);
    console.log(`Created test script: ${filePath}`);
  });

// Utilities
async function exists(p: string): Promise<boolean> {
  try {
    await fs.access(p);
    return true;
  } catch {
    return false;
  }
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

program.parse();
