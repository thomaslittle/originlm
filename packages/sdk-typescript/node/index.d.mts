import type { Transport } from '../dist/types.js';
export interface StdioOptions {
  /** Absolute path to the approved native binary. Shell scripts are not launched. */
  executable: string;
  args?: string[];
  cwd?: string;
  env?: Record<string, string>;
  timeoutMs?: number;
  maxFrameBytes?: number;
  maxPendingRequests?: number;
  maxQueuedEvents?: number;
}
export declare function openStdioTransport(options: StdioOptions): Promise<Transport>;

export interface BrokerOptions extends Omit<StdioOptions,"args"> { address: string; token: string }
export declare function openBrokerTransport(options: BrokerOptions): Promise<Transport>;

export interface SharedBrokerOptions extends Omit<StdioOptions,"args"> {
  address: string;
  tokenFile: string;
  /** Extra permission flags passed only when this SDK has to start the shared broker. */
  startArgs?: string[];
  waitMs?: number;
}
export declare function openSharedBrokerTransport(options: SharedBrokerOptions): Promise<Transport>;

export declare function resolveBundledBinary(directory: string): Promise<string>;
export interface BundleOptions extends Omit<StdioOptions,"executable"> { bundleDirectory: string }
export declare function openBundledTransport(options: BundleOptions): Promise<Transport>;
