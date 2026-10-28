import type { Catalog, OptionView, Reconciliation, Selection } from './types.ts';
export function validateOption(descriptor: OptionView, value: unknown): string | null {
  if (!descriptor.id || !descriptor.revision) return 'INVALID_DESCRIPTOR';
  const control = descriptor.control;
  let valid = false;
  switch (control.type) {
    case 'select':
      valid = typeof value === 'string' && control.choices.some(c => c.value === value && !c.disabledReason);
      break;
    case 'boolean': valid = typeof value === 'boolean'; break;
    case 'text':
      valid = typeof value === 'string' && !value.includes('\0') && [...value].length <= control.maxLength;
      break;
    case 'number': {
      if (typeof value !== 'number' || ![value, control.min, control.max, control.step].every(Number.isFinite)
        || control.step <= 0 || control.min > control.max || value < control.min || value > control.max) break;
      const steps = (value - control.min) / control.step;
      valid = Math.abs(steps - Math.round(steps)) <= 1e-8;
      break;
    }
    case 'unknown': break;
  }
  return valid ? null : `OPTION_UNSUPPORTED:${descriptor.id}`;
}
export function reconcileSelection(catalog: Catalog, selection: Selection): Reconciliation {
  const entry = catalog.models.get(selection.modelId);
  if (!entry) return { selection: null, needsReview: true, removedOptions: [], reason: 'MODEL_UNAVAILABLE' };
  if (entry.route.id !== selection.routeId) return { selection: null, needsReview: true, removedOptions: [], reason: 'ROUTE_MISMATCH' };
  const descriptors = new Map(entry.model.options.map(d => [d.id, d]));
  const options: Record<string, unknown> = Object.create(null);
  const removedOptions: string[] = [];
  for (const [id, value] of Object.entries(selection.options)) {
    const descriptor = descriptors.get(id);
    if (!descriptor || validateOption(descriptor, value)) removedOptions.push(id);
    else options[id] = value;
  }
  return {
    selection: { routeId: selection.routeId, modelId: selection.modelId, revision: entry.model.revision, options: { ...options } },
    needsReview: removedOptions.length > 0 || selection.revision !== entry.model.revision,
    removedOptions,
  };
}
