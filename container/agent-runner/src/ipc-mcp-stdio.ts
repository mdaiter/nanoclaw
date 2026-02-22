/**
 * Stdio MCP Server for NanoClaw
 * Standalone process that agent teams subagents can inherit.
 * Reads context from environment variables, writes IPC files for the host.
 */

import { McpServer } from '@modelcontextprotocol/sdk/server/mcp.js';
import { StdioServerTransport } from '@modelcontextprotocol/sdk/server/stdio.js';
import { z } from 'zod';
import fs from 'fs';
import path from 'path';
import { CronExpressionParser } from 'cron-parser';

const IPC_DIR = '/workspace/ipc';
const MESSAGES_DIR = path.join(IPC_DIR, 'messages');
const TASKS_DIR = path.join(IPC_DIR, 'tasks');

// Context from environment variables (set by the agent runner)
const chatJid = process.env.NANOCLAW_CHAT_JID!;
const groupFolder = process.env.NANOCLAW_GROUP_FOLDER!;
const isMain = process.env.NANOCLAW_IS_MAIN === '1';

function writeIpcFile(dir: string, data: object): string {
  fs.mkdirSync(dir, { recursive: true });

  const filename = `${Date.now()}-${Math.random().toString(36).slice(2, 8)}.json`;
  const filepath = path.join(dir, filename);

  // Atomic write: temp file then rename
  const tempPath = `${filepath}.tmp`;
  fs.writeFileSync(tempPath, JSON.stringify(data, null, 2));
  fs.renameSync(tempPath, filepath);

  return filename;
}

const server = new McpServer({
  name: 'nanoclaw',
  version: '1.0.0',
});

server.tool(
  'send_message',
  "Send a message to the user or group immediately while you're still running. Use this for progress updates or to send multiple messages. You can call this multiple times. Note: when running as a scheduled task, your final output is NOT sent to the user — use this tool if you need to communicate with the user or group.",
  {
    text: z.string().describe('The message text to send'),
    sender: z
      .string()
      .optional()
      .describe(
        'Your role/identity name (e.g. "Researcher"). When set, messages appear from a dedicated bot in Telegram.',
      ),
  },
  async (args) => {
    const data: Record<string, string | undefined> = {
      type: 'message',
      chatJid,
      text: args.text,
      sender: args.sender || undefined,
      groupFolder,
      timestamp: new Date().toISOString(),
    };

    writeIpcFile(MESSAGES_DIR, data);

    return { content: [{ type: 'text' as const, text: 'Message sent.' }] };
  },
);

