import type { LocalAIClient, Snapshot, Model, Connection, Session } from '../../packages/sdk-typescript/dist/index.js';
declare const ai: LocalAIClient;
const snapshot: Snapshot = await ai.snapshot.get();
const models: Model[] = await ai.models.list();
const connections: Connection[] = await ai.connections.list();
const projected = await ai.snapshot.get({ fields: ['connections', 'revision'] });
const projectedConnections: Connection[] = projected.connections;
// @ts-expect-error A projection must not claim an unrequested field.
projected.models;
// @ts-expect-error Session creation requires a selection and workspace.
ai.sessions.create();
// @ts-expect-error Only actual snapshot fields can be projected.
ai.snapshot.get({ fields: ['not_a_snapshot_field'] });
// @ts-expect-error A probe requires the native connection identifier.
ai.connections.probe({ label: 'wrong argument' });
const session: Session = await ai.sessions.create({ selection: { routeId: 'r', modelId: 'm', revision: 'v', options: {} }, workspaceId: 'default' });
void [snapshot, models, connections, projectedConnections, session];
