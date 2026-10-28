/** Explicit illustrative usage. Never attached to a live runtime or presented as account history. */
export function fixtureCosts(snapshot) {
  const route = snapshot.routes.find((item) => item.id === 'route-opencode-api');
  const model = snapshot.models.find((item) => item.routeId === route?.id);
  let prices = model ? [{ providerId: route.providerId, nativeModelId: model.nativeId, inputUsdPerMillion: '3', outputUsdPerMillion: '15', reviewedAt: '2026-10-28', source: 'Illustrative example rates; not a provider price claim' }] : [];
  const counts = [[1800, 400], [2400, 650], [1200, 320], [3200, 820], [4100, 1100], [2100, 600], [1800, 450], [3600, 900], [2800, 700], [1600, 350], [2200, 580], [3000, 740]];
  const end = Date.now();
  const observations = model ? counts.map(([input, output], index) => ({
    sessionId: 'example-cost-session', turnId: `example-turn-${index}`, connectionId: route.connectionId, routeId: route.id, modelId: model.id,
    observedAt: new Date(end - (counts.length - index) * 3600000).toISOString(), inputTokens: String(input), outputTokens: String(output), reportedUsd: null, complete: true,
  })) : [];
  return {
    read() {
      const price = prices.find((item) => item.providerId === route?.providerId && item.nativeModelId === model?.nativeId);
      return { records: observations.map((usage) => ({ usage, providerId: route.providerId, nativeModelId: model.nativeId, price: price ?? null,
        amountUsd: price ? String((Number(usage.inputTokens) * Number(price.inputUsdPerMillion) + Number(usage.outputTokens) * Number(price.outputUsdPerMillion)) / 1000000) : null,
        basis: price ? 'estimated' : 'unknown',
      })), prices, scope: 'Illustrative token counts × example prices. No provider was called or charged.', canEditPrices: true, pricesPersisted: false, storageError: null };
    },
    set(values) {
      if (!Array.isArray(values) || values.length > 128 || values.some((value) => !value || typeof value.providerId !== 'string' || !value.providerId || typeof value.nativeModelId !== 'string' || !value.nativeModelId || !/^\d+(\.\d+)?$/.test(value.inputUsdPerMillion) || !/^\d+(\.\d+)?$/.test(value.outputUsdPerMillion))) throw Object.assign(new Error('INVALID_REQUEST'), { code: 'INVALID_REQUEST' });
      prices = structuredClone(values);
      return prices;
    },
  };
}