server.tool(
  'schedule_task',
  `Schedule a recurring or one-time task. The task will run as a full agent with access to all tools.

CONTEXT MODE - Choose based on task type:
\u2022 "group": Task runs in the group's conversation context, with access to chat history. Use for tasks that need context about ongoing discussions, user preferences, or recent interactions.
\u2022 "isolated": Task runs in a fresh session with no conversation history. Use for independent tasks that don't need prior context. When using isolated mode, include all necessary context in the prompt itself.

If unsure which mode to use, you can ask the user. Examples:
- "Remind me about our discussion" \u2192 group (needs conversation context)
- "Check the weather every morning" \u2192 isolated (self-contained task)
- "Follow up on my request" \u2192 group (needs to know what was requested)
- "Generate a daily report" \u2192 isolated (just needs instructions in prompt)

MESSAGING BEHAVIOR - The task agent's output is sent to the user or group. It can also use send_message for immediate delivery, or wrap output in <internal> tags to suppress it. Include guidance in the prompt about whether the agent should:
\u2022 Always send a message (e.g., reminders, daily briefings)
\u2022 Only send a message when there's something to report (e.g., "notify me if...")
\u2022 Never send a message (background maintenance tasks)

SCHEDULE VALUE FORMAT (all times are LOCAL timezone):
\u2022 cron: Standard cron expression (e.g., "*/5 * * * *" for every 5 minutes, "0 9 * * *" for daily at 9am LOCAL time)
\u2022 interval: Milliseconds between runs (e.g., "300000" for 5 minutes, "3600000" for 1 hour)
\u2022 once: Local time WITHOUT "Z" suffix (e.g., "2026-02-01T15:30:00"). Do NOT use UTC/Z suffix.`,
  {
    prompt: z
      .string()
      .describe(
        'What the agent should do when the task runs. For isolated mode, include all necessary context here.',
      ),
    schedule_type: z
      .enum(['cron', 'interval', 'once'])
      .describe(
        'cron=recurring at specific times, interval=recurring every N ms, once=run once at specific time',
      ),
    schedule_value: z
      .string()
      .describe(
        'cron: "*/5 * * * *" | interval: milliseconds like "300000" | once: local timestamp like "2026-02-01T15:30:00" (no Z suffix!)',
      ),
    context_mode: z
      .enum(['group', 'isolated'])
      .default('group')
      .describe(
        'group=runs with chat history and memory, isolated=fresh session (include context in prompt)',
      ),
    target_group_jid: z
      .string()
      .optional()
      .describe(
        '(Main group only) JID of the group to schedule the task for. Defaults to the current group.',
      ),
  },
  async (args) => {
    // Validate schedule_value before writing IPC
    if (args.schedule_type === 'cron') {
      try {
        CronExpressionParser.parse(args.schedule_value);
      } catch {
        return {
          content: [
            {
              type: 'text' as const,
              text: `Invalid cron: "${args.schedule_value}". Use format like "0 9 * * *" (daily 9am) or "*/5 * * * *" (every 5 min).`,
            },
          ],
          isError: true,
        };
      }
    } else if (args.schedule_type === 'interval') {
      const ms = parseInt(args.schedule_value, 10);
      if (isNaN(ms) || ms <= 0) {
        return {
          content: [
            {
              type: 'text' as const,
              text: `Invalid interval: "${args.schedule_value}". Must be positive milliseconds (e.g., "300000" for 5 min).`,
            },
          ],
          isError: true,
        };
      }
    } else if (args.schedule_type === 'once') {
      const date = new Date(args.schedule_value);
      if (isNaN(date.getTime())) {
        return {
          content: [
            {
              type: 'text' as const,
              text: `Invalid timestamp: "${args.schedule_value}". Use ISO 8601 format like "2026-02-01T15:30:00.000Z".`,
            },
          ],
          isError: true,
        };
      }
    }

    // Non-main groups can only schedule for themselves
    const targetJid =
      isMain && args.target_group_jid ? args.target_group_jid : chatJid;

    const data = {
      type: 'schedule_task',
      prompt: args.prompt,
      schedule_type: args.schedule_type,
      schedule_value: args.schedule_value,
      context_mode: args.context_mode || 'group',
      targetJid,
      createdBy: groupFolder,
      timestamp: new Date().toISOString(),
    };

    const filename = writeIpcFile(TASKS_DIR, data);

    return {
      content: [
        {
          type: 'text' as const,
          text: `Task scheduled (${filename}): ${args.schedule_type} - ${args.schedule_value}`,
        },
      ],
    };
  },
);

server.tool(
  'list_tasks',
  "List all scheduled tasks. From main: shows all tasks. From other groups: shows only that group's tasks.",
  {},
  async () => {
    const tasksFile = path.join(IPC_DIR, 'current_tasks.json');

    try {
      if (!fs.existsSync(tasksFile)) {
        return {
          content: [
            { type: 'text' as const, text: 'No scheduled tasks found.' },
          ],
        };
      }

      const allTasks = JSON.parse(fs.readFileSync(tasksFile, 'utf-8'));

      const tasks = isMain
        ? allTasks
        : allTasks.filter(
            (t: { groupFolder: string }) => t.groupFolder === groupFolder,
          );

      if (tasks.length === 0) {
        return {
          content: [
            { type: 'text' as const, text: 'No scheduled tasks found.' },
          ],
        };
      }

      const formatted = tasks
        .map(
          (t: {
            id: string;
            prompt: string;
            schedule_type: string;
            schedule_value: string;
            status: string;
            next_run: string;
          }) =>
            `- [${t.id}] ${t.prompt.slice(0, 50)}... (${t.schedule_type}: ${t.schedule_value}) - ${t.status}, next: ${t.next_run || 'N/A'}`,
        )
        .join('\n');

      return {
        content: [
          { type: 'text' as const, text: `Scheduled tasks:\n${formatted}` },
        ],
      };
    } catch (err) {
      return {
        content: [
          {
            type: 'text' as const,
            text: `Error reading tasks: ${err instanceof Error ? err.message : String(err)}`,
          },
        ],
      };
    }
  },
);

server.tool(
  'pause_task',
  'Pause a scheduled task. It will not run until resumed.',
  { task_id: z.string().describe('The task ID to pause') },
  async (args) => {
    const data = {
      type: 'pause_task',
      taskId: args.task_id,
      groupFolder,
      isMain,
      timestamp: new Date().toISOString(),
    };

    writeIpcFile(TASKS_DIR, data);

    return {
      content: [
        {
          type: 'text' as const,
          text: `Task ${args.task_id} pause requested.`,
        },
      ],
    };
  },
);

