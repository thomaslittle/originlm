/** Source integration for T3's existing authenticated Effect MCP host; no server startup. */
import type * as Context from 'effect/Context';
import * as Effect from 'effect/Effect';
import * as Schema from 'effect/Schema';
import {Tool, Toolkit} from 'effect/ai';
import {cachedUsageToolDefinition} from 'originlm/mcp';
import type {T3CachedUsageTool, T3UsageInvocation} from 'originlm/mcp/host';

/** Pass T3's real McpInvocationContext tag, never a captured registration-time caller. */
export function createKitUsageToolkit<I>(invocation: Context.Key<I, T3UsageInvocation>) {
  const tool = Tool.make('kit_usage_cached', {
    description:cachedUsageToolDefinition.description,
    // An empty Struct would strip unknown keys; Never rejects refresh/scope arguments.
    parameters:Schema.Record(Schema.String,Schema.Never),
    success:Schema.Unknown, // The Kit projection owns the canonical quota/window contract.
    failure:Schema.Struct({code:Schema.Literals(['capability_denied','usage_unavailable']),message:Schema.String}),
    failureMode:'return',
    dependencies:[invocation],
  }).annotate(Tool.Readonly,true).annotate(Tool.Destructive,false)
    .annotate(Tool.Idempotent,true).annotate(Tool.OpenWorld,false);
  return {
    toolkit:Toolkit.make(tool),
    handler:(binding:T3CachedUsageTool) => Effect.fn('KitUsage.read')(function* (params:Record<string,never>) {
      const caller = yield* invocation;
      return yield* Effect.try({
        try:()=>binding.read(caller,params),
        catch:(error)=>({
          code:error instanceof Error && error.message === 'CACHED_USAGE_ACCESS_DENIED'
            ? 'capability_denied' as const : 'usage_unavailable' as const,
          message:'Scoped cached usage is unavailable for this caller.',
        }),
      });
    }),
  };
}
