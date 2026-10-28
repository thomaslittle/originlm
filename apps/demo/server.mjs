/** Local synthetic demo harness. It cannot inspect accounts or execute provider clients. */
import {createServer} from 'node:http';
import {randomBytes,timingSafeEqual} from 'node:crypto';
import {readFile,stat} from 'node:fs/promises';
import {resolve,dirname,join,sep} from 'node:path';
import {fileURLToPath,pathToFileURL} from 'node:url';
import {createFixtureService} from './fixture/service.mjs';
const HERE=dirname(fileURLToPath(import.meta.url));
const DIST=resolve(HERE,'dist');
const MIME={html:'text/html',css:'text/css',js:'text/javascript',mjs:'text/javascript',svg:'image/svg+xml',woff2:'font/woff2'};
const CLIENT_ROUTES=new Set(['/overview','/connections','/models','/usage','/selector-lab','/playground','/diagnostics','/settings']);
const SEGMENT=/^[A-Za-z0-9][A-Za-z0-9._-]*$/;
export async function createFixtureServer({port=0}={}){
 if(!Number.isInteger(port)||port<0||port>65535)throw Error('INVALID_PORT');
 const service=createFixtureService(),token=randomBytes(32).toString('base64url'),streams=new Set();let url,closing;
 const headers={
  'Cache-Control':'no-store','X-Content-Type-Options':'nosniff','Referrer-Policy':'no-referrer',
  'Content-Security-Policy':"default-src 'self'; script-src 'self'; style-src 'self'; connect-src 'self' http://127.0.0.1:* http://localhost:*; img-src 'self' data:; base-uri 'none'; form-action 'none'; frame-ancestors 'none'; object-src 'none'",
 };
 const sameToken=value=>{if(typeof value!=='string')return false;const a=Buffer.from(value),b=Buffer.from('Bearer '+token);return a.length===b.length&&timingSafeEqual(a,b);};
 const json=(res,status,value)=>{res.writeHead(status,{...headers,'Content-Type':'application/json'});res.end(JSON.stringify(value));};
 const server=createServer(async(req,res)=>{
  try{
   const current=new URL(req.url,url);
   if(req.headers.host!==new URL(url).host||(req.headers.origin&&req.headers.origin!==url)){json(res,403,{error:'ORIGIN_DENIED'});return;}
   if([...current.searchParams.keys()].some(k=>current.pathname!=='/v1/events'||!['sessionId','after'].includes(k))){json(res,400,{error:'INVALID_REQUEST'});return;}
   if(current.pathname==='/runtime/info'&&req.method==='GET'){json(res,200,{mode:'synthetic',pairingRequired:false});return;}
   if(current.pathname==='/fixture/bootstrap'&&req.method==='GET'){
    if(req.headers['sec-fetch-site']==='cross-site'){json(res,403,{error:'ORIGIN_DENIED'});return;}
    json(res,200,{mode:'synthetic',sessionToken:token});return;
   }
   if(current.pathname.startsWith('/v1/')){
    if(!sameToken(req.headers.authorization)){json(res,401,{error:'AUTH_REQUIRED'});return;}
    if(current.pathname==='/v1/rpc'&&req.method==='POST'){
     if(!req.headers['content-type']?.startsWith('application/json')){json(res,415,{error:'CONTENT_TYPE_REQUIRED'});return;}
     if(Number(req.headers['content-length'])>1048576){req.resume();json(res,413,{error:'FRAME_TOO_LARGE'});return;}
     const parts=[];let bytes=0;
     for await(const part of req){bytes+=part.length;if(bytes>1048576){json(res,413,{error:'FRAME_TOO_LARGE'});return;}parts.push(part);}
     let rpc;try{rpc=JSON.parse(Buffer.concat(parts).toString('utf8'));}catch{json(res,400,{error:'INVALID_JSON'});return;}
     if(!rpc||rpc.jsonrpc!=='2.0'||!Number.isSafeInteger(rpc.id)||typeof rpc.method!=='string'||rpc.method.length>128){json(res,400,{error:'INVALID_REQUEST'});return;}
     try{json(res,200,{jsonrpc:'2.0',id:rpc.id,result:service.handle(rpc.method,rpc.params??{})});}
     catch(error){const code=typeof error.code==='string'&&/^[A-Z_]{1,64}$/.test(error.code)?error.code:'INVALID_REQUEST';json(res,200,{jsonrpc:'2.0',id:rpc.id,error:{code:-32000,message:code,data:{code,retryable:false}}});}
     return;
    }
    if(current.pathname==='/v1/events'&&req.method==='GET'){
     const sessionId=current.searchParams.get('sessionId')??undefined,after=current.searchParams.get('after')??undefined;
     if((sessionId&&sessionId.length>256)||(after&&after.length>32)){json(res,400,{error:'INVALID_REQUEST'});return;}
     if(streams.size>=32){json(res,429,{error:'RESOURCE_LIMIT'});return;}
     res.writeHead(200,{...headers,'Content-Type':'text/event-stream','Connection':'keep-alive'});res.write(': connected\n\n');streams.add(res);
     const send=event=>{if((!sessionId||event.sessionId===sessionId)&&!res.destroyed){if(res.writableLength>1048576){res.end('event: gap\ndata: {}\n\n');return;}res.write(`id: ${event.sequence}\nevent: update\ndata: ${JSON.stringify(event)}\n\n`);}};
     try{for(const event of service.replay(after))send(event);}catch{res.end('event: gap\ndata: {}\n\n');streams.delete(res);return;}
     const remove=service.subscribe(send),heartbeat=setInterval(()=>{if(!res.destroyed)res.write(': heartbeat\n\n');},15000);
     res.on('close',()=>{remove();clearInterval(heartbeat);streams.delete(res);});return;
    }
    json(res,404,{error:'NOT_FOUND'});return;
   }
   // Static showcase assets. Mirrors the native helper: only extension-allowlisted
   // files without traversal segments are served, capped at 2 MiB.
   if(req.method!=='GET'&&req.method!=='HEAD'){json(res,405,{error:'METHOD_NOT_ALLOWED'});return;}
   const name=current.pathname==='/'||CLIENT_ROUTES.has(current.pathname)?'index.html':current.pathname.replace(/^\/+/,'');
   const parts=name.split('/');
   const extension=name.includes('.')?name.split('.').pop():'';
   const ok=name.length>0&&name.length<=256&&parts.every(part=>SEGMENT.test(part))&&Object.hasOwn(MIME,extension);
   if(!ok){json(res,404,{error:'NOT_FOUND'});return;}
   const path=resolve(DIST,...parts);
   if(path!==DIST&&!path.startsWith(DIST+sep)){json(res,404,{error:'NOT_FOUND'});return;}
   let content;try{
    const info=await stat(path);
    if(!info.isFile()||info.size>2097152){json(res,404,{error:'NOT_FOUND'});return;}
    content=await readFile(path);
   }catch{json(res,404,{error:'BUILD_ASSET_MISSING',hint:'Run `npm run build:demo` first.'});return;}
   res.writeHead(200,{...headers,'Content-Type':MIME[extension]+(extension==='woff2'?'':'; charset=utf-8'),'Content-Length':content.length});if(req.method==='GET')res.end(content);else res.end();
  }catch{if(!res.headersSent)json(res,400,{error:'INVALID_REQUEST'});else res.end();}
 });
 server.maxConnections=64;server.headersTimeout=10000;server.requestTimeout=15000;server.keepAliveTimeout=5000;
 await new Promise((resolve,reject)=>{server.once('error',reject);server.listen(port,'127.0.0.1',resolve);});url=`http://127.0.0.1:${server.address().port}`;
 return {url,close(){if(!closing){closing=new Promise(resolve=>{service.close();for(const res of streams)res.end();server.close(resolve);server.closeAllConnections();});}return closing;}};
}
if(process.argv[1]&&pathToFileURL(resolve(process.argv[1])).href===import.meta.url){
 if(!process.argv.includes('--fixture')){console.error('This preview server requires --fixture. It never discovers real accounts.');process.exitCode=2;}
 else{
  const index=process.argv.indexOf('--port'),port=index<0?4499:Number(process.argv[index+1]);
  const app=await createFixtureServer({port});console.log(`OriginLM synthetic showcase: ${app.url}`);
  for(const signal of ['SIGINT','SIGTERM'])process.once(signal,async()=>{await app.close();process.exit(0);});
 }
}