server.tool(
  'resume_task',
  'Resume a paused task.',
  { task_id: z.string().describe('The task ID to resume') },
  async (args) => {
    const data = {
      type: 'resume_task',
      taskId: args.task_id,
      groupFolder,
      isMain,
      timestamp: new Date().toISOString(),
    };

    writeIpcFile(TASKS_DIR, data);

    return {
      content: [
        {
          type: 'text' as const,
          text: `Task ${args.task_id} resume requested.`,
        },
      ],
    };
  },
);

server.tool(
  'cancel_task',
  'Cancel and delete a scheduled task.',
  { task_id: z.string().describe('The task ID to cancel') },
  async (args) => {
    const data = {
      type: 'cancel_task',
      taskId: args.task_id,
      groupFolder,
      isMain,
      timestamp: new Date().toISOString(),
    };

    writeIpcFile(TASKS_DIR, data);

    return {
      content: [
        {
          type: 'text' as const,
          text: `Task ${args.task_id} cancellation requested.`,
        },
      ],
    };
  },
);

server.tool(
  'register_group',
  `Register a new WhatsApp group so the agent can respond to messages there. Main group only.

Use available_groups.json to find the JID for a group. The folder name should be lowercase with hyphens (e.g., "family-chat").`,
  {
    jid: z
      .string()
      .describe('The WhatsApp JID (e.g., "120363336345536173@g.us")'),
    name: z.string().describe('Display name for the group'),
    folder: z
      .string()
      .describe(
        'Folder name for group files (lowercase, hyphens, e.g., "family-chat")',
      ),
    trigger: z.string().describe('Trigger word (e.g., "@Andy")'),
  },
  async (args) => {
    if (!isMain) {
      return {
        content: [
          {
            type: 'text' as const,
            text: 'Only the main group can register new groups.',
          },
        ],
        isError: true,
      };
    }

    const data = {
      type: 'register_group',
      jid: args.jid,
      name: args.name,
      folder: args.folder,
      trigger: args.trigger,
      timestamp: new Date().toISOString(),
    };

    writeIpcFile(TASKS_DIR, data);

    return {
      content: [
        {
          type: 'text' as const,
          text: `Group "${args.name}" registered. It will start receiving messages immediately.`,
        },
      ],
    };
  },
);

// =============================================================================
// macOS Control tools — request-response IPC to host native binaries
// =============================================================================

const MACOS_CONTROL_DIR = path.join(IPC_DIR, 'macos_control');

