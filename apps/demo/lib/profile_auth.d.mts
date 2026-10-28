import type { Connection } from 'originlm';

export function connectionAuthLabel(connection: Pick<Connection, 'auth'> | null | undefined): string;
