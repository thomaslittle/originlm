import type { Catalog, SnapshotView } from './types.ts';
export function selectionKey(routeId: string, modelId: string): string {
  return JSON.stringify([routeId, modelId]);
}
function unique<T extends { id: string }>(values: readonly T[], kind: string): Map<string, T> {
  const map = new Map<string, T>();
  for (const value of values) {
    if (!value.id || map.has(value.id)) throw new Error(`Duplicate or empty ${kind} identifier`);
    map.set(value.id, value);
  }
  return map;
}
export function createCatalog(snapshot: SnapshotView): Catalog {
  const connections = unique(snapshot.connections, 'connection');
  const routes = unique(snapshot.routes, 'route');
  const models = new Map<string, import('./types.ts').IndexedModel>();
  const diagnostics: { modelId: string; reason: string }[] = [];
  for (const model of unique(snapshot.models, 'model').values()) {
    const route = routes.get(model.routeId);
    const connection = route && connections.get(route.connectionId);
    if (!route || !connection) {
      diagnostics.push({ modelId: model.id, reason: route ? 'CONNECTION_UNAVAILABLE' : 'ROUTE_UNAVAILABLE' });
      continue;
    }
    const search = [model.name, model.nativeId, route.displayName, route.providerId, connection.displayName,
      connection.providerId, ...model.capabilities.map(c => c.id)].join(' ').normalize('NFKC').toLowerCase();
    models.set(model.id, { model, route, connection, search });
  }
  return { models, diagnostics };
}
