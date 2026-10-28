/** Usage-only stateless Streamable HTTP route. Mount on an approved host; never listens. */
import {cachedUsageToolDefinition} from './cached-usage-mcp.ts';
import type {CachedUsageProducer} from './cached-usage-host.ts';

export interface CachedUsageRemoteOptions {
  source: Pick<CachedUsageProducer,'read'>;
  publicOrigin: string;
  path: string;
  principalId: string;
  /** Existing host authentication verifies the credential; never accept tool-supplied identity. */
  authenticate(request: Request): Promise<{subject:string;scopes:readonly string[]}|undefined>;
  allowedBrowserOrigins?: readonly string[];
}

const protocol = '2025-06-18';
const requiredScope = 'kit.usage.cached.read';
const object = (value:unknown):value is Record<string,unknown> => !!value && typeof value === 'object' && !Array.isArray(value);

/** Constructor and initialization/list calls are cache-inert. No sessions, timers, tokens or server. */
export function createCachedUsageRemoteMcp(options: CachedUsageRemoteOptions) {
  const origin = new URL(options.publicOrigin);
  if (origin.protocol !== 'https:' || origin.username || origin.password || origin.pathname !== '/' || origin.search || origin.hash
      || !/^\/[a-zA-Z0-9/_-]{1,128}$/.test(options.path)
      || !/^[A-Za-z0-9_.:-]{1,256}$/.test(options.principalId)
      || typeof options.authenticate !== 'function' || typeof options.source?.read !== 'function') throw new Error('INVALID_USAGE_REMOTE_CONFIG');
  const expectedOrigin = origin.origin, path = options.path, subject = options.principalId;
  const authenticate = options.authenticate;
  const browserOrigins = new Set(options.allowedBrowserOrigins ?? []);
  const source = options.source;
  let revoked = false;
  const response = (status:number,body:unknown=null,extra:Record<string,string>={}) => new Response(body===null ? null : JSON.stringify(body),{
    status,headers:{'content-type':'application/json','cache-control':'no-store','x-content-type-options':'nosniff',...extra},
  });
  return {
    requiredScope,
    revoke() { revoked = true; },
    async handle(request:Request):Promise<Response> {
      const url = new URL(request.url);
      if (url.origin !== expectedOrigin || url.pathname !== path || url.search) return response(404);
      const browserOrigin = request.headers.get('origin');
      if (browserOrigin && !browserOrigins.has(browserOrigin)) return response(403);
      let principal;
      try { principal = await authenticate(request); } catch { return response(401,null,{'www-authenticate':'Bearer realm="kit-usage"'}); }
      if (!principal) return response(401,null,{'www-authenticate':'Bearer realm="kit-usage"'});
      if (revoked || principal.subject !== subject || !Array.isArray(principal.scopes) || !principal.scopes.includes(requiredScope)) return response(403);
      if (request.method !== 'POST') return response(405,null,{allow:'POST'}); // No SSE subscription/feed.
      if (!request.headers.get('content-type')?.split(';')[0]?.trim().match(/^application\/json$/i)) return response(415);
      if (!request.headers.get('accept')?.includes('application/json')) return response(406);
      const version = request.headers.get('mcp-protocol-version');
      if (version && version !== protocol) return response(400);
      const declared = request.headers.get('content-length');
      if (declared && (!/^\d+$/.test(declared) || Number(declared)>65536)) return response(413);
      let packet:unknown;
      try {
        const reader = request.body?.getReader();
        if (!reader) return response(400);
        const chunks:Uint8Array[]=[];let bytes=0;
        try {
          for (;;) {
            const chunk=await reader.read();if (chunk.done) break;
            bytes+=chunk.value.byteLength;
            if (bytes>65536) { await reader.cancel();return response(413); }
            chunks.push(chunk.value);
          }
        } finally { reader.releaseLock(); }
        const input=new Uint8Array(bytes);let offset=0;
        for(const chunk of chunks){input.set(chunk,offset);offset+=chunk.byteLength;}
        packet=JSON.parse(new TextDecoder('utf-8',{fatal:true}).decode(input));
      } catch { return response(400); }
      // Uploads can outlive their authorization; retirement must precede dispatch/cache access.
      if (revoked) return response(403);
      if (!object(packet) || packet.jsonrpc!=='2.0' || typeof packet.method!=='string'
          || (packet.params!==undefined && !object(packet.params))) return response(400);
      if (!Object.hasOwn(packet,'id')) return response(202); // Notifications never read usage.
      const id=packet.id;
      if (!(typeof id==='string' && id.length<=128) && !Number.isSafeInteger(id)) return response(400);
      const params=(packet.params ?? {}) as Record<string,unknown>;
      const success=(result:unknown)=>response(200,{jsonrpc:'2.0',id,result});
      const error=(code:number,message:string)=>response(200,{jsonrpc:'2.0',id,error:{code,message}});
      if(packet.method==='initialize') {
        if(typeof params.protocolVersion!=='string' || !object(params.capabilities) || !object(params.clientInfo)) return error(-32602,'Invalid initialization');
        return success({protocolVersion:protocol,capabilities:{tools:{listChanged:false}},serverInfo:{name:'originlm-usage',version:'0.1.0-alpha.20'}});
      }
      if(packet.method==='ping') return success({});
      if(packet.method==='tools/list') {
        if(Object.keys(params).some(key=>key!=='_meta')) return error(-32602,'No tool-list cursor');
        return success({tools:[cachedUsageToolDefinition]});
      }
      if(packet.method!=='tools/call') return error(-32601,'Usage-only method unavailable');
      if(params.name!==cachedUsageToolDefinition.name || Object.keys(params).some(key=>!['name','arguments','_meta'].includes(key))
          || (params.arguments!==undefined && (!object(params.arguments)||Object.keys(params.arguments).length))) return error(-32602,'Invalid cached-usage arguments');
      let data;
      try { data=source.read(); } catch { return error(-32603,'Usage cache unavailable'); }
      return success({content:[{type:'text',text:JSON.stringify(data)}],structuredContent:data,isError:false});
    },
  };
}
