import { createHttpTransport, createLocalAI, type LocalAIClient } from 'originlm';

/** The single place the demo opens an SDK client over the public HTTP transport. */
export function connectRuntime(endpoint: string, token: string): Promise<LocalAIClient> {
  return createLocalAI({ clientName: 'originlm-showcase', transport: createHttpTransport({ baseUrl: endpoint, token }) });
}
