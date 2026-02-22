/**
 * macOS Control — host-side handler for PrivateFrameworks access.
 *
 * Processes IPC requests from container agents, dispatching to native binaries:
 *   - swift-section: DYLD cache parsing, Swift interface generation
 *   - header-index: API extraction, signature inference, shim generation
 *   - app-automation: Accessibility-based UI automation
 *   - swiftc/python3: compile-and-run code execution
 */

import { spawn, execSync, ChildProcess } from 'child_process';
import fs from 'fs';
import path from 'path';
import crypto from 'crypto';

import { DATA_DIR } from './config.js';
import { logger } from './logger.js';

const NATIVE_BIN = path.join(DATA_DIR, '..', 'native', 'bin');
const EXEC_TIMEOUT = 30_000;

// Track background processes spawned by execute_swift/execute_python
const backgroundProcesses = new Map<string, ChildProcess>();

export interface MacOSControlRequest {
  id: string;
  command: string;
  args: Record<string, unknown>;
}

export interface MacOSControlResponse {
  id: string;
  success: boolean;
  output?: string;
  error?: string;
  duration_ms: number;
}

function bin(name: string): string {
  return path.join(NATIVE_BIN, name);
}

function runBinary(
  name: string,
  args: string[],
  timeout = EXEC_TIMEOUT,
): Promise<{ stdout: string; stderr: string; code: number }> {
  return new Promise((resolve) => {
    const start = Date.now();
    const proc = spawn(bin(name), args, { timeout });
    let stdout = '';
    let stderr = '';
    proc.stdout.on('data', (d) => (stdout += d));
    proc.stderr.on('data', (d) => (stderr += d));
    proc.on('close', (code) => resolve({ stdout, stderr, code: code ?? 1 }));
    proc.on('error', (err) =>
      resolve({ stdout, stderr: err.message, code: 1 }),
    );
  });
}

// --- Command handlers ---

async function discoverFramework(
  args: Record<string, unknown>,
): Promise<MacOSControlResponse & { id: string }> {
  const name = args.framework as string;
  const start = Date.now();

  // Run both tools in parallel for richer results
  // header-index: C/ObjC/C++/Swift symbol extraction + signature inference
  // swift-section: Swift type/protocol interface generation from dyld shared cache
  const [headerResult, swiftResult] = await Promise.all([
    runBinary('header-index', ['extract', name]),
    runBinary(
      'swift-section',
      ['interface', '--uses-system-dyld-shared-cache', '-n', name],
      60_000,
    ).catch(() => ({
      stdout: '',
      stderr: 'swift-section not available',
      code: 1,
    })),
  ]);

  const output =
    headerResult.code === 0
      ? `## header-index (C/ObjC exports + inferred signatures)\n${headerResult.stdout}\n\n## swift-section (Swift interface)\n${swiftResult.stdout.slice(0, 10000)}`
      : swiftResult.stdout || headerResult.stderr;

  return {
    id: '',
    success: headerResult.code === 0 || swiftResult.code === 0,
    output,
    duration_ms: Date.now() - start,
  };
}

async function queryApis(
  args: Record<string, unknown>,
): Promise<MacOSControlResponse & { id: string }> {
  const start = Date.now();
  const query = args.query as string;
  const r = await runBinary('header-index', ['query', query]);
  return {
    id: '',
    success: r.code === 0,
    output: r.stdout,
    error: r.stderr || undefined,
    duration_ms: Date.now() - start,
  };
}

async function swiftInterface(
  args: Record<string, unknown>,
): Promise<MacOSControlResponse & { id: string }> {
  const start = Date.now();
  const framework = args.framework as string;
  const sectionArgs = [
    'interface',
    '--uses-system-dyld-shared-cache',
    '-n',
    framework,
  ];
  if (args.color === false) sectionArgs.push('--color-scheme', 'none');
  const r = await runBinary('swift-section', sectionArgs, 60_000);
  return {
    id: '',
    success: r.code === 0,
    output: r.stdout,
    error: r.stderr || undefined,
    duration_ms: Date.now() - start,
  };
}

