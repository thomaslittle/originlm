import type { CachedUsageMcpHandler } from '../dist/cached-usage-mcp.js';
import type { Readable, Writable } from 'node:stream';
export declare function serveCachedUsageMcp(handler: CachedUsageMcpHandler, options?: { input?: Readable; output?: Writable }): Promise<void>;