function macosControlRequest(
  command: string,
  args: Record<string, unknown>,
  timeoutMs = 30_000,
): Promise<{ success: boolean; output?: string; error?: string }> {
  fs.mkdirSync(MACOS_CONTROL_DIR, { recursive: true });
  const id = `${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
  const reqFile = path.join(MACOS_CONTROL_DIR, `${id}.request.json`);
  const resFile = path.join(MACOS_CONTROL_DIR, `${id}.response.json`);

  // Write request
  const tmpReq = `${reqFile}.tmp`;
  fs.writeFileSync(tmpReq, JSON.stringify({ id, command, args }));
  fs.renameSync(tmpReq, reqFile);

  // Poll for response
  return new Promise((resolve) => {
    const start = Date.now();
    const poll = () => {
      if (fs.existsSync(resFile)) {
        try {
          const data = JSON.parse(fs.readFileSync(resFile, 'utf-8'));
          fs.unlinkSync(resFile);
          resolve(data);
        } catch {
          resolve({ success: false, error: 'Failed to parse response' });
        }
        return;
      }
      if (Date.now() - start > timeoutMs) {
        resolve({
          success: false,
          error: `Timeout waiting for response (${timeoutMs}ms)`,
        });
        return;
      }
      setTimeout(poll, 250);
    };
    poll();
  });
}

server.tool(
  'discover_framework',
  `Discover APIs in a macOS framework (public or private). Extracts exports from the DYLD shared cache, infers signatures from naming patterns, and generates a Swift interface from binary metadata. Use this to find out what functions/classes a framework provides before writing code to use them.`,
  {
    framework: z
      .string()
      .describe(
        'Framework name (e.g., "CoreDock", "SkyLight", "Metal", "IOSurface")',
      ),
  },
  async (args) => {
    const r = await macosControlRequest('discover_framework', args, 60_000);
    return {
      content: [
        {
          type: 'text' as const,
          text: r.success ? r.output! : `Error: ${r.error}`,
        },
      ],
    };
  },
);

server.tool(
  'query_apis',
  `Search for macOS APIs by natural language query across all previously discovered frameworks. Searches the knowledge base cached at ~/.mcgyver/kb/. Run discover_framework first to populate the KB.`,
  {
    query: z
      .string()
      .describe(
        'Natural language search (e.g., "dock tile label", "status bar item", "window compositing")',
      ),
  },
  async (args) => {
    const r = await macosControlRequest('query_apis', args);
    return {
      content: [
        {
          type: 'text' as const,
          text: r.success ? r.output! : `Error: ${r.error}`,
        },
      ],
    };
  },
);

server.tool(
  'swift_interface',
  `Generate a complete Swift interface for a framework from the DYLD shared cache. Shows all types, protocols, functions, and extensions as they appear in the binary. More detailed than discover_framework for Swift-heavy frameworks.`,
  {
    framework: z
      .string()
      .describe('Framework name (e.g., "SwiftUI", "AttributeGraph")'),
  },
  async (args) => {
    const r = await macosControlRequest('swift_interface', args, 60_000);
    return {
      content: [
        {
          type: 'text' as const,
          text: r.success ? r.output! : `Error: ${r.error}`,
        },
      ],
    };
  },
);

server.tool(
  'execute_code',
  `Compile and run Swift or Python code on the host macOS machine. Swift code is compiled with swiftc and can link any framework including PrivateFrameworks. Use this to call discovered APIs, create UI elements, or interact with the system.

For persistent processes (menu bar items, dock tiles, real-time updates), set background=true. The process runs detached and you get a processId to kill it later.`,
  {
    code: z.string().describe('Swift or Python source code'),
    language: z
      .enum(['swift', 'python'])
      .default('swift')
      .describe('Programming language'),
    frameworks: z
      .array(z.string())
      .default([])
      .describe('Public frameworks to link (e.g., ["AppKit", "CoreGraphics"])'),
    privateFrameworks: z
      .array(z.string())
      .default([])
      .describe('Private frameworks to link (e.g., ["SkyLight", "CoreDock"])'),
    background: z
      .boolean()
      .default(false)
      .describe(
        'Run as a background process (for persistent UI like menu bar items)',
      ),
    timeout: z
      .number()
      .default(30000)
      .describe('Execution timeout in ms (ignored for background processes)'),
  },
  async (args) => {
    const r = await macosControlRequest(
      'execute_code',
      args,
      args.background ? 10_000 : args.timeout + 5_000,
    );
    return {
      content: [
        {
          type: 'text' as const,
          text: r.success
            ? r.output || 'Code executed successfully.'
            : `Error: ${r.error}`,
        },
      ],
    };
  },
);

server.tool(
  'observe_ui',
  `Get an accessibility snapshot of a running macOS app. Returns the UI tree with element IDs, roles, titles, values, and available actions. Use this to verify visual changes after executing code.`,
  {
    appName: z
      .string()
      .describe('Application name (e.g., "Finder", "Safari", "Dock")'),
  },
  async (args) => {
    const r = await macosControlRequest('observe_ui', args);
    return {
      content: [
        {
          type: 'text' as const,
          text: r.success ? r.output! : `Error: ${r.error}`,
        },
      ],
    };
  },
);

server.tool(
  'perform_action',
  `Perform a UI action on a macOS app via accessibility. Can click, type, scroll, or press keys.`,
  {
    appName: z.string().describe('Application name'),
    action: z
      .enum(['click', 'type', 'scroll', 'pressKey'])
      .describe('Action to perform'),
    selector: z
      .string()
      .optional()
      .describe('JSON selector to find the target element'),
    text: z.string().optional().describe('Text to type (for "type" action)'),
  },
  async (args) => {
    const r = await macosControlRequest('perform_action', args);
    return {
      content: [
        {
          type: 'text' as const,
          text: r.success
            ? r.output || 'Action performed.'
            : `Error: ${r.error}`,
        },
      ],
    };
  },
);

server.tool(
  'kill_background',
  `Kill a background process started by execute_code with background=true.`,
  {
    processId: z.string().describe('The process ID returned by execute_code'),
  },
  async (args) => {
    const r = await macosControlRequest('kill_background', args);
    return {
      content: [
        {
          type: 'text' as const,
          text: r.success ? r.output! : `Error: ${r.error}`,
        },
      ],
    };
  },
);

server.tool(
  'list_frameworks',
  `Discover all macOS frameworks on this system (public and private). Extracts from /System/Library/Frameworks and /System/Library/PrivateFrameworks. Results are cached — subsequent calls are fast.`,
  {},
  async () => {
    const r = await macosControlRequest('list_frameworks', {}, 120_000);
    return {
      content: [
        {
          type: 'text' as const,
          text: r.success ? r.output! : `Error: ${r.error}`,
        },
      ],
    };
  },
);

// Start the stdio transport
const transport = new StdioServerTransport();
await server.connect(transport);