async function executeCode(
  args: Record<string, unknown>,
): Promise<MacOSControlResponse & { id: string }> {
  const start = Date.now();
  const code = args.code as string;
  const language = (args.language as string) || 'swift';
  const frameworks = (args.frameworks as string[]) || [];
  const privateFrameworks = (args.privateFrameworks as string[]) || [];
  const background = (args.background as boolean) || false;
  const timeout = (args.timeout as number) || EXEC_TIMEOUT;

  const hash = crypto
    .createHash('sha256')
    .update(code)
    .digest('hex')
    .slice(0, 12);
  const tmpDir = path.join(DATA_DIR, 'exec-tmp');
  fs.mkdirSync(tmpDir, { recursive: true });

  if (language === 'swift') {
    const srcFile = path.join(tmpDir, `exec-${hash}.swift`);
    const binFile = path.join(tmpDir, `exec-${hash}`);
    fs.writeFileSync(srcFile, code);

    // Compile
    const compileArgs = [srcFile, '-o', binFile, '-O'];
    for (const fw of frameworks) compileArgs.push('-framework', fw);
    for (const fw of privateFrameworks) {
      compileArgs.push(
        '-framework',
        fw,
        '-F',
        '/System/Library/PrivateFrameworks',
      );
    }
    // Always link AppKit for GUI operations
    if (
      !frameworks.includes('AppKit') &&
      !privateFrameworks.includes('AppKit')
    ) {
      compileArgs.push('-framework', 'AppKit');
    }

    try {
      execSync(`swiftc ${compileArgs.map((a) => `"${a}"`).join(' ')}`, {
        timeout: 60_000,
        stdio: 'pipe',
      });
    } catch (err: unknown) {
      const e = err as { stderr?: Buffer };
      return {
        id: '',
        success: false,
        error: `Compilation failed:\n${e.stderr?.toString() || String(err)}`,
        duration_ms: Date.now() - start,
      };
    }

    // Run
    if (background) {
      const proc = spawn(binFile, [], { detached: true, stdio: 'ignore' });
      proc.unref();
      const pid = proc.pid!;
      backgroundProcesses.set(hash, proc);
      return {
        id: '',
        success: true,
        output: `Background process started (pid: ${pid}, id: ${hash})`,
        duration_ms: Date.now() - start,
      };
    }

    const r = await new Promise<{
      stdout: string;
      stderr: string;
      code: number;
    }>((resolve) => {
      const proc = spawn(binFile, [], { timeout });
      let stdout = '';
      let stderr = '';
      proc.stdout.on('data', (d) => (stdout += d));
      proc.stderr.on('data', (d) => (stderr += d));
      proc.on('close', (code) => resolve({ stdout, stderr, code: code ?? 1 }));
      proc.on('error', (err) =>
        resolve({ stdout, stderr: err.message, code: 1 }),
      );
    });
    return {
      id: '',
      success: r.code === 0,
      output: r.stdout,
      error: r.stderr || undefined,
      duration_ms: Date.now() - start,
    };
  } else if (language === 'python') {
    const srcFile = path.join(tmpDir, `exec-${hash}.py`);
    fs.writeFileSync(srcFile, code);

    if (background) {
      const proc = spawn('python3', [srcFile], {
        detached: true,
        stdio: 'ignore',
      });
      proc.unref();
      backgroundProcesses.set(hash, proc);
      return {
        id: '',
        success: true,
        output: `Background process started (pid: ${proc.pid}, id: ${hash})`,
        duration_ms: Date.now() - start,
      };
    }

    const r = await new Promise<{
      stdout: string;
      stderr: string;
      code: number;
    }>((resolve) => {
      const proc = spawn('python3', [srcFile], { timeout });
      let stdout = '';
      let stderr = '';
      proc.stdout.on('data', (d) => (stdout += d));
      proc.stderr.on('data', (d) => (stderr += d));
      proc.on('close', (code) => resolve({ stdout, stderr, code: code ?? 1 }));
      proc.on('error', (err) =>
        resolve({ stdout, stderr: err.message, code: 1 }),
      );
    });
    return {
      id: '',
      success: r.code === 0,
      output: r.stdout,
      error: r.stderr || undefined,
      duration_ms: Date.now() - start,
    };
  }

  return {
    id: '',
    success: false,
    error: `Unsupported language: ${language}`,
    duration_ms: Date.now() - start,
  };
}

async function observeUi(
  args: Record<string, unknown>,
): Promise<MacOSControlResponse & { id: string }> {
  const start = Date.now();
  const r = await runBinary('app-automation', [
    'observe',
    args.appName as string,
  ]);
  return {
    id: '',
    success: r.code === 0,
    output: r.stdout,
    error: r.stderr || undefined,
    duration_ms: Date.now() - start,
  };
}

async function performAction(
  args: Record<string, unknown>,
): Promise<MacOSControlResponse & { id: string }> {
  const start = Date.now();
  const action = args.action as string;
  const appName = args.appName as string;
  const selector = args.selector ? JSON.stringify(args.selector) : '{}';
  const cliArgs = [action, appName, selector];
  if (args.text) cliArgs.push(args.text as string);
  const r = await runBinary('app-automation', cliArgs);
  return {
    id: '',
    success: r.code === 0,
    output: r.stdout,
    error: r.stderr || undefined,
    duration_ms: Date.now() - start,
  };
}

async function killBackground(
  args: Record<string, unknown>,
): Promise<MacOSControlResponse & { id: string }> {
  const start = Date.now();
  const id = args.processId as string;
  const proc = backgroundProcesses.get(id);
  if (proc) {
    proc.kill('SIGTERM');
    backgroundProcesses.delete(id);
    return {
      id: '',
      success: true,
      output: `Process ${id} killed`,
      duration_ms: Date.now() - start,
    };
  }
  return {
    id: '',
    success: false,
    error: `Process ${id} not found`,
    duration_ms: Date.now() - start,
  };
}

async function listFrameworks(): Promise<
  MacOSControlResponse & { id: string }
> {
  const start = Date.now();
  const r = await runBinary('header-index', ['discover']);
  return {
    id: '',
    success: r.code === 0,
    output: r.stdout,
    error: r.stderr || undefined,
    duration_ms: Date.now() - start,
  };
}

// --- IPC Processing ---

const HANDLERS: Record<
  string,
  (
    args: Record<string, unknown>,
  ) => Promise<MacOSControlResponse & { id: string }>
> = {
  discover_framework: discoverFramework,
  query_apis: queryApis,
  swift_interface: swiftInterface,
  execute_code: executeCode,
  observe_ui: observeUi,
  perform_action: performAction,
  kill_background: killBackground,
  list_frameworks: listFrameworks,
};

export async function processMacOSControlRequest(
  request: MacOSControlRequest,
): Promise<MacOSControlResponse> {
  const handler = HANDLERS[request.command];
  if (!handler) {
    return {
      id: request.id,
      success: false,
      error: `Unknown command: ${request.command}`,
      duration_ms: 0,
    };
  }

  try {
    const result = await handler(request.args);
    result.id = request.id;
    return result;
  } catch (err) {
    return {
      id: request.id,
      success: false,
      error: String(err),
      duration_ms: 0,
    };
  }
}

/**
 * Start watching for macOS control IPC requests.
 * Scans for .request.json files, processes them, writes .response.json files.
 */
export function startMacOSControlWatcher(): void {
  const ipcBase = path.join(DATA_DIR, 'ipc');
  const pollInterval = 500;

  const poll = async () => {
    try {
      const groupFolders = fs.readdirSync(ipcBase).filter((f) => {
        try {
          return fs.statSync(path.join(ipcBase, f)).isDirectory();
        } catch {
          return false;
        }
      });

      for (const group of groupFolders) {
        const controlDir = path.join(ipcBase, group, 'macos_control');
        if (!fs.existsSync(controlDir)) continue;

        const requests = fs
          .readdirSync(controlDir)
          .filter((f) => f.endsWith('.request.json'));
        for (const file of requests) {
          const filePath = path.join(controlDir, file);
          try {
            const request: MacOSControlRequest = JSON.parse(
              fs.readFileSync(filePath, 'utf-8'),
            );
            fs.unlinkSync(filePath);

            const response = await processMacOSControlRequest(request);

            const responseFile = file.replace(
              '.request.json',
              '.response.json',
            );
            const responsePath = path.join(controlDir, responseFile);
            const tmpPath = `${responsePath}.tmp`;
            fs.writeFileSync(tmpPath, JSON.stringify(response));
            fs.renameSync(tmpPath, responsePath);

            logger.debug(
              {
                command: request.command,
                group,
                duration: response.duration_ms,
              },
              'macOS control request processed',
            );
          } catch (err) {
            logger.error(
              { file, group, err },
              'Error processing macOS control request',
            );
            try {
              fs.unlinkSync(filePath);
            } catch {}
          }
        }
      }
    } catch (err) {
      logger.error({ err }, 'Error in macOS control watcher');
    }
    setTimeout(poll, pollInterval);
  };

  poll();
  logger.info('macOS control watcher started');
}

/** Kill all background processes on shutdown */
export function cleanupBackgroundProcesses(): void {
  for (const [id, proc] of backgroundProcesses) {
    logger.info({ id }, 'Killing background process');
    proc.kill('SIGTERM');
  }
  backgroundProcesses.clear();
}
